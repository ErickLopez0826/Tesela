//! Regla pura: ¿se puede gestionar esta ventana?

/// Hechos observables de una ventana, recogidos por la infraestructura.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowFacts {
    pub handle: isize,
    pub exe: String,
    pub title: String,
    pub class: String,
    pub visible: bool,
    pub cloaked: bool,
    pub tool_window: bool,
    pub no_activate: bool,
    pub owned: bool,
    pub own_process: bool,
    pub minimized: bool,
    pub fullscreen: bool,
    /// Monitor (id de `Monitor`) donde está la ventana.
    pub monitor: Option<u32>,
}

/// Qué ventanas puede gestionar el gestor (ver DECISIONS.md D-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    All,
    TitlePrefix(String),
}

impl Scope {
    /// Formato de línea de comandos: `all` o `prefix:TEXTO`.
    pub fn parse(s: &str) -> Option<Scope> {
        if s == "all" {
            Some(Scope::All)
        } else {
            s.strip_prefix("prefix:")
                .filter(|p| !p.is_empty())
                .map(|p| Scope::TitlePrefix(p.to_string()))
        }
    }

    pub fn allows(&self, f: &WindowFacts) -> bool {
        match self {
            Scope::All => true,
            Scope::TitlePrefix(p) => f.title.starts_with(p.as_str()),
        }
    }
}

const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    // Ventanas nativas de la interfaz de Windows (avisos, buscador, entrada de texto): nunca son «aplicaciones».
    "Windows.UI.Core.CoreWindow",
    "ForegroundStaging",
];

/// Programas del propio Windows cuyas ventanas son superposiciones o avisos pasajeros, no ventanas de trabajo:
/// la herramienta de captura (Win+Shift+S) y su aviso, el menú de inicio, el buscador, el teclado táctil...
const SHELL_EXES: &[&str] = &[
    "screenclippinghost.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "searchapp.exe",
    "textinputhost.exe",
    "lockapp.exe",
    "shellhost.exe",
    "pickerhost.exe",
];

/// Una ventana es candidata a gestión si es una ventana normal de aplicación
/// y está dentro del alcance. Los diálogos (con propietario) no entran en el layout (§5.3).
pub fn is_manageable(f: &WindowFacts, scope: &Scope) -> bool {
    f.visible
        && !f.cloaked
        && !f.tool_window
        && !f.no_activate
        && !f.owned
        && !f.own_process
        && !f.title.trim().is_empty()
        && !SHELL_CLASSES.contains(&f.class.as_str())
        && !SHELL_EXES.contains(&f.exe.to_lowercase().as_str())
        && scope.allows(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal() -> WindowFacts {
        WindowFacts {
            handle: 1,
            exe: "app.exe".into(),
            title: "WMTEST 1".into(),
            class: "Form".into(),
            visible: true,
            ..Default::default()
        }
    }

    #[test]
    fn avisos_y_superposiciones_de_windows_no_se_gestionan() {
        // Aviso de la captura de pantalla (Win+Shift+S) y otras ventanas pasajeras de la interfaz de Windows.
        let toast = WindowFacts { class: "Windows.UI.Core.CoreWindow".into(), title: "Nueva notificación".into(), ..normal() };
        assert!(!is_manageable(&toast, &Scope::All));
        let clip = WindowFacts { exe: "ScreenClippingHost.exe".into(), title: "Captura".into(), ..normal() };
        assert!(!is_manageable(&clip, &Scope::All));
        let shell = WindowFacts { exe: "ShellExperienceHost.exe".into(), title: "Centro de notificaciones".into(), ..normal() };
        assert!(!is_manageable(&shell, &Scope::All));
        // Una aplicación normal con otro nombre de programa sigue siendo gestionable.
        assert!(is_manageable(&WindowFacts { exe: "SnippingTool.exe".into(), ..normal() }, &Scope::All));
    }

    #[test]
    fn ventana_normal_es_gestionable() {
        assert!(is_manageable(&normal(), &Scope::All));
    }

    #[test]
    fn filtros_basicos() {
        let n = normal();
        for bad in [
            WindowFacts { visible: false, ..n.clone() },
            WindowFacts { cloaked: true, ..n.clone() },
            WindowFacts { tool_window: true, ..n.clone() },
            WindowFacts { no_activate: true, ..n.clone() },
            WindowFacts { owned: true, ..n.clone() },
            WindowFacts { own_process: true, ..n.clone() },
            WindowFacts { title: " ".into(), ..n.clone() },
            WindowFacts { class: "Shell_TrayWnd".into(), ..n.clone() },
        ] {
            assert!(!is_manageable(&bad, &Scope::All));
        }
    }

    #[test]
    fn alcance_por_prefijo() {
        let scope = Scope::TitlePrefix("WMTEST".into());
        assert!(is_manageable(&normal(), &scope));
        assert!(!is_manageable(&WindowFacts { title: "Chrome".into(), ..normal() }, &scope));
    }

    #[test]
    fn parseo_de_alcance() {
        assert_eq!(Scope::parse("all"), Some(Scope::All));
        assert_eq!(Scope::parse("prefix:WMTEST"), Some(Scope::TitlePrefix("WMTEST".into())));
        assert_eq!(Scope::parse("prefix:"), None);
        assert_eq!(Scope::parse("otro"), None);
    }
}
