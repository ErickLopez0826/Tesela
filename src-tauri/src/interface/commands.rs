use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;

use crate::application::list_windows as use_case;
use crate::application::ports::SystemStatsProvider;
use crate::domain::battery::BatteryState;
use crate::infrastructure::claude_store;
use crate::domain::actions::Action;
use crate::application::system::{list_wallpapers, resolve_wallpaper, SystemServices};
use crate::application::modules::{ModuleApi, StateSource};
use crate::domain::modules::Permission;
use crate::domain::snapshot::{StateSnapshot, SystemStats};
use crate::infrastructure::module_store::{InstallPreview, InstalledModule, ModuleStore};
use crate::domain::system::{
    ActionOutcome, AudioDevice, BluetoothDevice, EngineLibrary, HotspotInfo, MediaCommand, MediaInfo, PowerAction, RadioKind, RadioState,
    SystemOverview, VolumeState, WifiNetwork,
};
use crate::domain::rules::{ExclusionList, ExclusionRule};
use crate::domain::window::WindowInfo;
use crate::infrastructure::exclusion_store::ExclusionStore;

/// Cómo está arrancada la app.
#[derive(Default, Clone, Copy)]
pub struct GeneralInfo {
    pub managing: bool,
    pub bar: bool,
}

/// Configuración de la prueba de extremo a extremo de un módulo.
pub struct SelfTest {
    pub module: String,
    pub out: std::path::PathBuf,
}

/// Último estado del escritorio publicado por el gestor; lo leen la barra, el panel y los módulos.
#[derive(Default)]
pub struct LastState(pub Mutex<Option<StateSnapshot>>);

impl LastState {
    pub fn set(&self, state: StateSnapshot) -> Option<StateSnapshot> {
        self.0.lock().unwrap().replace(state)
    }
}

impl StateSource for LastState {
    fn snapshot(&self) -> Option<StateSnapshot> {
        self.0.lock().unwrap().clone()
    }
}

/// Estado compartido de la app: las exclusiones y, si está activo, el gestor de ventanas.
pub struct AppState {
    pub exclusions: Arc<ExclusionStore>,
    /// Último estado publicado por el gestor, para que una barra recién abierta lo pida.
    pub last_state: Arc<LastState>,
    /// Módulos instalables (§10): almacén en disco y API que usan.
    pub modules: Arc<ModuleStore>,
    pub module_api: Arc<ModuleApi>,
    /// Solo con `--selftest-module`.
    pub selftest: Option<SelfTest>,
    /// Con qué opciones arrancó esta instancia (para la pestaña General).
    pub general: GeneralInfo,
    pub stats: Mutex<Box<dyn SystemStatsProvider + Send>>,
    /// Módulos de sistema y de acción (medios, energía, conectividad, fondos, volumen, estadísticas).
    pub services: Arc<SystemServices>,
    /// `[system] allow_changes` de la configuración (modo seguro cuando es `false`).
    pub allow_changes: bool,
    pub wallpapers_dir: std::path::PathBuf,
    pub notes: crate::infrastructure::notes_store::NotesStore,
    /// `[system] wallpaper_transition`: dibujar la transición al cambiar de fondo.
    pub wallpaper_transition: bool,
    /// Buscador de aplicaciones (§18): redirecciones, historial de uso y servicio de búsqueda.
    #[cfg(windows)]
    pub search: Arc<crate::interface::search::SearchState>,
    #[cfg(windows)]
    pub manager: Mutex<Option<crate::infrastructure::windows::runtime::Runtime>>,
    #[cfg(not(windows))]
    pub manager: Mutex<Option<()>>,
}

