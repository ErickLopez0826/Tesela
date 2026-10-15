//! Acciones que pueden ejecutar los atajos (§8). Son datos puros: quien las ejecuta decide cómo.

use crate::domain::layout::{Dir, Orientation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // 8.1 / 8.2 foco y movimiento
    FocusDir(Dir),
    MoveDir(Dir),
    // 8.3 / 8.4 redimensionar. `percent` con signo: +2 agranda, -2 achica.
    Resize { orientation: Orientation, percent: i8 },
    EnterResizeMode,
    ExitResizeMode,
    // 8.5 workspaces (1 a 9)
    FocusWorkspace(u8),
    MoveToWorkspace(u8),
    NextWorkspace,
    PrevWorkspace,
    RecentWorkspace,
    MoveWorkspaceDir(Dir),
    // 8.6 ventana
    ToggleStack,
    ToggleFloating,
    ToggleFloatingCentered,
    ToggleFullscreen,
    ToggleMinimize,
    CloseWindow,
    CycleFocus,
    // 8.7 sistema
    OpenTerminal,
    TogglePause,
    /// Abre o cierra el panel desplegable (§9.2).
    TogglePanel,
    ReloadConfig,
    Redraw,
    Exit,
}

impl Action {
    /// Nombre estable usado en el archivo de configuración.
    pub fn name(&self) -> String {
        use Action::*;
        let dir = |d: &Dir| match d {
            Dir::Left => "left",
            Dir::Right => "right",
            Dir::Up => "up",
            Dir::Down => "down",
        };
        match self {
            FocusDir(d) => format!("focus_{}", dir(d)),
            MoveDir(d) => format!("move_{}", dir(d)),
            Resize { orientation, percent } => {
                let axis = if *orientation == Orientation::X { "width" } else { "height" };
                format!("resize_{axis}_{percent:+}")
            }
            EnterResizeMode => "resize_mode".into(),
            ExitResizeMode => "exit_resize_mode".into(),
            FocusWorkspace(n) => format!("workspace_{n}"),
            MoveToWorkspace(n) => format!("move_to_workspace_{n}"),
            NextWorkspace => "next_workspace".into(),
            PrevWorkspace => "prev_workspace".into(),
            RecentWorkspace => "recent_workspace".into(),
            MoveWorkspaceDir(d) => format!("move_workspace_{}", dir(d)),
            ToggleStack => "toggle_stack".into(),
            ToggleFloating => "toggle_floating".into(),
            ToggleFloatingCentered => "toggle_floating_centered".into(),
            ToggleFullscreen => "toggle_fullscreen".into(),
            ToggleMinimize => "toggle_minimize".into(),
            CloseWindow => "close_window".into(),
            CycleFocus => "cycle_focus".into(),
            OpenTerminal => "open_terminal".into(),
            TogglePause => "toggle_pause".into(),
            TogglePanel => "toggle_panel".into(),
            ReloadConfig => "reload_config".into(),
            Redraw => "redraw".into(),
            Exit => "exit".into(),
        }
    }

    /// Inversa de `name`.
    pub fn from_name(name: &str) -> Option<Action> {
        use Action::*;
        let dir = |s: &str| match s {
            "left" => Some(Dir::Left),
            "right" => Some(Dir::Right),
            "up" => Some(Dir::Up),
            "down" => Some(Dir::Down),
            _ => None,
        };
        if let Some(d) = name.strip_prefix("focus_").and_then(dir) {
            return Some(FocusDir(d));
        }
        if let Some(d) = name.strip_prefix("move_workspace_").and_then(dir) {
            return Some(MoveWorkspaceDir(d));
        }
        if let Some(n) = name.strip_prefix("move_to_workspace_") {
            return n.parse().ok().filter(|n| (1..=9).contains(n)).map(MoveToWorkspace);
        }
        if let Some(d) = name.strip_prefix("move_").and_then(dir) {
            return Some(MoveDir(d));
        }
        if let Some(rest) = name.strip_prefix("resize_width_") {
            return rest.parse().ok().map(|percent| Resize { orientation: Orientation::X, percent });
        }
        if let Some(rest) = name.strip_prefix("resize_height_") {
            return rest.parse().ok().map(|percent| Resize { orientation: Orientation::Y, percent });
        }
        if let Some(n) = name.strip_prefix("workspace_") {
            return n.parse().ok().filter(|n| (1..=9).contains(n)).map(FocusWorkspace);
        }
        Some(match name {
            "resize_mode" => EnterResizeMode,
            "exit_resize_mode" => ExitResizeMode,
            "next_workspace" => NextWorkspace,
            "prev_workspace" => PrevWorkspace,
            "recent_workspace" => RecentWorkspace,
            "toggle_stack" => ToggleStack,
            "toggle_floating" => ToggleFloating,
            "toggle_floating_centered" => ToggleFloatingCentered,
            "toggle_fullscreen" => ToggleFullscreen,
            "toggle_minimize" => ToggleMinimize,
            "close_window" => CloseWindow,
            "cycle_focus" => CycleFocus,
            "open_terminal" => OpenTerminal,
            "toggle_pause" => TogglePause,
            "toggle_panel" => TogglePanel,
            "reload_config" => ReloadConfig,
            "redraw" => Redraw,
            "exit" => Exit,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_ida_y_vuelta() {
        let all = [
            Action::FocusDir(Dir::Left),
            Action::MoveDir(Dir::Down),
            Action::Resize { orientation: Orientation::X, percent: -2 },
            Action::Resize { orientation: Orientation::Y, percent: 2 },
            Action::EnterResizeMode,
            Action::ExitResizeMode,
            Action::FocusWorkspace(9),
            Action::MoveToWorkspace(1),
            Action::NextWorkspace,
            Action::PrevWorkspace,
            Action::RecentWorkspace,
            Action::MoveWorkspaceDir(Dir::Up),
            Action::ToggleStack,
            Action::ToggleFloating,
            Action::ToggleFloatingCentered,
            Action::ToggleFullscreen,
            Action::ToggleMinimize,
            Action::CloseWindow,
            Action::CycleFocus,
            Action::OpenTerminal,
            Action::TogglePause,
            Action::TogglePanel,
            Action::ReloadConfig,
            Action::Redraw,
            Action::Exit,
        ];
        for a in all {
            assert_eq!(Action::from_name(&a.name()), Some(a), "{}", a.name());
        }
    }

    #[test]
    fn nombres_invalidos() {
        assert_eq!(Action::from_name("workspace_0"), None);
        assert_eq!(Action::from_name("workspace_10"), None);
        assert_eq!(Action::from_name("no_existe"), None);
    }
}
