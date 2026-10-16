//! Volumen maestro del dispositivo de salida predeterminado (Core Audio).

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCommunications, eConsole, eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;

use crate::application::system::VolumeControl;
use crate::domain::system::{ActionOutcome, AudioDevice, VolumeState};

/// Interfaz no documentada, pero estable desde Windows Vista, que usa el propio panel de sonido de Windows para elegir
/// el dispositivo predeterminado. Solo se llama a `SetDefaultEndpoint`; el resto están para que la tabla de métodos
/// quede en su sitio.
#[allow(non_snake_case)]
mod policy {
    use std::ffi::c_void;

    use windows::core::{interface, IUnknown, IUnknown_Vtbl, HRESULT, PCWSTR};
    use windows::Win32::Media::Audio::ERole;

    #[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    pub unsafe trait IPolicyConfig: IUnknown {
        fn GetMixFormat(&self, device: PCWSTR, format: *mut c_void) -> HRESULT;
        fn GetDeviceFormat(&self, device: PCWSTR, default: i32, format: *mut c_void) -> HRESULT;
        fn ResetDeviceFormat(&self, device: PCWSTR) -> HRESULT;
        fn SetDeviceFormat(&self, device: PCWSTR, endpoint: *mut c_void, mix: *mut c_void) -> HRESULT;
        fn GetProcessingPeriod(&self, device: PCWSTR, default: i32, default_period: *mut i64, min_period: *mut i64) -> HRESULT;
        fn SetProcessingPeriod(&self, device: PCWSTR, period: *mut i64) -> HRESULT;
        fn GetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
        fn SetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
        fn GetPropertyValue(&self, device: PCWSTR, key: *const c_void, value: *mut c_void) -> HRESULT;
        fn SetPropertyValue(&self, device: PCWSTR, key: *const c_void, value: *const c_void) -> HRESULT;
        fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
        fn SetEndpointVisibility(&self, device: PCWSTR, visible: i32) -> HRESULT;
    }

    /// Pone `device` como predeterminado para un rol (consola, multimedia o comunicaciones).
    pub fn set_default_endpoint(policy: &IPolicyConfig, device: PCWSTR, role: ERole) -> windows::core::Result<()> {
        unsafe { policy.SetDefaultEndpoint(device, role).ok() }
    }
}
use policy::{set_default_endpoint, IPolicyConfig};

/// CLSID de `PolicyConfigClient`.
const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

pub struct CoreAudioVolume;

impl CoreAudioVolume {
    pub fn new() -> Self {
        CoreAudioVolume
    }

    /// El dispositivo predeterminado puede cambiar (auriculares, HDMI), así que se pide en cada uso.
    fn endpoint() -> Result<IAudioEndpointVolume, String> {
        unsafe {
            // S_FALSE si el hilo ya estaba inicializado; cualquier otro error se verá al crear el objeto.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| e.to_string())?;
            device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).map_err(|e| e.to_string())
        }
    }
}

impl CoreAudioVolume {
    fn enumerator() -> Result<IMMDeviceEnumerator, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())
        }
    }
}

impl Default for CoreAudioVolume {
    fn default() -> Self {
        Self::new()
    }
}

impl VolumeControl for CoreAudioVolume {
    fn get(&mut self) -> Result<VolumeState, String> {
        let ep = Self::endpoint()?;
        unsafe {
            let scalar = ep.GetMasterVolumeLevelScalar().map_err(|e| e.to_string())?;
            let muted = ep.GetMute().map_err(|e| e.to_string())?.as_bool();
            Ok(VolumeState { percent: (scalar * 100.0).clamp(0.0, 100.0), muted })
        }
    }

    fn set_percent(&mut self, percent: f32) -> Result<ActionOutcome, String> {
        let p = percent.clamp(0.0, 100.0);
        let ep = Self::endpoint()?;
        unsafe {
            ep.SetMasterVolumeLevelScalar(p / 100.0, std::ptr::null()).map_err(|e| e.to_string())?;
        }
        Ok(ActionOutcome::done(format!("Volumen al {}%", p.round())))
    }

    fn set_muted(&mut self, muted: bool) -> Result<ActionOutcome, String> {
        let ep = Self::endpoint()?;
        unsafe {
            ep.SetMute(muted, std::ptr::null()).map_err(|e| e.to_string())?;
        }
        Ok(ActionOutcome::done(if muted { "Silenciado" } else { "Sonido activado" }))
    }

    fn outputs(&mut self) -> Result<Vec<AudioDevice>, String> {
        let e = |e: windows::core::Error| e.to_string();
        let enumerator = Self::enumerator()?;
        unsafe {
            let default_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.GetId())
                .map(|id| {
                    let text = id.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(id.0 as *const _));
                    text
                })
                .unwrap_or_default();
            let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).map_err(e)?;
            let mut out = Vec::new();
            for i in 0..devices.GetCount().map_err(e)? {
                let device = devices.Item(i).map_err(e)?;
                let id_ptr = device.GetId().map_err(e)?;
                let id = id_ptr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(id_ptr.0 as *const _));
                let name = device
                    .OpenPropertyStore(STGM_READ)
                    .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
                    .and_then(|value| PropVariantToStringAlloc(&value))
                    .map(|p| {
                        let text = p.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(p.0 as *const _));
                        text
                    })
                    .unwrap_or_default();
                if !id.is_empty() {
                    out.push(AudioDevice { default: id == default_id, name: if name.is_empty() { id.clone() } else { name }, id });
                }
            }
            Ok(out)
        }
    }

    fn set_output(&mut self, id: &str) -> Result<ActionOutcome, String> {
        // Solo se acepta un dispositivo de la lista actual: nada de identificadores inventados.
        let devices = self.outputs()?;
        let target = devices.iter().find(|d| d.id == id).ok_or("ese dispositivo de audio ya no está disponible")?;
        let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let policy: IPolicyConfig = CoCreateInstance(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            for role in [eConsole, eMultimedia, eCommunications] {
                set_default_endpoint(&policy, PCWSTR(wide.as_ptr()), role).map_err(|e| e.to_string())?;
            }
        }
        Ok(ActionOutcome::done(format!("Salida de audio: {}", target.name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura y escrituras idempotentes (mismo valor): no cambia nada audible.
    #[test]
    fn lee_el_volumen_real_y_reescribir_el_mismo_valor_no_cambia_nada() {
        let mut v = CoreAudioVolume::new();
        let Ok(before) = v.get() else {
            println!("sin dispositivo de audio: se omite");
            return;
        };
        println!("volumen: {before:?}");
        assert!((0.0..=100.0).contains(&before.percent));
        v.set_percent(before.percent).unwrap();
        v.set_muted(before.muted).unwrap();
        let after = v.get().unwrap();
        assert_eq!(after.muted, before.muted);
        assert!((after.percent - before.percent).abs() < 1.0, "{before:?} -> {after:?}");
    }

    /// Lista las salidas reales y «cambia» a la que ya es la predeterminada: no cambia nada audible.
    #[test]
    fn lista_las_salidas_y_reasignar_la_actual_no_cambia_nada() {
        let mut v = CoreAudioVolume::new();
        let Ok(list) = v.outputs() else { return };
        println!("salidas: {list:?}");
        let Some(current) = list.iter().find(|d| d.default) else { return };
        v.set_output(&current.id).unwrap();
        let after = v.outputs().unwrap();
        assert!(after.iter().find(|d| d.default).is_some_and(|d| d.id == current.id));
        assert!(v.set_output("{inventado}").is_err());
    }
}