impl AppState {
    pub fn new(
        exclusions: Arc<ExclusionStore>,
        services: Arc<SystemServices>,
        allow_changes: bool,
        modules: Arc<ModuleStore>,
        module_api: Arc<ModuleApi>,
        last_state: Arc<LastState>,
    ) -> Self {
        AppState {
            exclusions,
            modules,
            module_api,
            selftest: None,
            general: GeneralInfo::default(),
            services,
            allow_changes,
            wallpapers_dir: crate::infrastructure::config_file::config_dir().join("wallpapers"),
            wallpaper_transition: true,
            #[cfg(windows)]
            search: Arc::new(crate::interface::search::SearchState::default()),
            notes: crate::infrastructure::notes_store::NotesStore::load(crate::infrastructure::notes_store::NotesStore::default_path()),
            last_state,
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
    state.last_state.snapshot()
}

#[tauri::command(async)]
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

/// Ajusta el alto del panel al de la pestaña visible (alto lógico); la parte superior se queda donde está.
#[cfg(windows)]
#[tauri::command]
pub fn panel_resize(app: tauri::AppHandle, height: f64) {
    crate::interface::bar::resize_panel(&app, height);
}

#[cfg(windows)]
#[tauri::command]
pub fn close_panel(app: tauri::AppHandle) {
    crate::interface::bar::hide_panel(&app);
}

/// Imagen de lo que hay detrás de la barra o del panel, ya desenfocada con la intensidad (0 a 100) pedida: la vista la
/// dibuja bajo su tinte. `None` si no se pudo copiar (entonces se queda solo el tinte). Con `refresh` se vuelve a copiar
/// el fondo de pantalla de la barra o del panel (cambió el fondo).
#[cfg(windows)]
#[tauri::command]
pub async fn glass_image(
    window: tauri::WebviewWindow,
    kind: String,
    blur: f32,
    refresh: bool,
) -> Result<Option<crate::infrastructure::windows::backdrop::GlassImage>, String> {
    use crate::domain::geometry::Rect;
    use crate::infrastructure::windows::backdrop;
    let scale = window.scale_factor().unwrap_or(1.0);
    match kind.as_str() {
        "bar" => {
            let pos = window.outer_position().map_err(|e| e.to_string())?;
            let size = window.outer_size().map_err(|e| e.to_string())?;
            let own = Rect::new(pos.x, pos.y, size.width as i32, size.height as i32);
            // Las barras de tareas copian siempre la misma zona (el centro de abajo del monitor, del tamaño máximo que puede
            // tener la isla), no la de su ventana: así crecer o encogerse no obliga a copiar de nuevo y el desenfoque no
            // desaparece mientras tanto. La vista coloca la imagen según el tamaño que tenga la isla en cada momento.
            let rect = if window.label().starts_with("taskbar-") {
                crate::interface::taskbar::glass_area(own)
            } else {
                own
            };
            let label = window.label().to_string();
            blocking(move || Ok(backdrop::bar_image(&label, rect, blur, scale, refresh))).await
        }
        "panel" => blocking(move || Ok(backdrop::panel_image(blur, scale, refresh))).await,
        "search" => blocking(move || Ok(backdrop::search_image(blur, scale, refresh))).await,
        "popup" => blocking(move || Ok(backdrop::popup_image(blur, scale, refresh))).await,
        other => Err(format!("fondo desconocido: {other}")),
    }
}

// ----- Barra de tareas propia y tecla Windows -----

/// Lo que Tesela reemplaza de Windows (barra de tareas y tecla Windows).
#[cfg(windows)]
#[tauri::command]
pub fn shell_get() -> crate::domain::shell::ShellSettings {
    crate::interface::shell::get()
}

#[cfg(windows)]
#[tauri::command]
pub fn shell_set(app: tauri::AppHandle, settings: crate::domain::shell::ShellSettings) -> Result<crate::domain::shell::ShellSettings, String> {
    crate::interface::shell::set(&app, settings)
}

/// La vista de la barra de tareas avisa del ancho (lógico) que necesita su contenido.
#[cfg(windows)]
#[tauri::command]
pub fn taskbar_resize(app: tauri::AppHandle, window: tauri::WebviewWindow, width: f64, height: f64) {
    if let Some(monitor) = crate::interface::taskbar::monitor_of_label(window.label()) {
        crate::interface::taskbar::resize(&app, monitor, width, height);
    }
}

/// La barra de tareas se cierra a sí misma (al tocar una app).
#[cfg(windows)]
#[tauri::command]
pub fn taskbar_hide(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    if let Some(monitor) = crate::interface::taskbar::monitor_of_label(window.label()) {
        crate::interface::taskbar::hide(&app, monitor);
    }
}

/// Aplicaciones en segundo plano (con icono en la bandeja y en marcha) con lo que consumen.
#[cfg(windows)]
#[tauri::command]
pub async fn background_apps() -> Result<Vec<crate::infrastructure::windows::system::background::BackgroundApp>, String> {
    blocking(|| Ok(crate::infrastructure::windows::system::background::background_apps())).await
}

/// Icono de un ejecutable en marcha (para la sección de segundo plano).
#[cfg(windows)]
#[tauri::command]
pub async fn exe_icon(path: String) -> Result<Option<String>, String> {
    use crate::infrastructure::windows::system::background::{background_apps, known_path};
    // Solo se da el icono de aplicaciones que el propio núcleo listó como en segundo plano.
    blocking(move || Ok(known_path(&background_apps(), &path).and_then(|p| crate::infrastructure::windows::icons::exe_icon_url(&p)))).await
}

/// La aplicación en segundo plano con ese ejecutable. La interfaz nunca manda rutas: se busca aquí entre las que el núcleo
/// listó, así ningún comando lanza un programa que no esté en esa lista.
#[cfg(windows)]
fn background_app(exe: &str) -> Result<crate::infrastructure::windows::system::background::BackgroundApp, String> {
    use crate::infrastructure::windows::system::background::{background_apps, find_by_exe};
    find_by_exe(&background_apps(), exe).ok_or_else(|| format!("«{exe}» no es una aplicación en segundo plano"))
}

/// Muestra y enfoca la ventana de la aplicación (aunque esté oculta); si no tiene, vuelve a lanzar su ejecutable (las apps de
/// instancia única se limitan a mostrar la suya).
#[cfg(windows)]
fn open_app(app: &crate::infrastructure::windows::system::background::BackgroundApp) -> Result<(), String> {
    use crate::infrastructure::windows::facts::{facts_of, hwnd_from, top_level_windows};
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE, SW_SHOW};
    let stem = app.exe.to_lowercase();
    let stem = stem.trim_end_matches(".exe");
    let window = top_level_windows()
        .into_iter()
        .filter_map(facts_of)
        .find(|f| f.exe.to_lowercase().trim_end_matches(".exe") == stem && !f.title.trim().is_empty() && !f.tool_window);
    if let Some(f) = window {
        unsafe {
            let _ = ShowWindow(hwnd_from(f.handle), if f.visible { SW_RESTORE } else { SW_SHOW });
        }
        crate::infrastructure::windows::platform::focus_window(f.handle);
        return Ok(());
    }
    std::process::Command::new(&app.path).spawn().map(|_| ()).map_err(|e| format!("no se pudo abrir {}: {e}", app.exe))
}

/// Abre una aplicación en segundo plano.
#[cfg(windows)]
#[tauri::command]
pub async fn background_open(exe: String) -> Result<(), String> {
    blocking(move || open_app(&background_app(&exe)?)).await
}

/// Una acción que se puede hacer con una aplicación en segundo plano.
#[cfg(windows)]
#[derive(Serialize)]
pub struct AppActionInfo {
    pub id: String,
    pub label: String,
    /// Pide confirmación y se pinta como peligrosa (cerrar la aplicación).
    pub danger: bool,
    /// Si es un interruptor, su estado actual (silenciada o no).
    pub on: Option<bool>,
}

#[cfg(windows)]
fn user_actions_path() -> std::path::PathBuf {
    crate::infrastructure::config_file::config_dir().join("app-actions.json")
}

/// Lo que se puede hacer con esa aplicación, según lo que sea: abrir y cerrar, silenciar su sonido si tiene, y las acciones
/// propias de su perfil (Discord, Wallpaper Engine, Spotify, Steam o las del usuario en `app-actions.json`).
#[cfg(windows)]
#[tauri::command]
pub async fn app_actions(exe: String) -> Result<Vec<AppActionInfo>, String> {
    blocking(move || {
        use crate::infrastructure::windows::system::{app_actions as actions, app_audio, background};
        let app = background_app(&exe)?;
        let mut list = vec![AppActionInfo { id: "open".into(), label: "Abrir".into(), danger: false, on: None }];
        for d in actions::profile(&app.exe, &actions::load_user(&user_actions_path())) {
            list.push(AppActionInfo { id: d.id, label: d.label, danger: false, on: None });
        }
        if let Some(muted) = app_audio::is_muted(&background::pids_for(&app.path)) {
            list.push(AppActionInfo { id: "mute".into(), label: if muted { "Activar sonido".into() } else { "Silenciar app".into() }, danger: false, on: Some(muted) });
        }
        list.push(AppActionInfo { id: "quit".into(), label: "Cerrar".into(), danger: true, on: None });
        Ok(list)
    })
    .await
}

/// Hace una de esas acciones. Todo se resuelve aquí: la interfaz solo dice qué aplicación y qué acción.
#[cfg(windows)]
#[tauri::command]
pub async fn app_action_run(exe: String, id: String) -> Result<(), String> {
    blocking(move || {
        use crate::infrastructure::windows::system::{app_actions as actions, app_audio, background};
        let app = background_app(&exe)?;
        match id.as_str() {
            "open" => open_app(&app),
            "quit" => {
                background::kill_all(&app.path);
                Ok(())
            }
            "mute" => {
                let pids = background::pids_for(&app.path);
                let muted = app_audio::is_muted(&pids).ok_or("la aplicación no tiene sonido")?;
                app_audio::set_muted(&pids, !muted);
                Ok(())
            }
            other => {
                let def = actions::profile(&app.exe, &actions::load_user(&user_actions_path()))
                    .into_iter()
                    .find(|d| d.id == other)
                    .ok_or_else(|| format!("acción desconocida: {other}"))?;
                match def.effect {
                    actions::Effect::Keys(k) => {
                        let keys = actions::parse_keys(&k).ok_or_else(|| format!("atajo no válido: {k}"))?;
                        actions::send_chord(&keys);
                        Ok(())
                    }
                    // Solo con el propio ejecutable de la aplicación (su ruta sale de la lista, no de la interfaz).
                    actions::Effect::Exec(args) => std::process::Command::new(&app.path).args(args).spawn().map(|_| ()).map_err(|e| e.to_string()),
                    actions::Effect::Uri(u) => {
                        if !actions::uri_allowed(&u) {
                            return Err(format!("enlace no permitido: {u}"));
                        }
                        actions::open_uri(&u)
                    }
                }
            }
        }
    })
    .await
}

/// Abre la ventana emergente de una aplicación en segundo plano junto a su botón «⋯». La barra manda el rectángulo del botón
/// (px lógicos de cliente de su ventana); el núcleo lo pasa a pantalla.
#[cfg(windows)]
#[tauri::command]
pub fn popup_open(app: tauri::AppHandle, window: tauri::WebviewWindow, exe: String, name: String, path: String, x: f64, y: f64, w: f64, h: f64, hover: Option<bool>) -> Result<(), String> {
    use crate::domain::geometry::Rect;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let anchor = Rect::new(pos.x + (x * scale).round() as i32, pos.y + (y * scale).round() as i32, (w * scale).round() as i32, (h * scale).round() as i32);
    crate::interface::popup::open(&app, anchor, &exe, &name, &path, hover.unwrap_or(false));
    Ok(())
}

#[cfg(windows)]
#[tauri::command]
pub fn popup_close(app: tauri::AppHandle) {
    crate::interface::popup::close(&app);
}

/// La ventana emergente pide el tamaño (lógico) que necesita su contenido.
#[cfg(windows)]
#[tauri::command]
pub fn popup_resize(app: tauri::AppHandle, width: f64, height: f64) {
    crate::interface::popup::resize(&app, width, height);
}

// ----- Buscador de aplicaciones (§18) -----
//
// La interfaz nunca manda rutas ni direcciones: manda el identificador de un resultado que el propio núcleo acaba de
// ofrecer (§18.10). Si no lo reconoce, no se hace nada.

/// Busca mientras se escribe. Con el cuadro vacío devuelve las aplicaciones más usadas.
#[cfg(windows)]
#[tauri::command]
pub async fn search_query(state: State<'_, AppState>, text: String) -> Result<Vec<crate::domain::search::SearchItem>, String> {
    let search = state.search.clone();
    blocking(move || Ok(search.query(&text))).await
}

/// Abre un resultado (Enter) o lo abre como administrador (ctrl+Enter) y cierra el buscador.
#[cfg(windows)]
#[tauri::command]
pub async fn search_open(app: tauri::AppHandle, state: State<'_, AppState>, id: String, as_admin: bool) -> Result<(), String> {
    let search = state.search.clone();
    let opened = blocking(move || search.open(&id, as_admin)).await?;
    if opened {
        // Lo abierto toma el foco: no se le devuelve a la ventana anterior.
        crate::interface::search::close(&app, false);
    }
    Ok(())
}

/// Cierra el buscador (Escape) devolviendo el foco a la ventana que lo tenía.
#[cfg(windows)]
#[tauri::command]
pub fn search_close(app: tauri::AppHandle) {
    crate::interface::search::close(&app, true);
}

/// La vista pide el alto (lógico) que necesita su contenido.
#[cfg(windows)]
#[tauri::command]
pub fn search_resize(app: tauri::AppHandle, height: f64) {
    crate::interface::search::resize(&app, height);
}

/// Icono (PNG en una URL de datos) de un resultado que es un programa.
#[cfg(windows)]
#[tauri::command]
pub async fn search_icon(state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    let search = state.search.clone();
    blocking(move || Ok(search.icon(&id))).await
}

#[cfg(windows)]
#[tauri::command]
pub fn search_redirects_get(state: State<'_, AppState>) -> Vec<crate::domain::search::Redirect> {
    state.search.redirects()
}

/// Guarda la lista completa de redirecciones y búsquedas web; devuelve la lista ya validada.
#[cfg(windows)]
#[tauri::command]
pub fn search_redirects_set(state: State<'_, AppState>, list: Vec<crate::domain::search::Redirect>) -> Result<Vec<crate::domain::search::Redirect>, String> {
    state.search.set_redirects(list)
}

/// Borra el historial de uso del buscador. Devuelve cuántas entradas quedan (cero).
#[cfg(windows)]
#[tauri::command]
pub fn search_history_clear(state: State<'_, AppState>) -> Result<usize, String> {
    state.search.clear_history()?;
    Ok(state.search.history_len())
}

/// Cuántas cosas recuerda el historial de uso del buscador.
#[cfg(windows)]
#[tauri::command]
pub fn search_history_len(state: State<'_, AppState>) -> usize {
    state.search.history_len()
}

/// Respaldo de la tecla Windows desde el panel (ver `winkey::page_key`).
#[cfg(windows)]
#[tauri::command]
pub fn winkey_page_key(kind: String) {
    crate::infrastructure::windows::winkey::page_key(&kind);
}

/// El panel avisa de que recibió un clic o el foco: se refresca el gancho de la tecla Windows (ver `keep_hook_fresh`).
#[cfg(windows)]
#[tauri::command]
pub fn winkey_refresh() {
    crate::interface::shell::keep_hook_fresh();
    crate::infrastructure::windows::winkey::reinstall();
}

/// Enfoca una ventana desde la barra de tareas (cambia de workspace y la restaura si hace falta).
#[cfg(windows)]
#[tauri::command]
pub fn focus_window(state: State<'_, AppState>, handle: isize) {
    if let Some(rt) = state.manager.lock().unwrap().as_ref() {
        rt.send(crate::application::controller::Message::FocusWindow(handle));
    }
}

/// Icono (PNG en una URL de datos) de la aplicación de una ventana. `key` agrupa las ventanas con el mismo icono.
#[cfg(windows)]
#[tauri::command]
pub async fn app_icon(handle: isize, key: String) -> Result<Option<String>, String> {
    blocking(move || Ok(crate::infrastructure::windows::icons::app_icon_url(handle, &key))).await
}

// ----- Módulos de sistema y de acción (Fase 6) -----

/// Ejecuta algo que puede bloquearse (COM/WinRT, procesos, disco) en el grupo de hilos «bloqueantes» de Tauri.
/// Con `#[tauri::command(async)]` una función síncrona correría sobre los hilos del ejecutor async, que son pocos:
/// unas cuantas llamadas atascadas (p. ej. el servicio de medios de Windows sin responder) los agotarían y
/// ningún otro comando (volumen, conectividad, energía…) llegaría a ejecutarse.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| format!("la tarea falló: {e}"))?
}

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
pub async fn overview(state: State<'_, AppState>) -> Result<SystemOverview, String> {
    let s = state.services.clone();
    blocking(move || Ok(s.overview.lock().map_err(lock_err)?.read())).await
}

