//! Apps abiertas, traer una al frente y abrir programas (§9.3).

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use windows::core::{w, HSTRING};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, ShowWindow, SW_RESTORE, SW_SHOWNORMAL};

use crate::application::system::AppsControl;
use crate::domain::manageable::{is_manageable, Scope};
use crate::domain::utility::OpenApp;
use crate::infrastructure::windows::facts::{exe_path_of, facts_of, hwnd_from, top_level_windows, window_exists};
use crate::infrastructure::windows::platform::{focus_window, hidden_by_manager};

/// Pide al gestor de ventanas que enfoque una ventana (cambiando de workspace si hace falta).
/// Devuelve `true` si el gestor se hizo cargo; si no, el adaptador lo hace directamente.
pub type FocusSink = Arc<Mutex<Option<Box<dyn Fn(isize) -> bool + Send>>>>;

pub struct Win32Apps {
    focus_sink: FocusSink,
}

impl Win32Apps {
    pub fn new(focus_sink: FocusSink) -> Self {
        Win32Apps { focus_sink }
    }
}

impl AppsControl for Win32Apps {
    fn open_apps(&mut self) -> Result<Vec<OpenApp>, String> {
        let hidden: HashSet<isize> = hidden_by_manager();
        let mut apps = Vec::new();
        for id in top_level_windows() {
            let Some(f) = facts_of(id) else { continue };
            // Las ocultas por la propia app (otro workspace) también son apps abiertas.
            let shown = (f.visible && !f.cloaked) || hidden.contains(&id);
            let as_if_visible = crate::domain::manageable::WindowFacts { visible: shown, cloaked: false, ..f.clone() };
            if !is_manageable(&as_if_visible, &Scope::All) || f.minimized && !shown {
                continue;
            }
            let path = exe_path_of(id).unwrap_or_default();
            apps.push(OpenApp { handle: id, exe: f.exe, path, title: f.title });
        }
        apps.sort_by(|a, b| a.exe.to_lowercase().cmp(&b.exe.to_lowercase()).then(a.title.cmp(&b.title)));
        Ok(apps)
    }

    fn focus(&mut self, handle: isize) -> Result<(), String> {
        if !window_exists(handle) {
            return Err("la ventana ya no existe".into());
        }
        if let Some(sink) = self.focus_sink.lock().unwrap().as_ref() {
            if sink(handle) {
                return Ok(());
            }
        }
        unsafe {
            if IsIconic(hwnd_from(handle)).as_bool() {
                let _ = ShowWindow(hwnd_from(handle), SW_RESTORE);
            }
        }
        focus_window(handle);
        Ok(())
    }

    fn launch(&mut self, target: &str) -> Result<(), String> {
        let result = unsafe {
            ShellExecuteW(None, w!("open"), &HSTRING::from(target), None, None, SW_SHOWNORMAL)
        };
        // ShellExecute devuelve un valor > 32 si tuvo éxito.
        if (result.0 as isize) > 32 {
            Ok(())
        } else {
            Err(format!("Windows no pudo abrir {target} (código {})", result.0 as isize))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: lista las apps abiertas de verdad. No enfoca ni abre nada.
    #[test]
    fn lista_las_apps_abiertas_reales() {
        let apps = Win32Apps::new(Arc::default()).open_apps().unwrap();
        println!("apps abiertas: {}", apps.len());
        for a in apps.iter().take(8) {
            println!("  {} | {} | {}", a.exe, a.title, a.path);
        }
        assert!(!apps.is_empty());
        assert!(apps.iter().all(|a| !a.title.trim().is_empty() && !a.exe.is_empty()));
        assert!(apps.iter().any(|a| a.path.to_lowercase().ends_with(".exe")), "al menos una con ruta completa");
        let handles: HashSet<isize> = apps.iter().map(|a| a.handle).collect();
        assert_eq!(handles.len(), apps.len(), "sin duplicados");
    }

    #[test]
    fn enfocar_una_ventana_que_no_existe_da_error() {
        assert!(Win32Apps::new(Arc::default()).focus(0x7FFF_FFF0).is_err());
    }
}
