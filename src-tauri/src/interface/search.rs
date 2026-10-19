//! Ventana del buscador de aplicaciones (§18) y su estado: redirecciones, historial de uso y servicio de búsqueda.
//!
//! La ventana sigue el patrón del panel (`bar.rs`): transparente, sin decoración, se oculta en vez de destruirse, su WebView
//! se duerme mientras está oculta, se cierra al perder el foco y el clic que la cierra no la reabre. A diferencia del panel
//! necesita foco de teclado (hay que escribir), así que lleva el mismo respaldo de la tecla Windows (§18.4).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::application::search::{SearchHost, SearchService};
use crate::domain::geometry::Rect;
use crate::domain::search::{Redirect, SearchItem};
use crate::infrastructure::search_store;
use crate::infrastructure::usage_store::{now_secs, UsageStore};
use crate::infrastructure::windows::appbar;
use crate::infrastructure::windows::facts::list_monitors;

const SEARCH_LABEL: &str = "search";
/// Ancho (lógico) de la ventana. El alto lo fija la vista según los resultados que tenga.
const SEARCH_WIDTH: f64 = 640.0;
const SEARCH_INITIAL_HEIGHT: f64 = 84.0;
/// Alto máximo (lógico): la copia de lo que hay detrás cubre hasta ahí.
const SEARCH_MAX_HEIGHT: f64 = 620.0;
const SEARCH_MIN_HEIGHT: f64 = 60.0;
/// Separación (lógica) bajo la barra superior.
const SEARCH_MARGIN: f64 = 12.0;
/// Lo que tarda la animación de cierre de la vista (algo más, por si el sistema va justo).
const SEARCH_CLOSE_MS: u64 = 460;

/// Cuándo se ocultó por perder el foco: evita que el mismo atajo o clic que la cerró la reabra al instante.
static HID_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Se está cerrando (la vista reproduce su animación al revés) y aún no se ha ocultado la ventana.
static CLOSING: AtomicBool = AtomicBool::new(false);
/// Sube cada vez que se vuelve a abrir: un cierre pendiente con otra generación se cancela.
static GEN: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------------------------------------------------
// Estado: lo que consultan los comandos
// ---------------------------------------------------------------------------------------------------------------------

pub struct SearchState {
    service: SearchService,
    host: Mutex<Box<dyn SearchHost>>,
    redirects: Mutex<Vec<Redirect>>,
    redirects_path: std::path::PathBuf,
    usage: UsageStore,
}

impl SearchState {
    pub fn new(host: Box<dyn SearchHost>) -> Self {
        let redirects_path = search_store::default_path();
        SearchState {
            service: SearchService::new(),
            host: Mutex::new(host),
            redirects: Mutex::new(search_store::load(&redirects_path)),
            redirects_path,
            usage: UsageStore::load(UsageStore::default_path()),
        }
    }

    pub fn query(&self, text: &str) -> Vec<SearchItem> {
        let redirects = self.redirects.lock().unwrap().clone();
        let usage = self.usage.snapshot();
        let mut host = self.host.lock().unwrap();
        self.service.query(host.as_mut(), text, &redirects, &usage, now_secs())
    }

