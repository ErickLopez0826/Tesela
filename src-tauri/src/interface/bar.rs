//! Ventanas de la barra (una por monitor, acopladas como AppBar) y del panel desplegable (§9).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::domain::desktop::Monitor;
use crate::domain::geometry::Rect;
use crate::infrastructure::windows::appbar;
use crate::infrastructure::windows::facts::list_monitors;

/// Alto de la barra en píxeles lógicos (se escala con el DPI de cada monitor).
pub const BAR_HEIGHT: f64 = 40.0;
const PANEL_SIZE: (f64, f64) = (940.0, 400.0);
/// Ancho (lógico) de la zona central de la barra que abre el panel al acercar el cursor.
const HOVER_HALF_WIDTH: f64 = 150.0;
/// Tiempo con el cursor en la zona antes de abrir (evita aperturas por pasar de largo).
const HOVER_DWELL: Duration = Duration::from_millis(160);
/// Tiempo fuera del panel y de la zona antes de cerrarlo.
const HOVER_LEAVE: Duration = Duration::from_millis(450);

/// ¿Hay que cerrar el panel por tener el cursor fuera? Sí si lo abrió el cursor, o si el cursor ya estuvo dentro (se usó con el
/// ratón, aunque se abriera con la tecla Windows o con un clic): al alejarse se cierra. Un panel abierto con el teclado al que
/// el cursor nunca ha entrado no se cierra por esto (se maneja con el teclado).
fn leave_closes(by_hover: bool, entered: bool, outside_for: Duration) -> bool {
    (by_hover || entered) && outside_for >= HOVER_LEAVE
}
const PANEL_MARGIN: f64 = 12.0;
const PANEL_LABEL: &str = "panel";

/// Cuándo se ocultó el panel por perder el foco: evita que el clic en el botón de la barra lo reabra al instante.
static PANEL_HID_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// El panel se está cerrando (la vista reproduce su animación al revés) y aún no se ha ocultado la ventana.
static PANEL_CLOSING: AtomicBool = AtomicBool::new(false);
/// Sube cada vez que el panel se vuelve a abrir: un cierre pendiente con otra generación se cancela.
static PANEL_GEN: AtomicU64 = AtomicU64::new(0);
/// Lo que tarda la animación de cierre de la vista (algo más, por si el sistema va justo).
const PANEL_CLOSE_MS: u64 = 460;
/// Alto máximo (lógico) que puede tener el panel: la copia de lo que hay detrás cubre hasta ahí.
const PANEL_MAX_HEIGHT: f64 = 900.0;

fn bar_label(monitor: u32) -> String {
    format!("bar-{monitor}")
}

fn hwnd_of(w: &tauri::WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}

/// Crea las barras que falten, acopla de nuevo las existentes y retira las de monitores que ya no están.
pub fn sync_bars(app: &AppHandle, monitors: &[Monitor]) {
    let wanted: HashSet<String> = monitors.iter().map(|m| bar_label(m.id)).collect();
    for (label, window) in app.webview_windows() {
        if label.starts_with("bar-") && !wanted.contains(&label) {
            if let Some(h) = hwnd_of(&window) {
                appbar::undock(h);
            }
            let _ = window.destroy();
        }
    }
    for m in monitors {
        let label = bar_label(m.id);
        let height = (BAR_HEIGHT * m.dpi_scale).round() as i32;
        let window = match app.get_webview_window(&label) {
            Some(w) => Some(w),
            None => {
                let url = WebviewUrl::App(format!("index.html?view=bar&monitor={}", m.id).into());
                WebviewWindowBuilder::new(app, &label, url)
                    .title("Tesela Bar")
                    .decorations(false)
                    // El cristal (tinte y desenfoque de lo que hay detrás) lo dibuja la propia vista, con los valores de Personalización.
                    .transparent(true)
                    .resizable(false)
                    .skip_taskbar(true)
                    .always_on_top(true)
                    .shadow(false)
                    .focused(false)
                    .visible(false)
                    .inner_size(200.0, BAR_HEIGHT)
                    .build()
                    .map_err(|e| eprintln!("[wm] no se pudo crear la barra {label}: {e}"))
                    .ok()
            }
        };
        if let Some(h) = window.as_ref().and_then(hwnd_of) {
            let reserved = appbar::dock_top(h, m.bounds, height);
            eprintln!("[wm] barra {label} reservó {reserved:?}");
        }
    }
    crate::interface::taskbar::sync(app, monitors);
}

