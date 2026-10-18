//! Barra de tareas propia: una «isla» centrada abajo en cada monitor, con las ventanas de cada workspace. Flota por encima
//! de las aplicaciones sin reservar espacio y, como el panel, está oculta hasta que se pide: con la tecla Windows, o
//! acercando el cursor al centro del borde de abajo. Se vuelve a ocultar al tocar una app, al alejar el cursor o con la
//! tecla Windows. Su ancho sigue a su contenido (la propia vista avisa de cuánto necesita).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::domain::desktop::Monitor;
use crate::domain::geometry::Rect;
use crate::infrastructure::windows::appbar;
use crate::infrastructure::windows::facts::list_monitors;

/// Alto (lógico) de la isla.
pub const ISLAND_HEIGHT: f64 = 48.0;
/// Separación (lógica) entre la isla y el borde de abajo del monitor.
pub const BOTTOM_MARGIN: f64 = 14.0;
const MIN_WIDTH: f64 = 120.0;
/// Lo que tarda la animación de cierre de la vista (algo más, por si el sistema va justo).
const CLOSE_MS: u64 = 460;
/// Mitad (lógica) del ancho de la zona del borde de abajo que despliega la barra.
const HOVER_HALF_WIDTH: f64 = 150.0;
/// Grosor (lógico) de esa zona.
const HOVER_EDGE: f64 = 4.0;
const HOVER_DWELL: Duration = Duration::from_millis(160);
const HOVER_LEAVE: Duration = Duration::from_millis(450);

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Una sola sincronización a la vez: si dos hilos crean la ventana de un monitor al mismo tiempo, la segunda falla.
static SYNCING: Mutex<()> = Mutex::new(());
/// Ancho (lógico) que pidió la vista de cada monitor.
static WIDTHS: Mutex<Option<HashMap<u32, f64>>> = Mutex::new(None);
/// Sube con cada despliegue: un cierre pendiente de otra generación se cancela.
static GEN: AtomicU64 = AtomicU64::new(0);
/// Monitores donde la barra está desplegada ahora.
static SHOWN: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
/// Monitores con una app a pantalla completa: ahí no se despliega.
static FULLSCREEN: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

pub fn label(monitor: u32) -> String {
    format!("taskbar-{monitor}")
}

/// Id de monitor que lleva la etiqueta de una ventana de la barra de tareas.
pub fn monitor_of_label(label: &str) -> Option<u32> {
    label.strip_prefix("taskbar-")?.parse().ok()
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

fn hwnd_of(w: &tauri::WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}

/// Rectángulo físico de la isla en un monitor: centrada y separada del borde de abajo.
pub fn island_rect(monitor: Rect, width_logical: f64, scale: f64) -> Rect {
    let w = (width_logical.max(MIN_WIDTH) * scale).round() as i32;
    let h = (ISLAND_HEIGHT * scale).round() as i32;
    let margin = (BOTTOM_MARGIN * scale).round() as i32;
    Rect::new(monitor.x + (monitor.w - w) / 2, monitor.bottom() - margin - h, w, h)
}

/// ¿El cursor está en la zona del borde de abajo, en el centro, que despliega la barra?
pub fn in_reveal_zone(monitor: Rect, scale: f64, x: i32, y: i32) -> bool {
    let half = (HOVER_HALF_WIDTH * scale) as i32;
    let mid = monitor.x + monitor.w / 2;
    let edge = ((HOVER_EDGE * scale) as i32).max(2);
    x >= mid - half && x <= mid + half && y >= monitor.bottom() - edge && y < monitor.bottom()
}

fn set_contains(set: &Mutex<Option<HashSet<u32>>>, monitor: u32) -> bool {
    set.lock().unwrap().as_ref().is_some_and(|s| s.contains(&monitor))
}

fn set_change(set: &Mutex<Option<HashSet<u32>>>, monitor: u32, present: bool) -> bool {
    let mut guard = set.lock().unwrap();
    let s = guard.get_or_insert_with(HashSet::new);
    if present { s.insert(monitor) } else { s.remove(&monitor) }
}

/// ¿Está desplegada la barra de tareas de ese monitor?
pub fn is_shown(monitor: u32) -> bool {
    set_contains(&SHOWN, monitor)
}

fn place(app: &AppHandle, monitor: u32) {
    let Some(window) = app.get_webview_window(&label(monitor)) else { return };
    let Some(m) = list_monitors().into_iter().find(|m| m.id == monitor) else { return };
    let width = WIDTHS.lock().unwrap().as_ref().and_then(|w| w.get(&monitor).copied()).unwrap_or(MIN_WIDTH);
    let r = island_rect(m.bounds, width, m.dpi_scale);
    if let Some(h) = hwnd_of(&window) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE};
        unsafe {
            let _ = SetWindowPos(HWND(h as *mut _), Some(HWND_TOPMOST), r.x, r.y, r.w, r.h, SWP_NOACTIVATE);
        }
    }
}

