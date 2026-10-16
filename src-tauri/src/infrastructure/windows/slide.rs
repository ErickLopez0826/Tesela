//! Animación del cambio de workspace: las ventanas del que se deja suben y salen por arriba mientras las del que se abre
//! bajan desde arriba hasta su sitio.
//!
//! Las ventanas reales no se mueven fotograma a fotograma (irían a tirones y se saldrían a un monitor vecino). En su
//! lugar se dibujan **miniaturas de DWM** de cada ventana dentro de una capa transparente que cubre solo el área útil del
//! monitor: se recorta sola en sus bordes y la mueve la tarjeta gráfica. Las ventanas que salen se ocultan con la capa
//! ya pintada (la miniatura toma el relevo en el mismo sitio, sin parpadeo); las que entran siguen ocultas hasta que
//! termina, y entonces el controlador las muestra y llama a `Slide::end`, que retira la capa.
//!
//! Si algo falla (no se puede registrar una miniatura, ocultar una ventana...), se deshace todo y `begin` devuelve
//! `None`: el cambio de workspace se hace sin animación, como siempre.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmQueryThumbnailSourceSize, DwmRegisterThumbnail, DwmSetWindowAttribute,
    DwmUnregisterThumbnail, DwmUpdateThumbnailProperties, DWMWA_CLOAK, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
    DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
};
use windows::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW, RegisterClassW, ShowWindow,
    TranslateMessage, HTTRANSPARENT, MSG, PM_REMOVE, SW_SHOWNA, WM_NCHITTEST, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::domain::effects::WindowId;
use crate::domain::geometry::Rect;

const CLASS: PCWSTR = w!("WMWorkspaceSlide");
/// Lo que dura el deslizamiento (200 ms: rápido, pero todavía se ve el movimiento).
pub const DURATION: Duration = Duration::from_millis(200);
/// Tiempo para que DWM componga las miniaturas en su sitio antes de ocultar las ventanas reales.
const SETTLE: Duration = Duration::from_millis(30);
/// Lo máximo que la capa espera la orden de cierre antes de retirarse sola.
const MAX_HOLD: Duration = Duration::from_millis(1500);

// ---------------------------------------------------------------- lógica pura

/// Curva de la animación: arranca suave, acelera y frena suave (cúbica).
pub fn ease_in_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Avance (0 a 1, ya con la curva) tras `elapsed`.
pub fn progress(elapsed: Duration) -> f64 {
    ease_in_out(elapsed.as_secs_f64() / DURATION.as_secs_f64())
}

/// Borde superior de una ventana que sale: sube tanto como mide el área.
pub fn out_top(top: i32, area_h: i32, p: f64) -> i32 {
    top - (area_h as f64 * p).round() as i32
}

/// Borde superior de una ventana que entra: empieza un área entera más arriba y baja hasta su sitio.
pub fn in_top(top: i32, area_h: i32, p: f64) -> i32 {
    top - (area_h as f64 * (1.0 - p)).round() as i32
}

// ---------------------------------------------------------------- Windows

fn cloak(id: WindowId, on: bool) -> bool {
    let value = BOOL(on as i32);
    unsafe { DwmSetWindowAttribute(HWND(id as *mut _), DWMWA_CLOAK, &value as *const _ as *const _, size_of::<BOOL>() as u32).is_ok() }
}

struct Thumb {
    handle: isize,
    /// Rectángulo final de la ventana, relativo a la capa.
    rect: Rect,
    leaving: bool,
}

fn set_thumb(thumb: &Thumb, top: i32) -> bool {
    let props = DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
        rcDestination: RECT { left: thumb.rect.x, top, right: thumb.rect.x + thumb.rect.w, bottom: top + thumb.rect.h },
        opacity: 255,
        fVisible: true.into(),
        fSourceClientAreaOnly: false.into(),
        ..Default::default()
    };
    unsafe { DwmUpdateThumbnailProperties(thumb.handle, &props).is_ok() }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Que los clics atraviesen la capa.
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn register_class() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        // Fondo negro: con el vidrio extendido a toda la ventana, lo negro es transparente.
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: CLASS,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            ..Default::default()
        };
        RegisterClassW(&class);
    });
}