#[tauri::command]
pub async fn media_now(state: State<'_, AppState>) -> Result<Option<MediaInfo>, String> {
    let s = state.services.clone();
    blocking(move || s.media.lock().map_err(lock_err)?.now_playing()).await
}

/// Portada o miniatura de lo que suena (data URL). La interfaz la pide solo cuando cambia la canción.
#[tauri::command]
pub async fn media_thumbnail(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let s = state.services.clone();
    blocking(move || s.media.lock().map_err(lock_err)?.thumbnail()).await
}

#[tauri::command]
pub async fn media_command(state: State<'_, AppState>, command: String) -> Result<ActionOutcome, String> {
    let cmd = MediaCommand::from_name(&command).ok_or_else(|| format!("orden de medios desconocida: {command}"))?;
    let s = state.services.clone();
    blocking(move || s.media.lock().map_err(lock_err)?.send(cmd)).await
}

#[tauri::command]
pub async fn volume_get(state: State<'_, AppState>) -> Result<VolumeState, String> {
    let s = state.services.clone();
    blocking(move || s.volume.lock().map_err(lock_err)?.get()).await
}

#[tauri::command]
pub async fn volume_set(state: State<'_, AppState>, percent: f32) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    blocking(move || s.volume.lock().map_err(lock_err)?.set_percent(percent)).await
}