/// Despliega la barra de tareas de un monitor (con su animación). No hace nada si está desactivada o hay una app a
/// pantalla completa en ese monitor.
pub fn show(app: &AppHandle, monitor: u32) {
    if !ENABLED.load(Ordering::SeqCst) || set_contains(&FULLSCREEN, monitor) {
        return;
    }
    let Some(window) = app.get_webview_window(&label(monitor)) else { return };
    // Si se pide mientras se cerraba, se cancela el cierre.
    GEN.fetch_add(1, Ordering::SeqCst);
    place(app, monitor);
    set_change(&SHOWN, monitor, true);
    crate::interface::webview::set_visible(&window, true);
    if let Some(h) = hwnd_of(&window) {
        appbar::set_visible(h, true);
    }
    let _ = window.emit("wm-taskbar-shown", ());
}

/// Cierra la barra de tareas de un monitor: la vista reproduce su animación y, pasado el tiempo, se oculta la ventana.
pub fn hide(app: &AppHandle, monitor: u32) {
    if !set_change(&SHOWN, monitor, false) {
        return;
    }
    let Some(window) = app.get_webview_window(&label(monitor)) else { return };
    let _ = window.emit("wm-taskbar-closing", ());
    let generation = GEN.load(Ordering::SeqCst);
    let _ = std::thread::Builder::new().name("wm-taskbar-close".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(CLOSE_MS));
        if GEN.load(Ordering::SeqCst) == generation && !set_contains(&SHOWN, monitor) {
            if let Some(h) = hwnd_of(&window) {
                appbar::set_visible(h, false);
            }
            crate::interface::webview::set_visible(&window, false);
        }
    });
}

/// Despliega o cierra la barra de tareas de un monitor.
pub fn toggle(app: &AppHandle, monitor: u32) {
    if is_shown(monitor) {
        hide(app, monitor);
    } else {
        show(app, monitor);
    }
}

/// Crea las barras de tareas que falten (ocultas) y retira las de monitores que ya no están. Sin la opción encendida, las
/// quita todas. Debe llamarse desde un hilo que no sea el principal (crea ventanas).
pub fn sync(app: &AppHandle, monitors: &[Monitor]) {
    let _one_at_a_time = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    if !ENABLED.load(Ordering::SeqCst) {
        remove_all(app);
        return;
    }
    let wanted: Vec<u32> = monitors.iter().map(|m| m.id).collect();
    for (label, window) in app.webview_windows() {
        if monitor_of_label(&label).is_some_and(|id| !wanted.contains(&id)) {
            let _ = window.destroy();
        }
    }
    for m in monitors {
        let name = label(m.id);
        if app.get_webview_window(&name).is_some() {
            continue;
        }
        let built = WebviewWindowBuilder::new(app, &name, WebviewUrl::App(format!("index.html?view=taskbar&monitor={}", m.id).into()))
            .title("Tesela Taskbar")
            .decorations(false)
            .transparent(true)
            .resizable(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .shadow(false)
            .focused(false)
            .focusable(false)
            .visible(false)
            .inner_size(MIN_WIDTH, ISLAND_HEIGHT)
            .build();
        match built {
            Ok(w) => {
                if let Some(h) = hwnd_of(&w) {
                    // Sin foco y fuera del Alt+Tab: es una barra, no una ventana de trabajo.
                    appbar::make_tool_window(h);
                }
                crate::interface::webview::set_visible(&w, false);
            }
            Err(e) => eprintln!("[wm] no se pudo crear la barra de tareas {name}: {e}"),
        }
    }
}

fn remove_all(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if monitor_of_label(&label).is_some() {
            let _ = window.destroy();
        }
    }
    SHOWN.lock().unwrap().take();
}

/// La vista avisa del ancho (lógico) que necesita su contenido.
pub fn resize(app: &AppHandle, monitor: u32, width: f64) {
    if !width.is_finite() {
        return;
    }
    WIDTHS.lock().unwrap().get_or_insert_with(HashMap::new).insert(monitor, width.clamp(MIN_WIDTH, 4000.0));
    place(app, monitor);
}

/// Pantalla completa en un monitor: la barra de tareas se cierra ya (sin animación) y no se despliega mientras dure.
pub fn set_fullscreen(app: &AppHandle, monitor: u32, fullscreen: bool) {
    set_change(&FULLSCREEN, monitor, fullscreen);
    if fullscreen && set_change(&SHOWN, monitor, false) {
        GEN.fetch_add(1, Ordering::SeqCst);
        if let Some(window) = app.get_webview_window(&label(monitor)) {
            if let Some(h) = hwnd_of(&window) {
                appbar::set_visible(h, false);
            }
            crate::interface::webview::set_visible(&window, false);
        }
    }
}

fn cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