fn pump() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

enum Status {
    /// La capa está puesta y las ventanas viejas ocultas (o no se pudo).
    Ready(bool),
    /// La animación llegó al final.
    Finished,
}

/// Una animación en marcha. `end` la retira.
pub struct Slide {
    close: Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Slide {
    /// Retira la capa (llamar con las ventanas reales ya en su sitio).
    pub fn end(mut self) {
        let _ = self.close.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Slide {
    fn drop(&mut self) {
        let _ = self.close.send(());
    }
}

/// Empieza la animación y **vuelve cuando termina**. `out` y `inn` son las ventanas con su rectángulo final.
pub fn begin(area: Rect, out: Vec<(WindowId, Rect)>, inn: Vec<(WindowId, Rect)>) -> Option<Slide> {
    if out.is_empty() && inn.is_empty() || area.w <= 0 || area.h <= 0 {
        return None;
    }
    let (status_tx, status_rx) = channel();
    let (close_tx, close_rx) = channel();
    let thread = std::thread::Builder::new()
        .name("wm-workspace-slide".into())
        .spawn(move || run(area, out, inn, status_tx, close_rx))
        .ok()?;
    let slide = Slide { close: close_tx, thread: Some(thread) };
    match status_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Status::Ready(true)) => {}
        _ => {
            slide.end();
            return None;
        }
    }
    // Espera al final de la animación (con holgura por si el sistema va justo).
    match status_rx.recv_timeout(DURATION + Duration::from_secs(1)) {
        Ok(Status::Finished) => Some(slide),
        _ => {
            slide.end();
            None
        }
    }
}

fn run(area: Rect, out: Vec<(WindowId, Rect)>, inn: Vec<(WindowId, Rect)>, status: Sender<Status>, close: Receiver<()>) {
    unsafe {
        register_class();
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            CLASS,
            w!(""),
            WS_POPUP,
            area.x,
            area.y,
            area.w,
            area.h,
            None,
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[wm] transición de workspace: no se pudo crear la capa: {}", e.message());
                let _ = status.send(Status::Ready(false));
                return;
            }
        };
        let glass = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &glass);

        // Una miniatura por ventana, en su sitio de partida.
        let mut thumbs: Vec<Thumb> = Vec::new();
        let mut ok = true;
        for (list, leaving) in [(&out, true), (&inn, false)] {
            for (id, rect) in list {
                let rel = Rect::new(rect.x - area.x, rect.y - area.y, rect.w, rect.h);
                match DwmRegisterThumbnail(hwnd, HWND(*id as *mut _)) {
                    Ok(handle) => {
                        // Si la ventana no tiene tamaño de origen, no hay nada que dibujar.
                        if DwmQueryThumbnailSourceSize(handle).map(|s| s.cx <= 0 || s.cy <= 0).unwrap_or(true) {
                            let _ = DwmUnregisterThumbnail(handle);
                            continue;
                        }
                        thumbs.push(Thumb { handle, rect: rel, leaving });
                    }
                    Err(_) => ok = false,
                }
            }
        }
        let start_top = |t: &Thumb| if t.leaving { t.rect.y } else { in_top(t.rect.y, area.h, 0.0) };
        ok &= thumbs.iter().all(|t| set_thumb(t, start_top(t)));

        let teardown = |thumbs: &[Thumb]| {
            for t in thumbs {
                let _ = DwmUnregisterThumbnail(t.handle);
            }
            let _ = DestroyWindow(hwnd);
        };
        if !ok || thumbs.is_empty() {
            teardown(&thumbs);
            let _ = status.send(Status::Ready(false));
            return;
        }

        // La capa se pinta con las ventanas viejas aún a la vista y, una vez compuesta, se ocultan las reales.
        let _ = ShowWindow(hwnd, SW_SHOWNA);
        let settle = Instant::now();
        while settle.elapsed() < SETTLE {
            pump();
            std::thread::sleep(Duration::from_millis(4));
        }
        let mut cloaked: Vec<WindowId> = Vec::new();
        for (id, _) in &out {
            if cloak(*id, true) {
                cloaked.push(*id);
            } else {
                // No se puede ocultar una: se deshace todo y no hay animación.
                for done in &cloaked {
                    cloak(*done, false);
                }
                teardown(&thumbs);
                let _ = status.send(Status::Ready(false));
                return;
            }
        }
        let _ = status.send(Status::Ready(true));

        // El deslizamiento.
        let anim = Instant::now();
        loop {
            let elapsed = anim.elapsed();
            let p = progress(elapsed);
            for t in &thumbs {
                let top = if t.leaving { out_top(t.rect.y, area.h, p) } else { in_top(t.rect.y, area.h, p) };
                set_thumb(t, top);
            }
            pump();
            if elapsed >= DURATION {
                break;
            }
            std::thread::sleep(Duration::from_millis(6));
        }
        let _ = status.send(Status::Finished);

        // Se queda con las ventanas nuevas dibujadas hasta que el controlador las muestra.
        let hold = Instant::now();
        while hold.elapsed() < MAX_HOLD {
            pump();
            match close.recv_timeout(Duration::from_millis(10)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        teardown(&thumbs);
    }
}