#[tauri::command]
pub async fn volume_mute(state: State<'_, AppState>, muted: bool) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    blocking(move || s.volume.lock().map_err(lock_err)?.set_muted(muted)).await
}

/// Batería del equipo; `None` en un equipo de escritorio (la interfaz entonces no muestra nada).
#[tauri::command]
pub async fn battery_get() -> Result<Option<BatteryState>, String> {
    blocking(|| Ok(crate::infrastructure::windows::system::battery::read())).await
}

/// Qué hace Claude Code ahora (trabajando, esperando o parado), según los avisos de sus hooks.
#[tauri::command]
pub async fn claude_activity_get() -> Result<claude_store::ActivityView, String> {
    blocking(|| Ok(claude_store::read_activity(&claude_store::Paths::from_env(), claude_store::now_secs()))).await
}

/// Uso de la suscripción de Claude (5 h y semanal). Solo tiene datos si la persona activó la función en Ajustes.
#[tauri::command]
pub async fn claude_usage_get() -> Result<crate::application::claude_usage::UsageView, String> {
    blocking(|| Ok(crate::infrastructure::windows::system::claude_usage::service().view())).await
}

/// Activa o apaga la lectura del uso. Al activar se lee enseguida; al apagar se olvida lo leído.
#[tauri::command]
pub async fn claude_usage_set_enabled(enabled: bool) -> Result<crate::application::claude_usage::UsageView, String> {
    use crate::infrastructure::windows::system::claude_usage as live;
    blocking(move || {
        let paths = claude_store::Paths::from_env();
        live::write_enabled(&paths, enabled)?;
        let svc = live::service();
        svc.set_enabled(enabled);
        if enabled {
            let now = claude_store::now_secs();
            svc.attempt(&mut live::LivePort { paths }, now, now * 1000, false);
        }
        Ok(svc.view())
    })
    .await
}

