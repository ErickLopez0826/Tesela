//! Borde que resalta la ventana en uso: una ventana transparente, sin foco y que ignora el ratón, colocada
//! alrededor de la ventana en primer plano. Su aspecto (blanco, palpitante, con un destello que recorre el
//! contorno) lo dibuja la vista `?view=border`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER};

use crate::domain::geometry::Rect;
use crate::infrastructure::windows::{appbar, facts};

const LABEL: &str = "focus-border";
/// Margen que el borde deja por fuera de la ventana, en píxeles físicos.
const OUTSET: i32 = 4;
/// ¿Se dibuja el borde? Lo apaga «Sin marco» en Personalización: sin borde la ventana ni se muestra ni se mueve.
static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

fn hwnd_of(w: &tauri::WebviewWindow) -> Option<isize> {
    w.hwnd().ok().map(|h| h.0 as isize)
}

fn create(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let url = WebviewUrl::App("index.html?view=border".into());
    let w = WebviewWindowBuilder::new(app, LABEL, url)
        .title("Tesela Border")
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(200.0, 200.0)
        .build()
        .map_err(|e| eprintln!("[wm] no se pudo crear el borde de la ventana activa: {e}"))
        .ok()?;
    let _ = w.set_ignore_cursor_events(true);
    if let Some(h) = hwnd_of(&w) {
        appbar::make_tool_window(h);
    }
    Some(w)
}

/// Mueve y cambia el tamaño de la ventana en UNA sola llamada. Con `set_position` y `set_size` por separado había un
/// instante en que la ventana ya estaba en su sitio nuevo pero con el tamaño viejo (el borde pequeño «a medias»).
fn place(w: &tauri::WebviewWindow, r: Rect) -> bool {
    let Some(h) = hwnd_of(w) else { return false };
    unsafe {
        SetWindowPos(
            HWND(h as *mut _),
            None,
            r.x - OUTSET,
            r.y - OUTSET,
            r.w + 2 * OUTSET,
            r.h + 2 * OUTSET,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
        .is_ok()
    }
}

/// Arranca el hilo que mantiene el borde sobre la ventana en primer plano.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("wm-focus-border".into()).spawn(move || {
        let mut window: Option<tauri::WebviewWindow> = None;
        let mut last: Option<Rect> = None;
        loop {
            std::thread::sleep(Duration::from_millis(60));
            let target = if ENABLED.load(Ordering::SeqCst) { facts::highlight_target() } else { None };
            if window.is_none() && target.is_some() {
                window = app.get_webview_window(LABEL).or_else(|| create(&app));
            }
            let Some(w) = &window else { continue };
            match target {
                None => {
                    if last.take().is_some() {
                        let _ = w.hide();
                        // Oculto no debe animar ni repintar nada.
                        crate::interface::webview::set_visible(w, false);
                    }
                }
                Some(r) if last != Some(r) => {
                    let first = last.is_none();
                    place(w, r);
                    if first {
                        crate::interface::webview::set_visible(w, true);
                        let _ = w.show();
                    }
                    last = Some(r);
                }
                _ => {}
            }
        }
    });
}