/// Muestra u oculta la barra de un monitor (pantalla completa, §6.4) sin quitarle el foco a nadie.
pub fn set_bar_visible(app: &AppHandle, monitor: u32, visible: bool) {
    if let Some(h) = app.get_webview_window(&bar_label(monitor)).as_ref().and_then(hwnd_of) {
        appbar::set_visible(h, visible);
    }
    // La barra de tareas de ese monitor también cede el paso a la pantalla completa.
    crate::interface::taskbar::set_fullscreen(app, monitor, !visible);
}

/// Quita la reserva de todas las barras (al salir).
pub fn undock_all(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with("bar-") {
            if let Some(h) = hwnd_of(&window) {
                appbar::undock(h);
            }
        }
    }
}

/// Abre o cierra el panel desplegable en el monitor indicado (§9.2).
pub fn toggle_panel(app: &AppHandle, monitor: u32) {
    open_panel_thread(app, monitor, true);
}

/// Igual, pero sin darle el foco al panel: la ventana que lo tenía lo conserva.
pub fn toggle_panel_unfocused(app: &AppHandle, monitor: u32) {
    open_panel_thread(app, monitor, false);
}

fn open_panel_thread(app: &AppHandle, monitor: u32, focus: bool) {
    // Crear una ventana desde el hilo principal bloquea el bucle de eventos (el webview necesita ese
    // mismo hilo para terminar de crearse), así que se hace desde un hilo aparte.
    let handle = app.clone();
    let _ = std::thread::Builder::new().name("wm-panel".into()).spawn(move || toggle_panel_inner(&handle, monitor, focus));
}

fn toggle_panel_inner(app: &AppHandle, monitor: u32, focus: bool) {
    eprintln!("[wm] alternar panel en el monitor {monitor}");
    let existing = app.get_webview_window(PANEL_LABEL);
    if PANEL_CLOSING.swap(false, Ordering::SeqCst) {
        // Se vuelve a pedir mientras se cerraba: se cancela el cierre y se vuelve a abrir.
        PANEL_GEN.fetch_add(1, Ordering::SeqCst);
    } else if let Some(w) = &existing {
        if w.is_visible().unwrap_or(false) {
            begin_hide(app);
            return;
        }
    }
    // El mismo clic que lo cerró por pérdida de foco no debe reabrirlo.
    if PANEL_HID_AT.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(300)) {
        return;
    }
    let Some(m) = list_monitors().into_iter().find(|m| m.id == monitor).or_else(|| list_monitors().into_iter().find(|m| m.primary))
    else {
        return;
    };
    let window = match existing {
        Some(w) => w,
        None => {
            let url = WebviewUrl::App("index.html?view=panel".into());
            let built = WebviewWindowBuilder::new(app, PANEL_LABEL, url)
                .title("Tesela Panel")
                .decorations(false)
                .transparent(true)
                // Sin la sombra de Windows: dibujaba un cuadro rectangular alrededor del panel redondeado.
                .shadow(false)
                .resizable(false)
                .skip_taskbar(true)
                .always_on_top(true)
                .visible(false)
                .inner_size(PANEL_SIZE.0, PANEL_SIZE.1)
                .build();
            match built {
                Ok(w) => {
                    if let Some(h) = hwnd_of(&w) {
                        appbar::make_tool_window(h);
                    }
                    let app2 = app.clone();
                    w.on_window_event(move |ev| {
                        if let WindowEvent::Focused(false) = ev {
                            eprintln!("[wm] el panel perdió el foco: se oculta");
                            begin_hide(&app2);
                            *PANEL_HID_AT.lock().unwrap() = Some(Instant::now());
                            // La barra de tareas que se abrió junto al panel se cierra con él.
                            if let Some(id) = crate::interface::shell::cursor_monitor() {
                                crate::interface::taskbar::hide_unless_hovered(&app2, id);
                            }
                        }
                    });
                    w
                }
                Err(e) => {
                    eprintln!("[wm] no se pudo crear el panel: {e}");
                    return;
                }
            }
        }
    };
    let s = m.dpi_scale;
    let (w, h) = ((PANEL_SIZE.0 * s) as u32, (PANEL_SIZE.1 * s) as u32);
    let margin = (PANEL_MARGIN * s) as i32;
    // Justo debajo de la barra (el área útil ya empieza ahí) y centrado en el monitor.
    let x = m.work_area.x + (m.work_area.w - w as i32) / 2;
    let y = m.work_area.y + margin;
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    // Con el panel aún oculto (si se reabre mientras se cierra sigue en pantalla y se conserva la copia anterior)
    // se guarda lo que hay detrás: la vista lo dibuja desenfocado bajo su tinte. Se guarda siempre (aunque el desenfoque
    // esté a cero): si se sube desde el propio panel, ya abierto, no se podría copiar lo de detrás sin copiar el panel.
    if !window.is_visible().unwrap_or(false) {
        let reach = ((PANEL_MAX_HEIGHT * s) as i32).min(m.work_area.bottom() - y).max(h as i32);
        crate::infrastructure::windows::backdrop::remember_panel(Rect::new(x, y, w as i32, reach));
    }
    crate::interface::webview::set_visible(&window, true);
    if focus {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // Abierto con el cursor o con la tecla Windows no roba el foco a la app en uso: con el panel como ventana activa,
        // los ganchos de teclado dejaban de recibir la tecla Windows y Windows abría su búsqueda.
        show_without_activating(&window);
    }
    // La ventana se reutiliza: la vista relanza así su animación de aparición.
    let _ = window.emit("wm-panel-shown", ());
}