/// Pide una lectura ahora (respeta la espera mínima y la que pida el servidor).
#[tauri::command]
pub async fn claude_usage_refresh() -> Result<crate::application::claude_usage::UsageView, String> {
    use crate::infrastructure::windows::system::claude_usage as live;
    blocking(|| {
        let svc = live::service();
        let now = claude_store::now_secs();
        svc.attempt(&mut live::LivePort { paths: claude_store::Paths::from_env() }, now, now * 1000, true);
        Ok(svc.view())
    })
    .await
}

/// Si los hooks de Tesela están puestos en el `settings.json` de Claude Code.
#[tauri::command]
pub async fn claude_hooks_status() -> Result<claude_store::HooksStatus, String> {
    blocking(|| Ok(claude_store::hooks_status(&claude_store::Paths::from_env()))).await
}

/// Pone los hooks (con copia de seguridad de `settings.json`). Solo cuando la persona lo pide en Ajustes.
#[tauri::command]
pub async fn claude_hooks_install() -> Result<claude_store::HooksStatus, String> {
    blocking(|| claude_store::install_hooks(&claude_store::Paths::from_env(), claude_store::now_secs())).await
}

/// Quita los hooks de Tesela, y solo ellos.
#[tauri::command]
pub async fn claude_hooks_uninstall() -> Result<claude_store::HooksStatus, String> {
    blocking(|| claude_store::uninstall_hooks(&claude_store::Paths::from_env(), claude_store::now_secs())).await
}

#[tauri::command]
pub async fn radio_states(state: State<'_, AppState>) -> Result<Vec<RadioState>, String> {
    let s = state.services.clone();
    blocking(move || Ok(s.radios.lock().map_err(lock_err)?.states())).await
}

#[tauri::command]
pub async fn radio_set(state: State<'_, AppState>, kind: String, on: bool) -> Result<ActionOutcome, String> {
    let kind = RadioKind::from_name(&kind).ok_or_else(|| format!("interruptor desconocido: {kind}"))?;
    let s = state.services.clone();
    blocking(move || s.radios.lock().map_err(lock_err)?.set(kind, on)).await
}

#[tauri::command]
pub async fn wifi_networks(state: State<'_, AppState>) -> Result<Vec<WifiNetwork>, String> {
    let s = state.services.clone();
    blocking(move || s.network.lock().map_err(lock_err)?.wifi_networks()).await
}

#[tauri::command]
pub async fn wifi_connect(state: State<'_, AppState>, ssid: String, password: Option<String>) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    blocking(move || s.network.lock().map_err(lock_err)?.wifi_connect(&ssid, password.as_deref())).await
}

#[tauri::command]
pub async fn wifi_disconnect(state: State<'_, AppState>) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    blocking(move || s.network.lock().map_err(lock_err)?.wifi_disconnect()).await
}

#[tauri::command]
pub async fn bluetooth_devices(state: State<'_, AppState>) -> Result<Vec<BluetoothDevice>, String> {
    let s = state.services.clone();
    blocking(move || s.network.lock().map_err(lock_err)?.bluetooth_devices()).await
}

#[tauri::command]
pub async fn hotspot_info(state: State<'_, AppState>) -> Result<Option<HotspotInfo>, String> {
    let s = state.services.clone();
    blocking(move || s.network.lock().map_err(lock_err)?.hotspot()).await
}

#[tauri::command]
pub async fn hotspot_configure(state: State<'_, AppState>, ssid: String, passphrase: String) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    blocking(move || s.network.lock().map_err(lock_err)?.hotspot_configure(&ssid, &passphrase)).await
}

#[tauri::command]
pub async fn audio_outputs(state: State<'_, AppState>) -> Result<Vec<AudioDevice>, String> {
    let s = state.services.clone();
    blocking(move || s.volume.lock().map_err(lock_err)?.outputs()).await
}

#[tauri::command]
pub async fn audio_set_output(state: State<'_, AppState>, id: String) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    blocking(move || s.volume.lock().map_err(lock_err)?.set_output(&id)).await
}