    /// Abre un resultado y anota el uso. `Ok(false)` si el identificador no se reconoce (no se hizo nada).
    pub fn open(&self, id: &str, as_admin: bool) -> Result<bool, String> {
        let key = {
            let mut host = self.host.lock().unwrap();
            self.service.open(host.as_mut(), id, as_admin)?
        };
        match key {
            Some(key) => {
                self.usage.record(&key);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn icon(&self, id: &str) -> Option<String> {
        let path = self.service.icon_path(id)?;
        crate::infrastructure::windows::icons::file_icon_url(&path)
    }

    pub fn redirects(&self) -> Vec<Redirect> {
        self.redirects.lock().unwrap().clone()
    }

    pub fn set_redirects(&self, list: Vec<Redirect>) -> Result<Vec<Redirect>, String> {
        let saved = search_store::save(&self.redirects_path, list)?;
        *self.redirects.lock().unwrap() = saved.clone();
        Ok(saved)
    }

    pub fn clear_history(&self) -> Result<(), String> {
        self.usage.clear()
    }

    /// Cuántas cosas recuerda el historial (para la pestaña de Ajustes).
    pub fn history_len(&self) -> usize {
        self.usage.snapshot().len()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        SearchState::new(Box::new(crate::infrastructure::windows::system::search_host::Win32SearchHost::new(Default::default())))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Ventana
// ---------------------------------------------------------------------------------------------------------------------

fn hwnd_of(w: &tauri::WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}

/// Abre o cierra el buscador en el monitor donde está el cursor (ctrl+shift+espacio).
pub fn toggle_search(app: &AppHandle) {
    // Crear una ventana desde el hilo principal bloquea el bucle de eventos: se hace desde un hilo aparte.
    let handle = app.clone();
    let _ = std::thread::Builder::new().name("wm-search".into()).spawn(move || toggle_inner(&handle));
}

/// ¿El buscador está en pantalla y no se está cerrando?
pub fn is_open(app: &AppHandle) -> bool {
    !CLOSING.load(Ordering::SeqCst) && app.get_webview_window(SEARCH_LABEL).is_some_and(|w| w.is_visible().unwrap_or(false))
}

fn toggle_inner(app: &AppHandle) {
    let existing = app.get_webview_window(SEARCH_LABEL);
    if CLOSING.swap(false, Ordering::SeqCst) {
        // Se vuelve a pedir mientras se cerraba: se cancela el cierre y se vuelve a abrir.
        GEN.fetch_add(1, Ordering::SeqCst);
    } else if let Some(w) = &existing {
        if w.is_visible().unwrap_or(false) {
            close(app, true);
            return;
        }
    }
    if HID_AT.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(300)) {
        return;
    }
    let Some(monitor) = crate::interface::shell::cursor_monitor() else { return };
    let Some(m) = list_monitors().into_iter().find(|m| m.id == monitor).or_else(|| list_monitors().into_iter().find(|m| m.primary)) else {
        return;
    };
    let window = match existing {
        Some(w) => w,
        None => match build(app) {
            Some(w) => w,
            None => return,
        },
    };
    let s = m.dpi_scale;
    let (w, h) = ((SEARCH_WIDTH * s) as u32, (SEARCH_INITIAL_HEIGHT * s) as u32);
    // Centrado en el monitor y justo debajo de la barra (el área útil ya empieza ahí).
    let x = m.work_area.x + (m.work_area.w - w as i32) / 2;
    let y = m.work_area.y + (SEARCH_MARGIN * s) as i32;
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    if !window.is_visible().unwrap_or(false) {
        // Antes de mostrarla se guarda lo que hay detrás: la vista lo dibuja desenfocado bajo su tinte.
        let reach = ((SEARCH_MAX_HEIGHT * s) as i32).min(m.work_area.bottom() - y).max(h as i32);
        crate::infrastructure::windows::backdrop::remember_search(Rect::new(x, y, w as i32, reach));
        // La ventana que tenía el foco se apunta para devolvérselo si se cierra sin abrir nada.
        crate::interface::shell::remember_foreground();
    }
    crate::interface::webview::set_visible(&window, true);
    let _ = window.show();
    let _ = window.set_focus();
    if let Some(h) = hwnd_of(&window) {
        // `set_focus` puede quedarse corto si otra aplicación tiene el primer plano: este es el truco que usan las demás.
        crate::infrastructure::windows::platform::focus_window(h);
    }
    // Con una ventana de Tesela con foco de teclado Windows deja de llamar al gancho de la tecla Windows: se mantiene vivo.
    crate::interface::shell::keep_hook_fresh();
    // La ventana se reutiliza: la vista relanza así su animación y enfoca el cuadro de texto.
    let _ = window.emit("wm-search-shown", ());
}

fn build(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let url = WebviewUrl::App("index.html?view=search".into());
    let built = WebviewWindowBuilder::new(app, SEARCH_LABEL, url)
        .title("Tesela Buscador")
        .decorations(false)
        .transparent(true)
        // Sin la sombra de Windows: dibujaba un cuadro rectangular alrededor de la ventana redondeada.
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .inner_size(SEARCH_WIDTH, SEARCH_INITIAL_HEIGHT)
        .build();
    match built {
        Ok(w) => {
            if let Some(h) = hwnd_of(&w) {
                appbar::make_tool_window(h);
            }
            let app2 = app.clone();
            w.on_window_event(move |ev| {
                if let WindowEvent::Focused(false) = ev {
                    // Perder el foco (clic fuera, otra aplicación) cierra sin quitarle el foco a quien lo ha tomado.
                    close(&app2, false);
                    *HID_AT.lock().unwrap() = Some(Instant::now());
                }
            });
            Some(w)
        }
        Err(e) => {
            eprintln!("[wm] no se pudo crear el buscador: {e}");
            None
        }
    }
}

/// Cambia el alto del buscador (lógico) conservando su borde superior.
pub fn resize(app: &AppHandle, height: f64) {
    let Some(w) = app.get_webview_window(SEARCH_LABEL) else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    let Ok(size) = w.inner_size() else { return };
    let h = (height.clamp(SEARCH_MIN_HEIGHT, SEARCH_MAX_HEIGHT) * scale).round() as u32;
    if h != size.height {
        let _ = w.set_size(PhysicalSize::new(size.width, h));
    }
}

/// Empieza el cierre: avisa a la vista para que reproduzca la apertura al revés y, pasado el tiempo de la animación,
/// oculta la ventana. Con `restore_focus` se devuelve el foco a la ventana que lo tenía antes (Escape, el mismo atajo);
/// si no (se abrió algo o se hizo clic fuera), el foco ya es de otra ventana y no se toca.
pub fn close(app: &AppHandle, restore_focus: bool) {
    let Some(w) = app.get_webview_window(SEARCH_LABEL) else { return };
    if !w.is_visible().unwrap_or(false) || CLOSING.swap(true, Ordering::SeqCst) {
        return;
    }
    if restore_focus {
        crate::interface::shell::restore_previous_focus();
    } else {
        crate::interface::shell::forget_previous_focus();
    }
    let generation = GEN.load(Ordering::SeqCst);
    let _ = w.emit("wm-search-closing", ());
    let _ = std::thread::Builder::new().name("wm-search-close".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(SEARCH_CLOSE_MS));
        if GEN.load(Ordering::SeqCst) == generation && CLOSING.swap(false, Ordering::SeqCst) {
            let _ = w.hide();
            // Oculto no debe animar nada ni consultar el sistema: se duerme su WebView hasta que se vuelva a abrir.
            crate::interface::webview::set_visible(&w, false);
        }
    });
}

/// Crea la ventana oculta al arrancar, desde un hilo aparte, para que el primer uso sea instantáneo.
pub fn prepare(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("wm-search-prepare".into()).spawn(move || {
        if app.get_webview_window(SEARCH_LABEL).is_none() {
            if let Some(w) = build(&app) {
                crate::interface::webview::set_visible(&w, false);
            }
        }
    });
}
