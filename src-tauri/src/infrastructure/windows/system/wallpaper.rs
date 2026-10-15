//! Fondos de pantalla (§9.2): cambio directo con la transición del sistema, mediante `IDesktopWallpaper`
//! (la misma interfaz que usa la página Personalización de Windows, que mezcla el fondo nuevo con el anterior).

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper, DWPOS_FILL};

use crate::application::system::WallpaperControl;
use crate::domain::system::ActionOutcome;

pub struct ShellWallpaper;

impl ShellWallpaper {
    pub fn new() -> Self {
        ShellWallpaper
    }

    fn interface() -> Result<IDesktopWallpaper, String> {
        unsafe {
            // IDesktopWallpaper es una interfaz de shell: se usa desde un hilo STA.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).map_err(|e| e.to_string())
        }
    }

    /// Ejecuta `f` en un hilo nuevo (STA limpio): los hilos del pool de comandos pueden venir ya
    /// inicializados en modo multihilo por otros módulos.
    fn on_sta_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
        std::thread::Builder::new()
            .name("wm-wallpaper".into())
            .spawn(f)
            .map_err(|e| e.to_string())?
            .join()
            .map_err(|_| "el hilo de fondos falló".to_string())
    }
}

impl Default for ShellWallpaper {
    fn default() -> Self {
        Self::new()
    }
}

impl WallpaperControl for ShellWallpaper {
    fn current(&mut self) -> Result<Option<String>, String> {
        Self::on_sta_thread(Self::current_inner)?
    }

    fn set(&mut self, path: &str) -> Result<ActionOutcome, String> {
        let path = path.to_string();
        Self::on_sta_thread(move || Self::set_inner(&path))?
    }
}

impl ShellWallpaper {
    fn current_inner() -> Result<Option<String>, String> {
        let dw = Self::interface()?;
        unsafe {
            let count = dw.GetMonitorDevicePathCount().map_err(|e| e.to_string())?;
            for i in 0..count {
                let monitor = dw.GetMonitorDevicePathAt(i).map_err(|e| e.to_string())?;
                let path = dw.GetWallpaper(PCWSTR(monitor.0));
                CoTaskMemFree(Some(monitor.0 as *const _));
                if let Ok(p) = path {
                    let text = p.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(p.0 as *const _));
                    if !text.is_empty() {
                        return Ok(Some(text));
                    }
                }
            }
        }
        Ok(None)
    }

    fn set_inner(path: &str) -> Result<ActionOutcome, String> {
        if !std::path::Path::new(path).is_file() {
            return Err(format!("no existe la imagen: {path}"));
        }
        let dw = Self::interface()?;
        unsafe {
            dw.SetPosition(DWPOS_FILL).map_err(|e| e.to_string())?;
            // Sin monitor concreto: todos los monitores.
            dw.SetWallpaper(PCWSTR::null(), &HSTRING::from(path)).map_err(|e| e.to_string())?;
        }
        Ok(ActionOutcome::done(format!("Fondo cambiado a {path}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: no se cambia el fondo real en las pruebas.
    #[test]
    fn lee_el_fondo_actual_del_sistema() {
        let mut w = ShellWallpaper::new();
        let current = w.current().expect("IDesktopWallpaper responde");
        println!("fondo actual: {current:?}");
        if let Some(p) = current {
            assert!(!p.is_empty());
        }
    }

    #[test]
    fn rechaza_una_imagen_que_no_existe_sin_tocar_nada() {
        let mut w = ShellWallpaper::new();
        assert!(w.set(r"C:\no\existe\nada.png").is_err());
    }
}