/// ¿Se puede registrar una miniatura de esa ventana y tiene tamaño? Sirve para comprobar que funcionan con ventanas
/// ocultas (`cloak`), de lo que depende toda la animación. Devuelve el tamaño de origen.
pub fn thumbnail_size(dest: HWND, source: HWND) -> Option<(i32, i32)> {
    unsafe {
        let handle = DwmRegisterThumbnail(dest, source).ok()?;
        let size = DwmQueryThumbnailSourceSize(handle).ok();
        let _ = DwmUnregisterThumbnail(handle);
        size.map(|s| (s.cx, s.cy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_curva_va_de_cero_a_uno_y_siempre_sube() {
        assert_eq!(ease_in_out(0.0), 0.0);
        assert_eq!(ease_in_out(1.0), 1.0);
        assert_eq!(ease_in_out(-3.0), 0.0);
        assert_eq!(ease_in_out(9.0), 1.0);
        assert!((ease_in_out(0.5) - 0.5).abs() < 1e-9);
        let mut last = -1.0;
        for i in 0..=100 {
            let v = ease_in_out(i as f64 / 100.0);
            assert!(v >= last);
            last = v;
        }
        assert!(ease_in_out(0.1) < 0.1, "arranca suave");
    }

    #[test]
    fn el_progreso_llega_a_uno_justo_al_acabar() {
        assert_eq!(progress(Duration::ZERO), 0.0);
        assert_eq!(progress(DURATION), 1.0);
        assert_eq!(progress(DURATION * 4), 1.0);
    }

    #[test]
    fn las_que_salen_suben_un_area_entera_y_las_que_entran_acaban_en_su_sitio() {
        // Una ventana en y=100 de un área de 1000 de alto.
        assert_eq!(out_top(100, 1000, 0.0), 100);
        assert_eq!(out_top(100, 1000, 1.0), -900);
        assert_eq!(in_top(100, 1000, 0.0), -900, "empieza un área más arriba");
        assert_eq!(in_top(100, 1000, 1.0), 100, "termina en su sitio");
        // Al salir, la ventana más baja posible (borde inferior = alto del área) queda fuera por arriba.
        let (top, h) = (400, 600);
        assert!(out_top(top, 1000, 1.0) + h <= 0);
        // A mitad de camino se cruzan: la que sale y la que entra pasan por el mismo sitio.
        assert_eq!(out_top(100, 1000, 0.5), in_top(100, 1000, 0.5));
    }
}
