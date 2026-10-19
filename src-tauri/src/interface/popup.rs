//! Ventana emergente con las acciones de una aplicación en segundo plano (silenciar, cerrar, acciones propias…).
//!
//! Va aparte de la barra de tareas para no ensancharla: es una ventana más, por encima de todo, que se coloca junto al botón
//! «⋯» de la tarjeta. Como la barra, no toma el foco (un WebView de Tesela con foco de teclado deja sordos los ganchos de la
//! tecla Windows), así que se cierra por su cuenta: al actuar, al volver a pulsar su botón, al cerrarse la barra o al hacer
//! clic fuera (un hilo mira el botón del ratón mientras está abierta).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::domain::geometry::Rect;
use crate::infrastructure::windows::appbar;
use crate::infrastructure::windows::facts::list_monitors;

pub const LABEL: &str = "popup";
/// Tamaño lógico mínimo (la vista pide el real al medir su contenido).
const MIN_SIZE: (f64, f64) = (200.0, 80.0);
/// Separación lógica entre el botón y la ventana.
const GAP: f64 = 6.0;
/// Lo que tarda la animación de cierre de la vista (algo más, por si el sistema va justo).
const CLOSE_MS: u64 = 460;

static OPEN: AtomicBool = AtomicBool::new(false);
static GEN: AtomicU64 = AtomicU64::new(0);
static WATCHER: std::sync::Once = std::sync::Once::new();

#[derive(Clone)]
struct Current {
    exe: String,
    /// Botón que la abrió (físico), para no cerrarla con el clic que la abre.
    anchor: Rect,
    /// Tamaño lógico que pidió la vista.
    size: (f64, f64),
}

static CURRENT: Mutex<Option<Current>> = Mutex::new(None);

#[derive(Serialize, Clone)]
struct OpenPayload<'a> {
    exe: &'a str,
    name: &'a str,
    /// Ruta del ejecutable, solo para pedir su icono (el núcleo la comprueba contra su lista al servirlo).
    path: &'a str,
}

/// Dónde va la ventana: con su borde derecho alineado con el del botón y debajo de él; si no cabe por abajo, encima; siempre
/// dentro del área útil. Pura, para poder probarla.
pub fn popup_rect(anchor: Rect, size: (i32, i32), work: Rect, gap: i32) -> Rect {
    let (w, h) = (size.0.min(work.w), size.1.min(work.h));
    let mut x = anchor.right() - w;
    x = x.min(work.right() - w).max(work.x);
    let below = anchor.bottom() + gap;
    let mut y = if below + h <= work.bottom() { below } else { anchor.y - gap - h };
    y = y.min(work.bottom() - h).max(work.y);
    Rect::new(x, y, w, h)
}

fn hwnd_of(w: &tauri::WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}

fn place(app: &AppHandle) {
    let Some(cur) = CURRENT.lock().unwrap().clone() else { return };
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let cx = cur.anchor.x + cur.anchor.w / 2;
    let cy = cur.anchor.y + cur.anchor.h / 2;
    let monitors = list_monitors();
    let Some(m) = monitors.iter().find(|m| cx >= m.bounds.x && cx < m.bounds.right() && cy >= m.bounds.y && cy < m.bounds.bottom()).or(monitors.first()) else {
        return;
    };
    let scale = m.dpi_scale;
    let size = ((cur.size.0 * scale).round() as i32, (cur.size.1 * scale).round() as i32);
    let r = popup_rect(cur.anchor, size, m.bounds, (GAP * scale).round() as i32);
    if let Some(h) = hwnd_of(&window) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE};
        unsafe {
            let _ = SetWindowPos(HWND(h as *mut _), Some(HWND_TOPMOST), r.x, r.y, r.w, r.h, SWP_NOACTIVATE);
        }
    }
}

