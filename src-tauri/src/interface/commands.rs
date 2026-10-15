use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;

use crate::application::list_windows as use_case;
use crate::application::ports::SystemStatsProvider;
use crate::domain::actions::Action;
use crate::domain::snapshot::{StateSnapshot, SystemStats};
use crate::domain::rules::{ExclusionList, ExclusionRule};
use crate::domain::window::WindowInfo;
use crate::infrastructure::exclusion_store::ExclusionStore;

/// Estado compartido de la app: las exclusiones y, si está activo, el gestor de ventanas.
pub struct AppState {
    pub exclusions: Arc<ExclusionStore>,
    /// Último estado publicado por el gestor, para que una barra recién abierta lo pida.
    pub last_state: Mutex<Option<StateSnapshot>>,
    pub stats: Mutex<Box<dyn SystemStatsProvider + Send>>,
    #[cfg(windows)]
    pub manager: Mutex<Option<crate::infrastructure::windows::runtime::Runtime>>,
    #[cfg(not(windows))]
    pub manager: Mutex<Option<()>>,
}

impl AppState {
    pub fn new(exclusions: Arc<ExclusionStore>) -> Self {
        AppState {
            exclusions,
            last_state: Mutex::new(None),
            stats: Mutex::new(Box::new(crate::infrastructure::stats::SysinfoStats::new())),
            manager: Mutex::new(None),
        }
    }

    /// Avisa al gestor (si corre) de que la lista cambió para reevaluar las ventanas abiertas.
    fn publish(&self, list: &ExclusionList) {
        #[cfg(windows)]
        if let Some(rt) = self.manager.lock().unwrap().as_ref() {
            rt.send(crate::application::controller::Message::SetExclusions(list.clone()));
        }
        #[cfg(not(windows))]
        let _ = list;
    }
}

#[derive(Serialize)]
pub struct ExclusionDto {
    pub exe: String,
    pub title_contains: Option<String>,
}

fn dto(list: &ExclusionList) -> Vec<ExclusionDto> {
    list.rules
        .iter()
        .map(|ExclusionRule { exe, title_contains }| ExclusionDto { exe: exe.clone(), title_contains: title_contains.clone() })
        .collect()
}

#[tauri::command]
pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    #[cfg(windows)]
    {
        use crate::infrastructure::windows::enumerator::Win32WindowEnumerator;
        use_case::list_windows(&Win32WindowEnumerator)
    }
    #[cfg(not(windows))]
    {
        Err("solo disponible en Windows".into())
    }
}

#[tauri::command]
pub fn list_exclusions(state: State<'_, AppState>) -> Vec<ExclusionDto> {
    dto(&state.exclusions.list())
}

#[tauri::command]
pub fn add_exclusion(
    state: State<'_, AppState>,
    exe: String,
    title_contains: Option<String>,
) -> Result<Vec<ExclusionDto>, String> {
    let list = state.exclusions.add(&exe, title_contains.as_deref())?;
    state.publish(&list);
    Ok(dto(&list))
}

#[tauri::command]
pub fn remove_exclusion(
    state: State<'_, AppState>,
    exe: String,
    title_contains: Option<String>,
) -> Result<Vec<ExclusionDto>, String> {
    let list = state.exclusions.remove(&exe, title_contains.as_deref())?;
    state.publish(&list);
    Ok(dto(&list))
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> Option<StateSnapshot> {
    state.last_state.lock().unwrap().clone()
}

#[tauri::command]
pub fn system_stats(state: State<'_, AppState>) -> SystemStats {
    state.stats.lock().unwrap().read()
}

/// Ejecuta una acción de la sección 8 por su nombre (p. ej. `workspace_3`) desde la interfaz.
#[tauri::command]
pub fn run_action(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let action = Action::from_name(&name).ok_or_else(|| format!("acción desconocida: {name}"))?;
    #[cfg(windows)]
    {
        let guard = state.manager.lock().unwrap();
        let rt = guard.as_ref().ok_or("el gestor de ventanas no está activo")?;
        rt.send(crate::application::controller::Message::Action(action));
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (state, action);
        Err("solo disponible en Windows".into())
    }
}

#[cfg(windows)]
#[tauri::command]
pub fn open_panel(app: tauri::AppHandle, monitor: u32) {
    crate::interface::bar::toggle_panel(&app, monitor);
}

#[cfg(windows)]
#[tauri::command]
pub fn close_panel(app: tauri::AppHandle) {
    crate::interface::bar::hide_panel(&app);
}
