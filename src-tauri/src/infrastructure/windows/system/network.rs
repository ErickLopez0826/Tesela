//! Redes Wi-Fi, dispositivos Bluetooth y zona compartida con las API WinRT del sistema.
//!
//! Ver redes y dispositivos es solo lectura. Conectar, desconectar o cambiar la zona compartida corta o cambia
//! conexiones reales: solo se llama a esas funciones a través de `Guarded` con `live = true`. La contraseña de una red
//! se entrega a Windows y no se guarda ni se escribe en ningún registro.

use std::collections::HashMap;

use windows::core::HSTRING;
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice as WinBluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Devices::WiFi::{WiFiAccessStatus, WiFiAdapter, WiFiAvailableNetwork, WiFiConnectionStatus, WiFiReconnectionKind};
use windows::Networking::Connectivity::NetworkInformation;
use windows::Networking::NetworkOperators::{
    NetworkOperatorTetheringAccessPointConfiguration, NetworkOperatorTetheringManager, TetheringCapability,
};
use windows::Security::Credentials::PasswordCredential;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::application::system::{validate_hotspot, NetworkControl};
use crate::domain::system::{ActionOutcome, BluetoothDevice, HotspotClient, HotspotInfo, WifiNetwork};

pub struct WinRtNetwork;

fn err(e: windows::core::Error) -> String {
    e.message()
}

impl WinRtNetwork {
    pub fn new() -> Self {
        WinRtNetwork
    }

    fn init() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn adapter() -> Result<WiFiAdapter, String> {
        Self::init();
        let access = WiFiAdapter::RequestAccessAsync().map_err(err)?.join().map_err(err)?;
        if access != WiFiAccessStatus::Allowed {
            return Err("Windows no permite a la app ver las redes Wi-Fi (revisa Privacidad > Ubicación)".into());
        }
        let adapters = WiFiAdapter::FindAllAdaptersAsync().map_err(err)?.join().map_err(err)?;
        adapters.into_iter().next().ok_or_else(|| "este equipo no tiene adaptador Wi-Fi".to_string())
    }

    fn connected_ssid(adapter: &WiFiAdapter) -> Option<String> {
        let profile = adapter.NetworkAdapter().ok()?.GetConnectedProfileAsync().ok()?.join().ok()?;
        let ssid = profile.WlanConnectionProfileDetails().ok()?.GetConnectedSsid().ok()?;
        let text = ssid.to_string();
        (!text.is_empty()).then_some(text)
    }

    fn scan(adapter: &WiFiAdapter) -> Result<Vec<WiFiAvailableNetwork>, String> {
        adapter.ScanAsync().map_err(err)?.join().map_err(err)?;
        Ok(adapter.NetworkReport().map_err(err)?.AvailableNetworks().map_err(err)?.into_iter().collect())
    }

    fn tethering() -> Option<NetworkOperatorTetheringManager> {
        Self::init();
        let profile = NetworkInformation::GetInternetConnectionProfile().ok()?;
        if NetworkOperatorTetheringManager::GetTetheringCapabilityFromConnectionProfile(&profile).ok()? != TetheringCapability::Enabled {
            return None;
        }
        NetworkOperatorTetheringManager::CreateFromConnectionProfile(&profile).ok()
    }

    fn bluetooth_from(selector: HSTRING, low_energy: bool, out: &mut HashMap<String, BluetoothDevice>) {
        let Ok(op) = DeviceInformation::FindAllAsyncAqsFilter(&selector) else { return };
        let Ok(list) = op.join() else { return };
        for info in list {
            let Ok(id) = info.Id() else { continue };
            let (name, connected) = if low_energy {
                let Ok(d) = BluetoothLEDevice::FromIdAsync(&id).and_then(|o| o.join()) else { continue };
                (d.Name().map(|n| n.to_string()).unwrap_or_default(), d.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected))
            } else {
                let Ok(d) = WinBluetoothDevice::FromIdAsync(&id).and_then(|o| o.join()) else { continue };
                (d.Name().map(|n| n.to_string()).unwrap_or_default(), d.ConnectionStatus().ok() == Some(BluetoothConnectionStatus::Connected))
            };
            if name.trim().is_empty() {
                continue;
            }
            // Un mismo aparato puede aparecer por clásico y por baja energía: se junta por nombre.
            let entry = out.entry(name.clone()).or_insert(BluetoothDevice { id: id.to_string(), name, connected: false });
            entry.connected |= connected;
        }
    }
}