/// Despliega la barra al acercar el cursor al centro del borde de abajo y la cierra al alejarlo (si se desplegó así, o si
/// ya se había pasado el cursor por encima). Si se desplegó con la tecla Windows y el cursor no ha llegado a estar sobre
/// ella, se queda hasta que se vuelva a pulsar Windows o se toque una app.
pub fn start_hover_trigger(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("wm-taskbar-hover".into()).spawn(move || {
        let mut dwell = Duration::ZERO;
        let mut armed = true;
        let mut entered: HashSet<u32> = HashSet::new();
        let mut outside_since: HashMap<u32, Instant> = HashMap::new();
        let tick = Duration::from_millis(40);
        loop {
            std::thread::sleep(tick);
            if !ENABLED.load(Ordering::SeqCst) {
                continue;
            }
            let Some((cx, cy)) = cursor_pos() else { continue };
            let monitors = list_monitors();

            // Cierre: el cursor se alejó de una barra desplegada.
            for m in &monitors {
                if !is_shown(m.id) {
                    entered.remove(&m.id);
                    outside_since.remove(&m.id);
                    continue;
                }
                let width = WIDTHS.lock().unwrap().as_ref().and_then(|w| w.get(&m.id).copied()).unwrap_or(MIN_WIDTH);
                let r = island_rect(m.bounds, width, m.dpi_scale);
                let pad = (28.0 * m.dpi_scale) as i32;
                let inside = cx >= r.x - pad && cx <= r.right() + pad && cy >= r.y - pad && cy <= m.bounds.bottom();
                if inside {
                    entered.insert(m.id);
                    outside_since.remove(&m.id);
                } else if entered.contains(&m.id) && outside_since.entry(m.id).or_insert_with(Instant::now).elapsed() >= HOVER_LEAVE {
                    hide(&app, m.id);
                    entered.remove(&m.id);
                    outside_since.remove(&m.id);
                }
            }

            // Apertura: el cursor en el centro del borde de abajo.
            let zone = monitors.iter().find(|m| in_reveal_zone(m.bounds, m.dpi_scale, cx, cy));
            match zone {
                Some(m) => {
                    if !armed || is_shown(m.id) {
                        dwell = Duration::ZERO;
                        continue;
                    }
                    dwell += tick;
                    if dwell >= HOVER_DWELL {
                        dwell = Duration::ZERO;
                        armed = false;
                        show(&app, m.id);
                        entered.insert(m.id);
                    }
                }
                None => {
                    dwell = Duration::ZERO;
                    armed = true;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_isla_se_centra_en_el_monitor_y_se_separa_del_borde_de_abajo() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        let r = island_rect(monitor, 400.0, 1.0);
        assert_eq!((r.x, r.w, r.h), (760, 400, 48));
        assert_eq!(r.bottom(), 1080 - 14, "deja el margen por debajo");
    }

    #[test]
    fn con_escala_se_multiplica_y_nunca_baja_del_minimo() {
        let monitor = Rect::new(1920, 0, 2560, 1440);
        let r = island_rect(monitor, 10.0, 1.5);
        assert_eq!(r.w, 180, "el ancho mínimo (120) por la escala");
        assert_eq!(r.h, 72);
        assert_eq!(r.x + r.w / 2, 1920 + 1280, "centrada en el monitor");
    }

    #[test]
    fn la_zona_que_despliega_es_el_centro_del_borde_de_abajo() {
        let m = Rect::new(0, 0, 1920, 1080);
        assert!(in_reveal_zone(m, 1.0, 960, 1079));
        assert!(in_reveal_zone(m, 1.0, 960 + 150, 1077));
        assert!(!in_reveal_zone(m, 1.0, 960 + 151, 1079), "fuera por los lados");
        assert!(!in_reveal_zone(m, 1.0, 960, 1070), "no basta con acercarse: es el borde");
        assert!(!in_reveal_zone(m, 1.0, 960, 1080), "ya fuera del monitor");
        let second = Rect::new(1920, 0, 2560, 1440);
        assert!(in_reveal_zone(second, 1.5, 1920 + 1280, 1439));
    }

    #[test]
    fn el_estado_de_desplegada_y_pantalla_completa_se_lleva_por_monitor() {
        assert!(!is_shown(90_001));
        assert!(set_change(&SHOWN, 90_001, true));
        assert!(is_shown(90_001) && !is_shown(90_002));
        assert!(set_change(&SHOWN, 90_001, false));
        assert!(!is_shown(90_001));
        assert!(!set_change(&SHOWN, 90_001, false), "quitar lo que no está no cambia nada");
    }

    #[test]
    fn los_permisos_de_tauri_cubren_todas_las_ventanas_de_la_app() {
        // Una ventana que no está en las capacidades no puede usar comandos ni eventos: su vista se queda vacía.
        let caps: serde_json::Value = serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
        let patterns: Vec<&str> = caps["windows"].as_array().unwrap().iter().filter_map(|w| w.as_str()).collect();
        let covered = |label: &str| patterns.iter().any(|p| p.strip_suffix('*').map_or(*p == label, |prefix| label.starts_with(prefix)));
        for label in [label(7), "bar-7".to_string(), "panel".to_string(), "focus-border".to_string(), "main".to_string()] {
            assert!(covered(&label), "falta «{label}» en capabilities/default.json");
        }
    }

    #[test]
    fn la_etiqueta_lleva_el_monitor() {
        assert_eq!(monitor_of_label(&label(42)), Some(42));
        assert_eq!(monitor_of_label("bar-3"), None);
        assert_eq!(monitor_of_label("taskbar-x"), None);
    }
}
