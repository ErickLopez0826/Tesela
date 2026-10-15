//! Conectividad (§9.2): Wi-Fi, Bluetooth y compartir red, con las API WinRT del sistema.
//!
//! Leer el estado es seguro. Cambiarlo corta/enciende conexiones reales: solo se llama a `set`
//! a través de `Guarded` con `live = true` (`[system] allow_changes = true`).

use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioState as WinRadioState};
use windows::Networking::Connectivity::NetworkInformation;
use windows::Networking::NetworkOperators::{
    NetworkOperatorTetheringManager, TetheringCapability, TetheringOperationalState,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::application::system::RadioControl;
use crate::domain::system::{ActionOutcome, RadioKind, RadioState};

pub struct WinRtRadios;

impl WinRtRadios {
    pub fn new() -> Self {
        WinRtRadios
    }

    fn init() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn radios() -> Vec<Radio> {
        Self::init();
        Radio::GetRadiosAsync()
            .and_then(|op| op.join())
            .map(|list| list.into_iter().collect())
            .unwrap_or_default()
    }

    fn matches(kind: RadioKind, radio: &Radio) -> bool {
        use windows::Devices::Radios::RadioKind as WinKind;
        match (kind, radio.Kind()) {
            (RadioKind::Wifi, Ok(k)) => k == WinKind::WiFi,
            (RadioKind::Bluetooth, Ok(k)) => k == WinKind::Bluetooth,
            _ => false,
        }
    }

    fn tethering() -> Option<NetworkOperatorTetheringManager> {
        Self::init();
        let profile = NetworkInformation::GetInternetConnectionProfile().ok()?;
        if NetworkOperatorTetheringManager::GetTetheringCapabilityFromConnectionProfile(&profile).ok()?
            != TetheringCapability::Enabled
        {
            return None;
        }
        NetworkOperatorTetheringManager::CreateFromConnectionProfile(&profile).ok()
    }
}

impl Default for WinRtRadios {
    fn default() -> Self {
        Self::new()
    }
}

impl RadioControl for WinRtRadios {
    fn states(&mut self) -> Vec<RadioState> {
        let radios = Self::radios();
        let mut out = Vec::new();
        for kind in [RadioKind::Wifi, RadioKind::Bluetooth] {
            let radio = radios.iter().find(|r| Self::matches(kind, r));
            out.push(RadioState {
                kind,
                available: radio.is_some(),
                on: radio.and_then(|r| r.State().ok()) == Some(WinRadioState::On),
            });
        }
        let hotspot = Self::tethering();
        out.push(RadioState {
            kind: RadioKind::Hotspot,
            available: hotspot.is_some(),
            on: hotspot
                .and_then(|m| m.TetheringOperationalState().ok())
                .is_some_and(|s| s == TetheringOperationalState::On),
        });
        out
    }

    fn set(&mut self, kind: RadioKind, on: bool) -> Result<ActionOutcome, String> {
        let verb = if on { "activado" } else { "desactivado" };
        if kind == RadioKind::Hotspot {
            let manager = Self::tethering().ok_or("este equipo no puede compartir su conexión")?;
            let result = if on { manager.StartTetheringAsync() } else { manager.StopTetheringAsync() }
                .and_then(|op| op.join())
                .map_err(|e| e.to_string())?;
            return match result.Status() {
                Ok(s) if s.0 == 0 => Ok(ActionOutcome::done(format!("Compartir red {verb}"))),
                other => Err(format!("no se pudo cambiar compartir red: {other:?}")),
            };
        }
        let radio = Self::radios()
            .into_iter()
            .find(|r| Self::matches(kind, r))
            .ok_or_else(|| format!("este equipo no tiene {}", kind.label()))?;
        let access = Radio::RequestAccessAsync().and_then(|op| op.join()).map_err(|e| e.to_string())?;
        if access != RadioAccessStatus::Allowed {
            return Err(format!("Windows no permitió controlar {}: {access:?}", kind.label()));
        }
        let status = radio
            .SetStateAsync(if on { WinRadioState::On } else { WinRadioState::Off })
            .and_then(|op| op.join())
            .map_err(|e| e.to_string())?;
        if status == RadioAccessStatus::Allowed {
            Ok(ActionOutcome::done(format!("{} {verb}", kind.label())))
        } else {
            Err(format!("no se pudo cambiar {}: {status:?}", kind.label()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: nunca se cambia el estado de una radio en las pruebas.
    #[test]
    fn lee_el_estado_real_de_wifi_bluetooth_y_compartir_red() {
        let states = WinRtRadios::new().states();
        println!("conectividad: {states:?}");
        assert_eq!(states.len(), 3);
        assert_eq!(states[0].kind, RadioKind::Wifi);
        assert_eq!(states[1].kind, RadioKind::Bluetooth);
        assert_eq!(states[2].kind, RadioKind::Hotspot);
        assert!(states.iter().all(|s| s.available || !s.on), "algo no disponible no puede estar encendido");
    }
}
