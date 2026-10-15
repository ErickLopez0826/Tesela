use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;

use crate::application::list_windows as use_case;
use crate::application::ports::SystemStatsProvider;
use crate::domain::actions::Action;
use crate::application::system::{list_wallpapers, resolve_wallpaper, SystemServices};
use crate::domain::snapshot::{StateSnapshot, SystemStats};
use crate::domain::system::{
    ActionOutcome, MediaCommand, MediaInfo, PowerAction, RadioKind, RadioState, SystemOverview, VolumeState,
};
use crate::domain::rules::{ExclusionList, ExclusionRule};
use crate::domain::window::WindowInfo;
use crate::infrastructure::exclusion_store::ExclusionStore;

/// Estado compartido de la app: las exclusiones y, si está activo, el gestor de ventanas.
pub struct AppState {
    pub exclusions: Arc<ExclusionStore>,
    /// Último estado publicado por el gestor, para que una barra recién abierta lo pida.
    pub last_state: Mutex<Option<StateSnapshot>>,
    pub stats: Mutex<Box<dyn SystemStatsProvider + Send>>,
    /// Módulos de sistema y de acción (medios, energía, conectividad, fondos, volumen, estadísticas).
    pub services: Arc<SystemServices>,
    /// `[system] allow_changes` de la configuración (modo seguro cuando es `false`).
    pub allow_changes: bool,
    pub wallpapers_dir: std::path::PathBuf,
    #[cfg(windows)]
    pub manager: Mutex<Option<crate::infrastructure::windows::runtime::Runtime>>,
    #[cfg(not(windows))]
    pub manager: Mutex<Option<()>>,
}

impl AppState {
    pub fn new(exclusions: Arc<ExclusionStore>, services: Arc<SystemServices>, allow_changes: bool) -> Self {
        AppState {
            exclusions,
            services,
            allow_changes,
            wallpapers_dir: crate::infrastructure::config_file::config_dir().join("wallpapers"),
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

// ----- Módulos de sistema y de acción (Fase 6) -----

fn lock_err<T>(e: std::sync::PoisonError<T>) -> String {
    format!("el módulo falló antes: {e}")
}

#[derive(Serialize)]
pub struct SystemMode {
    /// `false`: la energía y la conectividad solo se simulan.
    pub allow_changes: bool,
}

#[tauri::command]
pub fn system_mode(state: State<'_, AppState>) -> SystemMode {
    SystemMode { allow_changes: state.allow_changes }
}

#[tauri::command]
pub fn overview(state: State<'_, AppState>) -> Result<SystemOverview, String> {
    Ok(state.services.overview.lock().map_err(lock_err)?.read())
}

#[tauri::command]
pub fn media_now(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    state.services.media.lock().map_err(lock_err)?.now_playing()
}

#[tauri::command]
pub fn media_command(state: State<'_, AppState>, command: String) -> Result<ActionOutcome, String> {
    let cmd = MediaCommand::from_name(&command).ok_or_else(|| format!("orden de medios desconocida: {command}"))?;
    state.services.media.lock().map_err(lock_err)?.send(cmd)
}

#[tauri::command]
pub fn volume_get(state: State<'_, AppState>) -> Result<VolumeState, String> {
    state.services.volume.lock().map_err(lock_err)?.get()
}

#[tauri::command]
pub fn volume_set(state: State<'_, AppState>, percent: f32) -> Result<ActionOutcome, String> {
    state.services.volume.lock().map_err(lock_err)?.set_percent(percent)
}

#[tauri::command]
pub fn volume_mute(state: State<'_, AppState>, muted: bool) -> Result<ActionOutcome, String> {
    state.services.volume.lock().map_err(lock_err)?.set_muted(muted)
}

#[tauri::command]
pub fn radio_states(state: State<'_, AppState>) -> Result<Vec<RadioState>, String> {
    Ok(state.services.radios.lock().map_err(lock_err)?.states())
}

#[tauri::command]
pub fn radio_set(state: State<'_, AppState>, kind: String, on: bool) -> Result<ActionOutcome, String> {
    let kind = RadioKind::from_name(&kind).ok_or_else(|| format!("interruptor desconocido: {kind}"))?;
    state.services.radios.lock().map_err(lock_err)?.set(kind, on)
}

#[tauri::command]
pub fn power_run(state: State<'_, AppState>, action: String) -> Result<ActionOutcome, String> {
    let action = PowerAction::from_name(&action).ok_or_else(|| format!("acción de energía desconocida: {action}"))?;
    state.services.power.lock().map_err(lock_err)?.run(action)
}

#[derive(Serialize)]
pub struct WallpaperList {
    pub dir: String,
    pub images: Vec<String>,
    pub current: Option<String>,
}

#[tauri::command]
pub fn wallpapers_list(state: State<'_, AppState>) -> Result<WallpaperList, String> {
    let _ = std::fs::create_dir_all(&state.wallpapers_dir);
    let current = state.services.wallpaper.lock().map_err(lock_err)?.current().unwrap_or(None);
    Ok(WallpaperList {
        dir: state.wallpapers_dir.display().to_string(),
        images: list_wallpapers(&state.wallpapers_dir),
        current,
    })
}

#[tauri::command]
pub fn wallpaper_set(state: State<'_, AppState>, name: String) -> Result<ActionOutcome, String> {
    let path = resolve_wallpaper(&state.wallpapers_dir, &name)?;
    state.services.wallpaper.lock().map_err(lock_err)?.set(&path.to_string_lossy())
}
