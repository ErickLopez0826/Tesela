//! Ventanas de la barra (una por monitor, acopladas como AppBar) y del panel desplegable (§9).

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::domain::desktop::Monitor;
use crate::infrastructure::windows::appbar;
use crate::infrastructure::windows::facts::list_monitors;

/// Alto de la barra en píxeles lógicos (se escala con el DPI de cada monitor).
pub const BAR_HEIGHT: f64 = 40.0;
const PANEL_SIZE: (f64, f64) = (420.0, 780.0);
const PANEL_MARGIN: f64 = 12.0;
const PANEL_LABEL: &str = "panel";

/// Cuándo se ocultó el panel por perder el foco: evita que el clic en el botón de la barra lo reabra al instante.
static PANEL_HID_AT: Mutex<Option<Instant>> = Mutex::new(None);

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
                    .title("Window Manager Bar")
                    .decorations(false)
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
}

/// Muestra u oculta la barra de un monitor (pantalla completa, §6.4) sin quitarle el foco a nadie.
pub fn set_bar_visible(app: &AppHandle, monitor: u32, visible: bool) {
    if let Some(h) = app.get_webview_window(&bar_label(monitor)).as_ref().and_then(hwnd_of) {
        appbar::set_visible(h, visible);
    }
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
    // Crear una ventana desde el hilo principal bloquea el bucle de eventos (el webview necesita ese
    // mismo hilo para terminar de crearse), así que se hace desde un hilo aparte.
    let handle = app.clone();
    let _ = std::thread::Builder::new().name("wm-panel".into()).spawn(move || toggle_panel_inner(&handle, monitor));
}

fn toggle_panel_inner(app: &AppHandle, monitor: u32) {
    eprintln!("[wm] alternar panel en el monitor {monitor}");
    let existing = app.get_webview_window(PANEL_LABEL);
    if let Some(w) = &existing {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
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
                .title("Window Manager Panel")
                .decorations(false)
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
                    let w2 = w.clone();
                    w.on_window_event(move |ev| {
                        if let WindowEvent::Focused(false) = ev {
                            eprintln!("[wm] el panel perdió el foco: se oculta");
                            let _ = w2.hide();
                            *PANEL_HID_AT.lock().unwrap() = Some(Instant::now());
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
    // Justo debajo de la barra (el área útil ya empieza ahí) y pegado a la derecha del monitor.
    let x = m.bounds.right() - w as i32 - margin;
    let y = m.work_area.y + margin;
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
}

/// Cierra el panel (Escape o botón).
pub fn hide_panel(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PANEL_LABEL) {
        let _ = w.hide();
    }
}