/// Abre una página de la Configuración de Windows (solo las de esta lista).
#[tauri::command]
pub fn open_windows_settings(page: String) -> Result<(), String> {
    let uri = match page.as_str() {
        "bluetooth" => "ms-settings:bluetooth",
        "wifi" => "ms-settings:network-wifi",
        "hotspot" => "ms-settings:network-mobilehotspot",
        "sound" => "ms-settings:sound",
        other => return Err(format!("página de configuración desconocida: {other}")),
    };
    use std::os::windows::process::CommandExt;
    std::process::Command::new("cmd")
        .args(["/C", "start", "", uri])
        .creation_flags(0x0800_0000) // sin ventana de consola
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn power_run(state: State<'_, AppState>, action: String) -> Result<ActionOutcome, String> {
    let action = PowerAction::from_name(&action).ok_or_else(|| format!("acción de energía desconocida: {action}"))?;
    let s = state.services.clone();
    blocking(move || s.power.lock().map_err(lock_err)?.run(action)).await
}

#[derive(Serialize)]
pub struct WallpaperList {
    pub dir: String,
    pub images: Vec<String>,
    pub current: Option<String>,
}

#[tauri::command]
pub async fn wallpapers_list(state: State<'_, AppState>) -> Result<WallpaperList, String> {
    let s = state.services.clone();
    let dir = state.wallpapers_dir.clone();
    blocking(move || {
        let _ = std::fs::create_dir_all(&dir);
        let current = s.wallpaper.lock().map_err(lock_err)?.current().unwrap_or(None);
        Ok(WallpaperList { dir: dir.display().to_string(), images: list_wallpapers(&dir), current })
    })
    .await
}

#[tauri::command]
pub async fn wallpaper_set(state: State<'_, AppState>, name: String) -> Result<ActionOutcome, String> {
    let path = resolve_wallpaper(&state.wallpapers_dir, &name)?;
    let s = state.services.clone();
    blocking(move || s.wallpaper.lock().map_err(lock_err)?.set(&path.to_string_lossy())).await
}

// ----- Notas rápidas -----

use crate::infrastructure::notes_store::Note;

#[tauri::command]
pub fn notes_list(state: State<'_, AppState>) -> Vec<Note> {
    state.notes.list()
}

#[tauri::command]
pub fn notes_add(state: State<'_, AppState>, text: String) -> Result<Note, String> {
    state.notes.add(&text)
}

#[tauri::command]
pub fn notes_update(state: State<'_, AppState>, id: String, text: String) -> Result<Note, String> {
    state.notes.update(&id, &text)
}

#[tauri::command]
pub fn notes_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.notes.delete(&id)
}

/// Carpeta donde se guardan las notas exportadas: Documentos/Tesela/Notas.
fn notes_export_dir() -> Result<std::path::PathBuf, String> {
    Ok(crate::infrastructure::windows::known_folders::documents()?.join("Tesela").join("Notas"))
}

/// Guarda el texto ya preparado por la interfaz (Markdown o texto plano) como archivo nuevo; devuelve la ruta.
#[tauri::command]
pub fn notes_export(name: String, content: String) -> Result<String, String> {
    let path = crate::infrastructure::notes_store::export_to(&notes_export_dir()?, &name, &content)?;
    Ok(path.display().to_string())
}

/// Abre en el Explorador la carpeta de notas exportadas (la crea si no existe).
#[tauri::command]
pub fn notes_open_folder() -> Result<(), String> {
    let dir = notes_export_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new("explorer").arg(&dir).creation_flags(0x0800_0000).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clipboard_read(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let s = state.services.clone();
    blocking(move || s.clipboard.lock().map_err(lock_err)?.read_text()).await
}

#[tauri::command]
pub async fn clipboard_write(state: State<'_, AppState>, text: String) -> Result<(), String> {
    let s = state.services.clone();
    blocking(move || s.clipboard.lock().map_err(lock_err)?.write_text(&text)).await
}

/// Biblioteca de Wallpaper Engine (vacía y `available: false` si no está instalado).
#[tauri::command]
pub async fn engine_library(state: State<'_, AppState>) -> Result<EngineLibrary, String> {
    let s = state.services.clone();
    blocking(move || Ok(s.engine.lock().map_err(lock_err)?.library())).await
}

/// Guarda la carpeta de Wallpaper Engine que indicó la persona (cuando no está en la ruta de siempre) y lo empieza a usar.
/// Devuelve la biblioteca ya leída; si la carpeta no sirve, el motivo.
#[tauri::command]
pub async fn engine_set_path(app: tauri::AppHandle, state: State<'_, AppState>, path: String) -> Result<EngineLibrary, String> {
    use crate::infrastructure::windows::system::wallpaper_engine as we;
    use tauri::Manager;
    let s = state.services.clone();
    blocking(move || {
        let install = we::resolve_install(&path)?;
        we::write_saved_path(&we::settings_file(), &install)?;
        let engine = we::WallpaperEngine::from_install(install);
        // Sus previsualizaciones se leen de sus carpetas de fondos.
        for dir in engine.source_dirs() {
            let _ = app.asset_protocol_scope().allow_directory(dir, true);
        }
        let mut slot = s.engine.lock().map_err(lock_err)?;
        *slot = Box::new(engine);
        Ok(slot.library())
    })
    .await
}

/// Pide a Wallpaper Engine que ponga un fondo de su biblioteca (`monitor` vacío = todos).
#[tauri::command]
pub async fn engine_apply(state: State<'_, AppState>, id: String, monitor: Option<u32>) -> Result<ActionOutcome, String> {
    let s = state.services.clone();
    let transition = state.wallpaper_transition;
    blocking(move || {
        if transition {
            if let Err(e) = play_wallpaper_transition(&s, &id, monitor) {
                // Sin transición el cambio se hace igual.
                eprintln!("[wm] sin transición de fondo: {e}");
            }
        }
        s.engine.lock().map_err(lock_err)?.apply(&id, monitor)
    })
    .await
}

/// Abre la transición (un círculo con la previsualización del fondo nuevo, detrás de las ventanas) en los monitores
/// afectados. Vuelve en cuanto está dibujada; la animación termina sola cuando Wallpaper Engine ya tiene el fondo.
fn play_wallpaper_transition(s: &Arc<SystemServices>, id: &str, monitor: Option<u32>) -> Result<(), String> {
    use crate::infrastructure::windows::{facts::list_monitors, fade};
    let (library, preview) = {
        let mut engine = s.engine.lock().map_err(lock_err)?;
        let library = engine.library();
        let preview = library.items.iter().find(|w| w.id == id).and_then(|w| w.preview.clone());
        (library, preview)
    };
    // La previsualización solo se usa si no se puede copiar el fondo actual.
    let preview = preview.unwrap_or_default();
    // Wallpaper Engine numera los monitores de izquierda a derecha (Monitor0 es el de más a la izquierda), igual que la app.
    let bounds: Vec<_> = list_monitors().into_iter().map(|m| m.bounds).collect();
    // Monitores afectados: el elegido o todos los que Wallpaper Engine tiene configurados.
    let targets: Vec<u32> = match monitor {
        Some(m) => vec![m],
        None if library.current.is_empty() => (0..bounds.len() as u32).collect(),
        None => library.current.iter().map(|c| c.monitor).collect(),
    };
    let overlays: Vec<fade::Overlay> = targets
        .iter()
        .filter_map(|&m| bounds.get(m as usize).map(|b| fade::Overlay { bounds: *b, preview: preview.clone() }))
        .collect();
    if overlays.is_empty() {
        return Err("no hay monitores donde dibujar la transición".into());
    }
    let cursor = {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p).ok().map(|_| (p.x, p.y)) }
    };
    let (services, wanted) = (s.clone(), id.to_string());
    let applied = Arc::new(move || {
        let Ok(mut engine) = services.engine.try_lock() else { return false };
        let current = engine.current();
        targets.iter().all(|m| current.iter().any(|c| c.monitor == *m && c.id == wanted))
    });
    fade::play(overlays, cursor, applied).map(|_| ())
}