/// Muestra la ventana sin activarla: durante el `show` lleva el estilo «sin activación» y después se lo quita, para que un
/// clic sobre ella sí la active con normalidad.
fn show_without_activating(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_NOACTIVATE};
    let Some(h) = hwnd_of(window) else {
        let _ = window.show();
        return;
    };
    let hwnd = HWND(h as *mut _);
    unsafe {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE.0 as i32);
        let _ = window.show();
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex & !(WS_EX_NOACTIVATE.0 as i32));
    }
}

/// Cambia el alto del panel (lógico) conservando su borde superior y su centro.
pub fn resize_panel(app: &AppHandle, height: f64) {
    let Some(w) = app.get_webview_window(PANEL_LABEL) else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    let Ok(size) = w.inner_size() else { return };
    let h = (height.clamp(120.0, 900.0) * scale).round() as u32;
    if h != size.height {
        let _ = w.set_size(PhysicalSize::new(size.width, h));
    }
}

/// Cierra el panel (Escape o botón) con su animación.
pub fn hide_panel(app: &AppHandle) {
    begin_hide(app);
}

/// ¿El panel está en pantalla y no se está cerrando?
pub fn panel_open(app: &AppHandle) -> bool {
    !PANEL_CLOSING.load(Ordering::SeqCst)
        && app.get_webview_window(PANEL_LABEL).is_some_and(|w| w.is_visible().unwrap_or(false))
}

/// Empieza el cierre: avisa a la vista para que reproduzca la apertura al revés y, pasado el tiempo de la animación,
/// oculta la ventana. Si el panel se vuelve a abrir mientras tanto, no se oculta.
fn begin_hide(app: &AppHandle) {
    let Some(w) = app.get_webview_window(PANEL_LABEL) else { return };
    if !w.is_visible().unwrap_or(false) || PANEL_CLOSING.swap(true, Ordering::SeqCst) {
        return;
    }
    let generation = PANEL_GEN.load(Ordering::SeqCst);
    let _ = w.emit("wm-panel-closing", ());
    let _ = std::thread::Builder::new().name("wm-panel-close".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(PANEL_CLOSE_MS));
        if PANEL_GEN.load(Ordering::SeqCst) == generation && PANEL_CLOSING.swap(false, Ordering::SeqCst) {
            let _ = w.hide();
            // Oculto no debe consultar el sistema ni animar nada: se duerme su WebView hasta que se vuelva a abrir.
            crate::interface::webview::set_visible(&w, false);
        }
    });
}

fn cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

