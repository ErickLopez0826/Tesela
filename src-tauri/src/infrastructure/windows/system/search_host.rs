//! Adaptador del buscador (§18) a Windows: aplicaciones instaladas y abiertas, enfocar, lanzar (con o sin elevación) y
//! abrir direcciones web.

use crate::application::search::SearchHost;
use crate::application::system::AppsControl;
use crate::domain::startup::AppSuggestion;
use crate::domain::utility::OpenApp;
use crate::infrastructure::windows::system::apps::{launch_elevated, open_http, FocusSink, Win32Apps};
use crate::infrastructure::windows::system::installed_apps;

pub struct Win32SearchHost {
    apps: Win32Apps,
}

impl Win32SearchHost {
    /// Con el mismo enlace de enfoque que los módulos: enfocar una ventana cambia de workspace si hace falta.
    pub fn new(focus_sink: FocusSink) -> Self {
        Win32SearchHost { apps: Win32Apps::new(focus_sink) }
    }
}

impl SearchHost for Win32SearchHost {
    fn apps(&mut self) -> Vec<AppSuggestion> {
        installed_apps::launcher_suggestions()
    }

    fn open_apps(&mut self) -> Vec<OpenApp> {
        self.apps.open_apps().unwrap_or_default()
    }

    fn focus(&mut self, handle: isize) -> Result<(), String> {
        self.apps.focus(handle)
    }

    fn launch(&mut self, path: &str, as_admin: bool) -> Result<(), String> {
        // Una app empaquetada no se puede elevar desde fuera (Windows no lo permite con `runas`): se abre normal.
        if as_admin && !crate::domain::packaged::is_launch_path(path) {
            launch_elevated(path)
        } else {
            self.apps.launch(path)
        }
    }

    fn open_url(&mut self, url: &str) -> Result<(), String> {
        open_http(url)
    }
}