/// Color dominante (`#rrggbb`) del fondo de pantalla del monitor donde está la ventana que lo pide (el de Wallpaper Engine si
/// lo hay); `None` si no se puede leer. Cada barra y cada panel toma así el color del fondo de su propio monitor.
#[tauri::command]
pub async fn wallpaper_color(window: tauri::WebviewWindow, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let s = state.services.clone();
    let center = match (window.outer_position(), window.outer_size()) {
        (Ok(p), Ok(sz)) => Some((p.x + sz.width as i32 / 2, p.y + sz.height as i32 / 2)),
        _ => None,
    };
    blocking(move || {
        use crate::infrastructure::windows::system::wallpaper_color::color_of;
        // Wallpaper Engine numera los monitores de izquierda a derecha, igual que la app.
        let monitor = center.and_then(|(cx, cy)| crate::infrastructure::windows::facts::monitor_index_at(cx, cy)).unwrap_or(0) as u32;
        // Con Wallpaper Engine el fondo de Windows es una copia casi negra: el color sale de la previsualización del
        // fondo que Wallpaper Engine tiene puesto de verdad en ese monitor.
        if let Some(preview) = s.engine.lock().map_err(lock_err)?.current_preview_for(monitor) {
            if let Ok(color) = color_of(&preview) {
                return Ok(Some(color));
            }
        }
        let Some(path) = s.wallpaper.lock().map_err(lock_err)?.current().unwrap_or(None) else { return Ok(None) };
        Ok(color_of(&path).ok())
    })
    .await
}

// ----- Módulos instalables (Fase 7) -----

/// Avisa a todas las ventanas (barra, panel, principal) de que cambió la lista de módulos.
fn changed(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _ = app.emit("wm-modules-changed", ());
}

fn errors_to_string(e: Vec<String>) -> String {
    e.join("\n")
}

#[tauri::command(async)]
pub fn modules_list(state: State<'_, AppState>) -> Vec<InstalledModule> {
    state.modules.list()
}

/// Paso 1 de instalar: revisa la carpeta y devuelve los permisos que pide, sin instalar nada (§10.3).
#[tauri::command(async)]
pub fn modules_preview(state: State<'_, AppState>, path: String) -> Result<InstallPreview, String> {
    state.modules.preview(std::path::Path::new(&path)).map_err(errors_to_string)
}

/// Paso 2: instala con los permisos que el usuario aceptó.
#[tauri::command(async)]
pub fn modules_install(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    granted: Vec<Permission>,
) -> Result<InstalledModule, String> {
    let installed = state.modules.install(std::path::Path::new(&path), &granted).map_err(errors_to_string)?;
    changed(&app);
    Ok(installed)
}

