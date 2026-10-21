//! Borde que resalta la ventana en uso: una ventana transparente, sin foco y que ignora el ratón, colocada
//! alrededor de la ventana en primer plano. Su aspecto (blanco, palpitante, con un destello que recorre el
//! contorno) lo dibuja la vista `?view=border`.
//!
//! Para que no vaya «detrás» de la ventana al arrastrarla o cambiarle el tamaño, no se limita a mirar cada 60 ms: un gancho de
//! eventos de Windows avisa en cuanto la ventana en primer plano cambia de sitio o de tamaño (o cambia el foco) y el borde se coloca
//! en ese mismo instante. El sondeo de 60 ms queda como red de seguridad.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER};

use crate::domain::geometry::Rect;
use crate::infrastructure::windows::{appbar, facts};

const LABEL: &str = "focus-border";
/// Margen que el borde deja por fuera de la ventana, en píxeles físicos.
const OUTSET: i32 = 4;
/// ¿Se dibuja el borde? Lo apaga «Sin marco» en Personalización: sin borde la ventana ni se muestra ni se mueve.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Aviso del gancho de eventos al hilo del borde: «mira otra vez dónde está la ventana».
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

fn wake() {
    *WAKE.0.lock().unwrap() = true;
    WAKE.1.notify_one();
}

/// Espera el aviso del gancho (o, como mucho, el tiempo del sondeo de seguridad).
fn wait_for_change(timeout: Duration) {
    let mut flag = WAKE.0.lock().unwrap();
    if !*flag {
        flag = WAKE.1.wait_timeout(flag, timeout).unwrap().0;
    }
    *flag = false;
}

/// Cambios que importan al borde: el foco (y el principio y el fin de arrastrar o redimensionar) y, de las ventanas, solo la que está
/// en primer plano cuando cambia de sitio o de tamaño.
unsafe extern "system" fn on_window_event(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, EVENT_OBJECT_LOCATIONCHANGE, OBJID_WINDOW};
    if event == EVENT_OBJECT_LOCATIONCHANGE && (id_object != OBJID_WINDOW.0 || hwnd != GetForegroundWindow()) {
        return;
    }
    wake();
}

/// Hilo con el gancho de eventos (necesita su propio bucle de mensajes).
fn start_event_hook() {
    use windows::Win32::UI::Accessibility::SetWinEventHook;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MOVESIZEEND, MSG,
        WINEVENT_OUTOFCONTEXT,
    };
    let _ = std::thread::Builder::new().name("wm-focus-border-hook".into()).spawn(|| unsafe {
        let focus = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MOVESIZEEND, None, Some(on_window_event), 0, 0, WINEVENT_OUTOFCONTEXT);
        let location = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, None, Some(on_window_event), 0, 0, WINEVENT_OUTOFCONTEXT);
        if focus.0.is_null() || location.0.is_null() {
            eprintln!("[wm] no se pudo vigilar el movimiento de las ventanas: el borde irá con el sondeo de siempre");
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

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
    start_event_hook();
    let app = app.clone();
    let _ = std::thread::Builder::new().name("wm-focus-border".into()).spawn(move || {
        let mut window: Option<tauri::WebviewWindow> = None;
        let mut last: Option<Rect> = None;
        let mut monitor: Option<usize> = None;
        loop {
            wait_for_change(Duration::from_millis(60));
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
                    // Si pasó a otro monitor, su vista toma ya el color del fondo de ese monitor (sin esperar al sondeo).
                    let now = facts::monitor_index_at(r.x + r.w / 2, r.y + r.h / 2);
                    if now != monitor {
                        monitor = now;
                        let _ = w.emit("wm-border-monitor", now);
                    }
                }
                _ => {}
            }
            // Un arrastre avisa muchas veces por fotograma: no hace falta recolocar más de una vez cada ~8 ms.
            std::thread::sleep(Duration::from_millis(8));
        }
    });
}
