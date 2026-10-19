//! Sonido de una aplicación concreta (sesiones de audio de Windows): saber si suena y silenciarla sin tocar el volumen general.

use windows::core::Interface;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

/// Las sesiones de audio del dispositivo predeterminado que pertenecen a alguno de esos procesos.
fn sessions(pids: &[u32]) -> Vec<ISimpleAudioVolume> {
    let mut out = Vec::new();
    if pids.is_empty() {
        return out;
    }
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return out };
        let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else { return out };
        let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return out };
        let Ok(list) = manager.GetSessionEnumerator() else { return out };
        let count = list.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(control) = list.GetSession(i) else { continue };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            let Ok(pid) = control2.GetProcessId() else { continue };
            if !pids.contains(&pid) {
                continue;
            }
            if let Ok(volume) = control.cast::<ISimpleAudioVolume>() {
                out.push(volume);
            }
        }
    }
    out
}

/// `None` si la aplicación no tiene sesión de audio; si la tiene, si está silenciada (todas sus sesiones).
pub fn is_muted(pids: &[u32]) -> Option<bool> {
    let list = sessions(pids);
    if list.is_empty() {
        return None;
    }
    Some(list.iter().all(|v| unsafe { v.GetMute().map(|m| m.as_bool()).unwrap_or(false) }))
}

/// Silencia o devuelve el sonido a la aplicación. Devuelve cuántas sesiones cambió.
pub fn set_muted(pids: &[u32], muted: bool) -> usize {
    sessions(pids).iter().filter(|v| unsafe { v.SetMute(muted, std::ptr::null()).is_ok() }).count()
}
