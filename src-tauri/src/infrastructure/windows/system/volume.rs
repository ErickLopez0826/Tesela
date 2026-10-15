//! Volumen maestro del dispositivo de salida predeterminado (Core Audio).

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

use crate::application::system::VolumeControl;
use crate::domain::system::{ActionOutcome, VolumeState};

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
}