#[tauri::command(async)]
pub fn modules_uninstall(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    // Primero se cortan sus suscripciones; después se borra todo.
    state.module_api.unsubscribe_all(&id);
    state.modules.uninstall(&id)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn modules_set_granted(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    granted: Vec<Permission>,
) -> Result<(), String> {
    state.modules.set_granted(&id, &granted)?;
    changed(&app);
    Ok(())
}

#[tauri::command(async)]
pub fn modules_set_enabled(app: tauri::AppHandle, state: State<'_, AppState>, id: String, enabled: bool) -> Result<(), String> {
    if !enabled {
        state.module_api.unsubscribe_all(&id);
    }
    state.modules.set_enabled(&id, enabled)?;
    changed(&app);
    Ok(())
}

/// Puente de la API del núcleo: el anfitrión reenvía aquí lo que el módulo pide por postMessage.
#[tauri::command]
pub async fn module_api(
    state: State<'_, AppState>,
    module: String,
    method: String,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let api = state.module_api.clone();
    blocking(move || api.call(&module, &method, params).map_err(|e| e.to_string())).await
}

/// Si la app se arrancó con `--selftest-module`, id del módulo a probar.
#[tauri::command]
pub fn selftest_config(state: State<'_, AppState>) -> Option<String> {
    state.selftest.as_ref().map(|t| t.module.clone())
}

/// La ventana principal entrega lo que observó; se guarda en `WM_SELFTEST_OUT` y la app termina.
#[tauri::command]
pub fn selftest_report(app: tauri::AppHandle, state: State<'_, AppState>, report: String) -> Result<(), String> {
    let t = state.selftest.as_ref().ok_or("no hay prueba en curso")?;
    std::fs::write(&t.out, report).map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

// ----- General: arranque con Windows, reinicio e información (Fase 10) -----

#[derive(Serialize)]
pub struct GeneralStatus {
    pub version: String,
    pub managing: bool,
    pub bar: bool,
    /// `true` si la app arranca sola al iniciar sesión en Windows.
    pub autostart_enabled: bool,
    pub autostart_command: Option<String>,
    pub config_dir: String,
    /// Carpeta con los módulos que trae el instalador, si existe.
    pub bundled_modules_dir: Option<String>,
    /// ¿Esta compilación incluye las actualizaciones automáticas?
    pub updater_compiled: bool,
}

fn general(app: &tauri::AppHandle, state: &AppState) -> GeneralStatus {
    use tauri::Manager;
    #[cfg(windows)]
    let autostart = crate::infrastructure::windows::autostart::status(
        crate::infrastructure::windows::autostart::RUN_KEY,
        crate::infrastructure::windows::autostart::VALUE_NAME,
    );
    #[cfg(not(windows))]
    let autostart: Option<String> = None;
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("modules"))
        .filter(|d| d.is_dir())
        .map(|d| d.display().to_string());
    GeneralStatus {
        version: crate::domain::modules::CORE_VERSION.to_string(),
        managing: state.general.managing,
        bar: state.general.bar,
        autostart_enabled: autostart.is_some(),
        autostart_command: autostart,
        config_dir: crate::infrastructure::config_file::config_dir().display().to_string(),
        bundled_modules_dir: bundled,
        updater_compiled: cfg!(feature = "updater"),
    }
}

#[tauri::command]
pub fn general_status(app: tauri::AppHandle, state: State<'_, AppState>) -> GeneralStatus {
    general(&app, &state)
}

/// Activa o desactiva el arranque con Windows. Al activarlo la app arrancará gestionando todas las ventanas y con barra.
#[tauri::command]
pub fn autostart_set(app: tauri::AppHandle, state: State<'_, AppState>, enabled: bool) -> Result<GeneralStatus, String> {
    #[cfg(windows)]
    {
        use crate::infrastructure::windows::autostart::{command_line, disable, enable, RUN_KEY, VALUE_NAME};
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            enable(RUN_KEY, VALUE_NAME, &command_line(&exe, true, true, true))?;
        } else {
            disable(RUN_KEY, VALUE_NAME)?;
        }
    }
    #[cfg(not(windows))]
    let _ = enabled;
    Ok(general(&app, &state))
}

/// Tareas del día, en el orden en que se hacen.
#[tauri::command]
pub fn tasks_list() -> Vec<crate::domain::tasks::Task> {
    crate::infrastructure::tasks_store::load(&crate::infrastructure::tasks_store::default_path())
}

/// Guarda la lista completa de tareas (la valida) y la devuelve ya limpia.
#[tauri::command]
pub fn tasks_save(tasks: Vec<crate::domain::tasks::Task>) -> Result<Vec<crate::domain::tasks::Task>, String> {
    crate::infrastructure::tasks_store::save(&crate::infrastructure::tasks_store::default_path(), tasks)
}

/// Aplicaciones que se abren junto con Tesela, cada una con su workspace.
#[tauri::command]
pub fn startup_list() -> Vec<crate::domain::startup::StartupApp> {
    crate::infrastructure::startup_store::load(&crate::infrastructure::startup_store::default_path())
}

/// Guarda la lista de aplicaciones de arranque (la valida) y la devuelve ya limpia.
#[tauri::command]
pub fn startup_save(apps: Vec<crate::domain::startup::StartupApp>) -> Result<Vec<crate::domain::startup::StartupApp>, String> {
    crate::infrastructure::startup_store::save(&crate::infrastructure::startup_store::default_path(), apps)
}

/// Las aplicaciones que tiene el usuario (instaladas y abiertas) para elegir una al excluir o al abrir al arrancar.
#[cfg(windows)]
#[tauri::command]
pub async fn app_suggestions() -> Result<Vec<crate::domain::startup::AppSuggestion>, String> {
    blocking(|| Ok(crate::infrastructure::windows::system::installed_apps::suggestions())).await
}

/// Cierra la app por completo: al salir se devuelven las ventanas a su sitio y se retira la barra.
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Enciende o apaga el borde de la ventana en uso («Sin marco» en Personalización).
#[cfg(windows)]
#[tauri::command]
pub fn border_enable(on: bool) {
    crate::interface::focus_border::set_enabled(on);
}

/// Reinicia la app con o sin gestor de ventanas y barra. La instancia nueva espera un momento para que esta
/// suelte los atajos y devuelva las ventanas a su sitio.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle, manage: bool, bar: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    // Ya están abiertas las aplicaciones de arranque: la instancia nueva no las vuelve a abrir.
    cmd.args(["--delay-ms", "2500", "--no-startup-apps"]);
    if manage {
        cmd.args(["--manage", "all"]);
    }
    if bar {
        cmd.arg("--bar");
    }
    cmd.spawn().map_err(|e| format!("no se pudo reiniciar: {e}"))?;
    app.exit(0);
    Ok(())
}