/// Crea la ventana (oculta) si no existe. Debe llamarse desde un hilo que no sea el principal.
pub fn ensure(app: &AppHandle) {
    if app.get_webview_window(LABEL).is_some() {
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=popup".into()))
        .title("Tesela Popup")
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .build();
    match built {
        Ok(w) => {
            if let Some(h) = hwnd_of(&w) {
                appbar::make_tool_window(h);
            }
            crate::interface::webview::set_visible(&w, false);
        }
        Err(e) => eprintln!("[wm] no se pudo crear la ventana emergente: {e}"),
    }
}

pub fn remove(app: &AppHandle) {
    close_now(app);
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
}

pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// Abre la ventana junto a `anchor` (rectángulo físico del botón) para `exe`; si ya estaba abierta para esa misma app, la cierra.
pub fn open(app: &AppHandle, anchor: Rect, exe: &str, name: &str, path: &str) {
    if is_open() && CURRENT.lock().unwrap().as_ref().is_some_and(|c| c.exe == exe) {
        close(app);
        return;
    }
    let Some(window) = app.get_webview_window(LABEL) else { return };
    GEN.fetch_add(1, Ordering::SeqCst);
    let size = CURRENT.lock().unwrap().as_ref().map_or(MIN_SIZE, |c| c.size);
    *CURRENT.lock().unwrap() = Some(Current { exe: exe.to_string(), anchor, size });
    place(app);
    OPEN.store(true, Ordering::SeqCst);
    crate::interface::webview::set_visible(&window, true);
    if let Some(h) = hwnd_of(&window) {
        appbar::set_visible(h, true);
    }
    let _ = window.emit("wm-popup-open", OpenPayload { exe, name, path });
    start_watcher(app);
}

/// La vista pide el tamaño (lógico) que necesita su contenido.
pub fn resize(app: &AppHandle, width: f64, height: f64) {
    if !width.is_finite() || !height.is_finite() {
        return;
    }
    if let Some(cur) = CURRENT.lock().unwrap().as_mut() {
        cur.size = (width.clamp(MIN_SIZE.0, 900.0), height.clamp(MIN_SIZE.1, 900.0));
    }
    place(app);
}

/// Cierra la ventana con su animación y, pasado el tiempo, la oculta (salvo que se haya vuelto a abrir).
pub fn close(app: &AppHandle) {
    if !OPEN.swap(false, Ordering::SeqCst) {
        return;
    }
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let _ = window.emit("wm-popup-closing", ());
    let generation = GEN.load(Ordering::SeqCst);
    let _ = std::thread::Builder::new().name("wm-popup-close".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(CLOSE_MS));
        if GEN.load(Ordering::SeqCst) == generation && !is_open() {
            if let Some(h) = hwnd_of(&window) {
                appbar::set_visible(h, false);
            }
            crate::interface::webview::set_visible(&window, false);
        }
    });
}

/// Oculta la ventana ya, sin animación (al destruirla).
fn close_now(app: &AppHandle) {
    OPEN.store(false, Ordering::SeqCst);
    GEN.fetch_add(1, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Some(h) = hwnd_of(&window) {
            appbar::set_visible(h, false);
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

fn inside(r: &Rect, x: i32, y: i32) -> bool {
    x >= r.x && x < r.right() && y >= r.y && y < r.bottom()
}

/// Mientras la ventana está abierta, un clic fuera de ella (y fuera del botón que la abrió) la cierra.
fn start_watcher(app: &AppHandle) {
    let app = app.clone();
    WATCHER.call_once(move || {
        let _ = std::thread::Builder::new().name("wm-popup-outside".into()).spawn(move || {
            use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
            loop {
                std::thread::sleep(Duration::from_millis(40));
                if !is_open() {
                    continue;
                }
                let pressed = unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) | GetAsyncKeyState(VK_RBUTTON.0 as i32)) as u16 & 0x8000 != 0 };
                if !pressed {
                    continue;
                }
                let (Some((x, y)), Some(cur), Some(window)) = (cursor_pos(), CURRENT.lock().unwrap().clone(), app.get_webview_window(LABEL)) else {
                    continue;
                };
                let own = match (window.outer_position(), window.outer_size()) {
                    (Ok(p), Ok(s)) => Rect::new(p.x, p.y, s.width as i32, s.height as i32),
                    _ => continue,
                };
                if !inside(&own, x, y) && !inside(&cur.anchor, x, y) {
                    close(&app);
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect { x: 0, y: 0, w: 1920, h: 1040 };

    #[test]
    fn por_defecto_va_debajo_del_boton_con_el_borde_derecho_alineado() {
        let anchor = Rect::new(900, 300, 26, 26);
        let r = popup_rect(anchor, (220, 160), WORK, 6);
        assert_eq!((r.x, r.y, r.w, r.h), (926 - 220, 332, 220, 160));
    }

    #[test]
    fn si_no_cabe_por_abajo_se_pone_encima() {
        let anchor = Rect::new(900, 980, 26, 26);
        let r = popup_rect(anchor, (220, 160), WORK, 6);
        assert_eq!(r.bottom(), 980 - 6);
        assert!(r.y >= 0);
    }

    #[test]
    fn se_recorta_a_los_lados_y_nunca_sale_del_area_util() {
        let izquierda = popup_rect(Rect::new(10, 100, 26, 26), (220, 160), WORK, 6);
        assert_eq!(izquierda.x, 0);
        let derecha = popup_rect(Rect::new(1900, 100, 26, 26), (220, 160), WORK, 6);
        assert_eq!(derecha.right(), 1920);
        let enorme = popup_rect(Rect::new(500, 500, 26, 26), (5000, 5000), WORK, 6);
        assert!(enorme.w <= WORK.w && enorme.h <= WORK.h && enorme.x >= 0 && enorme.y >= 0);
    }

    #[test]
    fn en_otro_monitor_respeta_su_area() {
        let work = Rect::new(1920, 0, 2560, 1400);
        let r = popup_rect(Rect::new(4470, 1300, 26, 26), (300, 200), work, 9);
        assert!(r.x >= work.x && r.right() <= work.right() && r.y >= work.y && r.bottom() <= work.bottom());
    }
}
