use serde::Serialize;

/// Ventana tal como la observa el sistema, antes de decidir si se lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowCandidate {
    pub handle: isize,
    pub exe: String,
    pub title: String,
    pub visible: bool,
    pub cloaked: bool,
    pub tool_window: bool,
    pub own_process: bool,
}

impl WindowCandidate {
    /// Una ventana se lista si es visible, no está oculta por DWM (cloak),
    /// tiene título, no es una ventana de herramientas y no es de la propia app.
    pub fn is_listable(&self) -> bool {
        self.visible
            && !self.cloaked
            && !self.tool_window
            && !self.own_process
            && !self.title.trim().is_empty()
    }
}

/// Ventana visible que se expone a la interfaz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowInfo {
    pub handle: isize,
    pub exe: String,
    pub title: String,
}

impl From<WindowCandidate> for WindowInfo {
    fn from(c: WindowCandidate) -> Self {
        WindowInfo { handle: c.handle, exe: c.exe, title: c.title }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> WindowCandidate {
        WindowCandidate {
            handle: 1,
            exe: "code.exe".into(),
            title: "Editor".into(),
            visible: true,
            cloaked: false,
            tool_window: false,
            own_process: false,
        }
    }

    #[test]
    fn ventana_normal_se_lista() {
        assert!(base().is_listable());
    }

    #[test]
    fn invisible_no_se_lista() {
        assert!(!WindowCandidate { visible: false, ..base() }.is_listable());
    }

    #[test]
    fn cloaked_no_se_lista() {
        assert!(!WindowCandidate { cloaked: true, ..base() }.is_listable());
    }

    #[test]
    fn sin_titulo_no_se_lista() {
        assert!(!WindowCandidate { title: "  ".into(), ..base() }.is_listable());
    }

    #[test]
    fn tool_window_no_se_lista() {
        assert!(!WindowCandidate { tool_window: true, ..base() }.is_listable());
    }

    #[test]
    fn ventana_propia_no_se_lista() {
        assert!(!WindowCandidate { own_process: true, ..base() }.is_listable());
    }

    #[test]
    fn conversion_conserva_datos() {
        let info: WindowInfo = base().into();
        assert_eq!(info, WindowInfo { handle: 1, exe: "code.exe".into(), title: "Editor".into() });
    }
}