impl Default for WinRtNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkControl for WinRtNetwork {
    fn wifi_networks(&mut self) -> Result<Vec<WifiNetwork>, String> {
        let adapter = Self::adapter()?;
        let connected = Self::connected_ssid(&adapter);
        let mut best: HashMap<String, WifiNetwork> = HashMap::new();
        for n in Self::scan(&adapter)? {
            let ssid = n.Ssid().map(|s| s.to_string()).unwrap_or_default();
            if ssid.trim().is_empty() {
                continue; // red oculta
            }
            let signal = n.SignalBars().map(|b| (b as u32 * 25).min(100) as u8).unwrap_or(0);
            let secured = n
                .SecuritySettings()
                .and_then(|s| s.NetworkAuthenticationType())
                .map(|t| t != windows::Networking::Connectivity::NetworkAuthenticationType::Open80211)
                .unwrap_or(true);
            let entry = best.entry(ssid.clone()).or_insert(WifiNetwork { connected: connected.as_deref() == Some(ssid.as_str()), ssid, signal: 0, secured });
            entry.signal = entry.signal.max(signal);
        }
        let mut list: Vec<WifiNetwork> = best.into_values().collect();
        list.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.signal.cmp(&a.signal)).then(a.ssid.cmp(&b.ssid)));
        Ok(list)
    }

    fn wifi_connect(&mut self, ssid: &str, password: Option<&str>) -> Result<ActionOutcome, String> {
        let adapter = Self::adapter()?;
        let network = Self::scan(&adapter)?
            .into_iter()
            .filter(|n| n.Ssid().map(|s| s.to_string()).unwrap_or_default() == ssid)
            .max_by_key(|n| n.SignalBars().unwrap_or(0))
            .ok_or_else(|| format!("la red «{ssid}» ya no está a la vista"))?;
        let result = match password.filter(|p| !p.is_empty()) {
            Some(pass) => {
                let credential = PasswordCredential::new().map_err(err)?;
                credential.SetPassword(&HSTRING::from(pass)).map_err(err)?;
                adapter.ConnectWithPasswordCredentialAsync(&network, WiFiReconnectionKind::Automatic, &credential)
            }
            None => adapter.ConnectAsync(&network, WiFiReconnectionKind::Automatic),
        }
        .map_err(err)?
        .join()
        .map_err(err)?;
        match result.ConnectionStatus().map_err(err)? {
            WiFiConnectionStatus::Success => Ok(ActionOutcome::done(format!("Conectado a {ssid}"))),
            WiFiConnectionStatus::InvalidCredential => Err("contraseña incorrecta o falta la contraseña de esta red".into()),
            WiFiConnectionStatus::UnsupportedAuthenticationProtocol => Err("la red usa un tipo de seguridad que la app no puede gestionar".into()),
            WiFiConnectionStatus::NetworkNotAvailable => Err(format!("la red «{ssid}» no está disponible")),
            WiFiConnectionStatus::Timeout => Err("la conexión tardó demasiado".into()),
            other => Err(format!("no se pudo conectar a «{ssid}» ({other:?})")),
        }
    }

    fn wifi_disconnect(&mut self) -> Result<ActionOutcome, String> {
        Self::adapter()?.Disconnect().map_err(err)?;
        Ok(ActionOutcome::done("Wi-Fi desconectado"))
    }

    fn bluetooth_devices(&mut self) -> Result<Vec<BluetoothDevice>, String> {
        Self::init();
        let mut found: HashMap<String, BluetoothDevice> = HashMap::new();
        if let Ok(sel) = WinBluetoothDevice::GetDeviceSelectorFromPairingState(true) {
            Self::bluetooth_from(sel, false, &mut found);
        }
        if let Ok(sel) = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true) {
            Self::bluetooth_from(sel, true, &mut found);
        }
        let mut list: Vec<BluetoothDevice> = found.into_values().collect();
        list.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(list)
    }

    fn hotspot(&mut self) -> Result<Option<HotspotInfo>, String> {
        let Some(manager) = Self::tethering() else { return Ok(None) };
        let config = manager.GetCurrentAccessPointConfiguration().map_err(err)?;
        let clients = manager
            .GetTetheringClients()
            .map(|list| {
                list.into_iter()
                    .map(|c| HotspotClient {
                        mac: c.MacAddress().map(|m| m.to_string()).unwrap_or_default(),
                        name: c
                            .HostNames()
                            .ok()
                            .and_then(|h| h.into_iter().find_map(|n| n.DisplayName().ok().map(|d| d.to_string())))
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some(HotspotInfo {
            ssid: config.Ssid().map(|s| s.to_string()).unwrap_or_default(),
            passphrase: config.Passphrase().map(|s| s.to_string()).unwrap_or_default(),
            clients,
            max_clients: manager.MaxClientCount().unwrap_or(0),
        }))
    }

    fn hotspot_configure(&mut self, ssid: &str, passphrase: &str) -> Result<ActionOutcome, String> {
        validate_hotspot(ssid, passphrase)?;
        let manager = Self::tethering().ok_or("este equipo no puede compartir su conexión")?;
        let config = NetworkOperatorTetheringAccessPointConfiguration::new().map_err(err)?;
        config.SetSsid(&HSTRING::from(ssid)).map_err(err)?;
        config.SetPassphrase(&HSTRING::from(passphrase)).map_err(err)?;
        manager.ConfigureAccessPointAsync(&config).map_err(err)?.join().map_err(err)?;
        Ok(ActionOutcome::done("Configuración de compartir red guardada"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: no se conecta, no se desconecta y no se toca la zona compartida.
    #[test]
    fn lee_redes_dispositivos_y_zona_compartida_reales() {
        let mut n = WinRtNetwork::new();
        match n.wifi_networks() {
            Ok(list) => {
                println!("redes: {}", list.len());
                assert!(list.windows(2).all(|w| w[0].connected >= w[1].connected), "la conectada va primero");
                assert!(list.iter().all(|w| w.signal <= 100));
            }
            Err(e) => println!("sin lista de redes: {e}"),
        }
        match n.bluetooth_devices() {
            Ok(list) => println!("bluetooth: {}", list.len()),
            Err(e) => println!("sin lista de bluetooth: {e}"),
        }
        match n.hotspot() {
            Ok(Some(h)) => assert!(!h.ssid.is_empty() || h.clients.is_empty()),
            Ok(None) => println!("sin zona compartida"),
            Err(e) => println!("sin zona compartida: {e}"),
        }
    }
}
