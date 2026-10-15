//! Estado visible del escritorio que consume la interfaz (barra y panel). Datos puros y serializables.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonitorSnapshot {
    pub id: u32,
    pub primary: bool,
    pub vertical: bool,
    /// Número (1 a 9) del workspace que muestra este monitor.
    pub visible_workspace: Option<usize>,
    /// La barra se oculta si hay una ventana en pantalla completa en este monitor (§6.4).
    pub bar_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkspaceSnapshot {
    /// 1 a 9.
    pub number: usize,
    pub monitor: u32,
    pub visible: bool,
    /// Es el workspace del monitor con el foco.
    pub active: bool,
    pub window_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FocusedSnapshot {
    pub exe: String,
    pub title: String,
}

/// Uso del sistema que muestran la barra y el panel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub ram_used: u64,
    pub ram_total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct StateSnapshot {
    pub monitors: Vec<MonitorSnapshot>,
    pub workspaces: Vec<WorkspaceSnapshot>,
    pub focused: Option<FocusedSnapshot>,
    /// El gestor está en pausa (alt+shift+p o emergencia).
    pub paused: bool,
    /// Modo de teclado activo (p. ej. "resize").
    pub mode: Option<String>,
}

/// Estado de ejemplo (3 workspaces, una ventana enfocada) para las pruebas de extremo a extremo.
pub fn sample_state() -> StateSnapshot {
    StateSnapshot {
        monitors: vec![MonitorSnapshot { id: 1, primary: true, vertical: false, visible_workspace: Some(1), bar_visible: true }],
        workspaces: (1..=3)
            .map(|n| WorkspaceSnapshot { number: n, monitor: 1, visible: n == 1, active: n == 1, window_count: n })
            .collect(),
        focused: Some(FocusedSnapshot { exe: "ejemplo.exe".into(), title: "Ventana de ejemplo".into() }),
        paused: false,
        mode: None,
    }
}