/// ¿La ventana que hay bajo el cursor es la propia barra? Si hay otra encima (la barra de herramientas de la captura
/// de pantalla, un menú, una app que se sale de su sitio...), el cursor no está «en la barra» y el panel no debe abrirse.
fn bar_is_under_cursor(bar: &tauri::WebviewWindow, x: i32, y: i32) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, WindowFromPoint, GA_ROOT};
    let Some(bar_hwnd) = hwnd_of(bar) else { return false };
    unsafe {
        let hit = WindowFromPoint(POINT { x, y });
        if hit.0.is_null() {
            return false;
        }
        GetAncestor(hit, GA_ROOT).0 as isize == bar_hwnd
    }
}

/// Abre el panel al acercar el cursor al centro de la barra y lo cierra al alejarlo. Solo si la barra de ese
/// monitor está visible (no abre nada sobre un juego o una app a pantalla completa).
pub fn start_hover_trigger(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("wm-panel-hover".into()).spawn(move || {
        let mut dwell = Duration::ZERO;
        let mut armed = true;
        let mut by_hover = false;
        // El cursor ha estado dentro del panel desde que se abrió.
        let mut entered = false;
        let mut outside_since: Option<Instant> = None;
        let tick = Duration::from_millis(40);
        loop {
            std::thread::sleep(tick);
            let Some((cx, cy)) = cursor_pos() else { continue };
            let monitors = list_monitors();
            let zone = monitors.iter().find(|m| {
                let half = (HOVER_HALF_WIDTH * m.dpi_scale) as i32;
                let mid = m.bounds.x + m.bounds.w / 2;
                let bar_h = (BAR_HEIGHT * m.dpi_scale).round() as i32;
                cx >= mid - half
                    && cx <= mid + half
                    && cy >= m.bounds.y
                    && cy < m.bounds.y + bar_h
                    && app
                        .get_webview_window(&bar_label(m.id))
                        .is_some_and(|b| b.is_visible().unwrap_or(false) && bar_is_under_cursor(&b, cx, cy))
            });
            let panel = app.get_webview_window(PANEL_LABEL);
            let visible = panel_open(&app);
            if !visible {
                by_hover = false;
                entered = false;
            }

            if let Some(m) = zone {
                outside_since = None;
                if !armed || visible {
                    dwell = Duration::ZERO;
                    continue;
                }
                dwell += tick;
                if dwell >= HOVER_DWELL {
                    dwell = Duration::ZERO;
                    armed = false;
                    by_hover = true;
                    open_panel_thread(&app, m.id, false);
                }
                continue;
            }
            // Fuera de la zona: se vuelve a armar (así Escape o un cierre no lo reabren mientras el cursor sigue ahí).
            dwell = Duration::ZERO;
            armed = true;
            if visible {
                let inside = panel
                    .as_ref()
                    .and_then(|w| Some((w.outer_position().ok()?, w.outer_size().ok()?)))
                    .is_some_and(|(p, sz)| {
                        let pad = 14;
                        cx >= p.x - pad && cx <= p.x + sz.width as i32 + pad && cy >= p.y - pad && cy <= p.y + sz.height as i32 + pad
                    });
                if inside {
                    outside_since = None;
                    entered = true;
                } else if leave_closes(by_hover, entered, outside_since.get_or_insert_with(Instant::now).elapsed()) {
                    hide_panel(&app);
                    // Si se abrió junto a la barra de tareas (tecla Windows), esta se va con él.
                    if let Some(id) = crate::interface::shell::cursor_monitor() {
                        crate::interface::taskbar::hide_unless_hovered(&app, id);
                    }
                    by_hover = false;
                    entered = false;
                    outside_since = None;
                } else if !(by_hover || entered) {
                    outside_since = None;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_panel_se_cierra_al_alejar_el_cursor_si_lo_abrio_el_cursor_o_ya_se_uso_con_el_raton() {
        let mucho = HOVER_LEAVE;
        let poco = Duration::from_millis(100);
        assert!(leave_closes(true, false, mucho), "abierto por el cursor");
        assert!(leave_closes(false, true, mucho), "abierto con la tecla Windows y usado con el ratón (p. ej. en Ajustes)");
        assert!(!leave_closes(true, true, poco), "no al instante");
        assert!(!leave_closes(false, false, Duration::from_secs(30)), "abierto con el teclado y sin tocarlo con el ratón");
    }
}
