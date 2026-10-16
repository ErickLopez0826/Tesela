//! Transición al cambiar de fondo de pantalla, dibujada por la app y estilo «grow» de Hyprland / CachyOS: la imagen del
//! fondo nuevo se abre como un círculo desde el cursor, se queda hasta que Wallpaper Engine ya tiene el fondo puesto y
//! entonces se desvanece.
//!
//! La capa es una ventana propia en el fondo del orden de ventanas (sin marco, sin foco, transparente a los clics): queda
//! encima del escritorio y del dibujo de Wallpaper Engine, pero debajo de cualquier ventana normal. Se comprobó que una
//! ventana hija de la que aloja el fondo NO se ve: Wallpaper Engine dibuja con DirectX y esa superficie queda por
//! encima de las ventanas hermanas, tengan el orden que tengan. Durante la transición tapa también los iconos del
//! escritorio. Si algo falla no se dibuja nada (el cambio de fondo se hace igual, sin efecto).
//!
//! No se puede mostrar el fondo nuevo «de verdad» antes de que Wallpaper Engine lo aplique (escenas y webs se
//! renderizan en vivo), así que se usa su previsualización; el fondo viejo que se ve debajo sí es el real.

use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::{w, GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::{COLORREF, GENERIC_READ, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CombineRgn, CreateCompatibleDC, CreateDIBSection, CreateEllipticRgn, CreateRectRgn, DeleteDC,
    DeleteObject, EndPaint, RGN_DIFF, SelectObject, SetWindowRgn, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ, PAINTSTRUCT, SRCCOPY,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand, WICRect,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumChildWindows, EnumWindows, GetClassNameW,
    GetParent, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, PeekMessageW, RegisterClassW,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, GWLP_USERDATA, GWL_EXSTYLE,
    HTTRANSPARENT, LWA_ALPHA, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNA, HWND_BOTTOM, WM_ERASEBKGND,
    WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::domain::geometry::Rect;
use crate::infrastructure::windows::facts::exe_name;

const CLASS: PCWSTR = w!("WMWallpaperFade");
/// Lo que tarda el círculo en abrirse.
pub const GROW: Duration = Duration::from_millis(900);
/// Tiempo extra con la capa puesta, tras ver el fondo nuevo en la configuración de Wallpaper Engine, para que termine de cargar.
pub const GRACE: Duration = Duration::from_millis(1200);
/// Lo máximo que se espera a que Wallpaper Engine aplique el fondo.
pub const MAX_WAIT: Duration = Duration::from_secs(6);
pub const FADE_OUT: Duration = Duration::from_millis(350);

// ---------------------------------------------------------------- lógica pura

/// Curva de apertura: rápida al principio y suave al final.
pub fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Radio del círculo en el instante `elapsed`, para llegar a `max_radius` al cabo de `GROW`.
pub fn radius_at(elapsed: Duration, max_radius: f64) -> f64 {
    ease_out(elapsed.as_secs_f64() / GROW.as_secs_f64()) * max_radius
}

/// Radio que cubre todo el monitor desde `origin` (la esquina más lejana, con un margen).
pub fn full_radius(origin: (i32, i32), w: i32, h: i32) -> f64 {
    let dx = origin.0.max(w - origin.0) as f64;
    let dy = origin.1.max(h - origin.1) as f64;
    (dx * dx + dy * dy).sqrt() + 8.0
}

/// Dónde nace el círculo dentro del monitor: en el cursor si está en él, y si no en el centro.
pub fn origin_in(bounds: &Rect, cursor: Option<(i32, i32)>) -> (i32, i32) {
    match cursor {
        Some((x, y)) if x >= bounds.x && x < bounds.right() && y >= bounds.y && y < bounds.bottom() => (x - bounds.x, y - bounds.y),
        _ => (bounds.w / 2, bounds.h / 2),
    }
}

/// Recorte «cover»: (ancho y alto a escalar, x e y del recorte) para llenar `w`×`h` sin deformar la imagen.
pub fn cover(fw: u32, fh: u32, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let scale = (w as f64 / fw.max(1) as f64).max(h as f64 / fh.max(1) as f64);
    let sw = ((fw as f64 * scale).ceil() as u32).max(w);
    let sh = ((fh as f64 * scale).ceil() as u32).max(h);
    (sw, sh, (sw - w) / 2, (sh - h) / 2)
}

// ---------------------------------------------------------------- ventana que aloja el fondo

/// Nombre de clase de una ventana (para diagnóstico).
pub fn class_name(hwnd: HWND) -> String {
    class_of(hwnd)
}

fn class_of(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn is_engine_window(hwnd: HWND) -> bool {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return false;
    }
    let exe = unsafe { exe_name(pid) }.to_lowercase();
    exe == "wallpaper64.exe" || exe == "wallpaper32.exe"
}

/// La ventana que aloja el fondo: el padre de la ventana de Wallpaper Engine dentro de `Progman` o de un `WorkerW`.
pub fn find_host() -> Option<HWND> {
    find_host_and_engine().map(|(host, _)| host)
}

/// El anfitrión del fondo y la ventana de Wallpaper Engine que hay dentro. La capa se coloca justo encima de ésta, así
/// los iconos del escritorio (que van por encima del fondo) siguen por encima de la capa.
pub fn find_host_and_engine() -> Option<(HWND, HWND)> {
    unsafe extern "system" fn top(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let found = &mut *(lparam.0 as *mut Option<(HWND, HWND)>);
        if found.is_some() {
            return false.into();
        }
        let class = class_of(hwnd);
        if class == "Progman" || class == "WorkerW" {
            unsafe extern "system" fn child(c: HWND, lparam: LPARAM) -> windows::core::BOOL {
                let found = &mut *(lparam.0 as *mut Option<(HWND, HWND)>);
                if found.is_none() && is_engine_window(c) {
                    if let Ok(parent) = GetParent(c) {
                        if !parent.0.is_null() {
                            *found = Some((parent, c));
                            return false.into();
                        }
                    }
                }
                true.into()
            }
            let _ = EnumChildWindows(Some(hwnd), Some(child), lparam);
        }
        true.into()
    }
    let mut found: Option<(HWND, HWND)> = None;
    unsafe {
        let _ = EnumWindows(Some(top), LPARAM(&mut found as *mut _ as isize));
    }
    found
}

// ---------------------------------------------------------------- imagen

/// Imagen de previsualización recortada «cover» al tamaño del monitor, en píxeles BGRA.
fn load_cover(path: &str, w: u32, h: u32) -> Result<Vec<u8>, String> {
    let e = |e: windows::core::Error| e.message();
    unsafe {
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(e)?;
        let decoder = factory
            .CreateDecoderFromFilename(&HSTRING::from(path), None::<*const GUID>, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(e)?;
        let frame = decoder.GetFrame(0).map_err(e)?;
        let (mut fw, mut fh) = (0u32, 0u32);
        frame.GetSize(&mut fw, &mut fh).map_err(e)?;
        let convert = factory.CreateFormatConverter().map_err(e)?;
        convert
            .Initialize(&frame, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)
            .map_err(e)?;
        let (sw, sh, x, y) = cover(fw, fh, w, h);
        let scaler = factory.CreateBitmapScaler().map_err(e)?;
        scaler.Initialize(&convert, sw, sh, WICBitmapInterpolationModeFant).map_err(e)?;
        let clipper = factory.CreateBitmapClipper().map_err(e)?;
        clipper.Initialize(&scaler, &WICRect { X: x as i32, Y: y as i32, Width: w as i32, Height: h as i32 }).map_err(e)?;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        clipper.CopyPixels(std::ptr::null(), w * 4, &mut pixels).map_err(e)?;
        Ok(pixels)
    }
}

// ---------------------------------------------------------------- ventana de la capa

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        // Que los clics atraviesen la capa.
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let bitmap = HBITMAP(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut _);
            if !bitmap.0.is_null() {
                let mem = CreateCompatibleDC(Some(hdc));
                let old = SelectObject(mem, HGDIOBJ(bitmap.0));
                let r = ps.rcPaint;
                let _ = BitBlt(hdc, r.left, r.top, r.right - r.left, r.bottom - r.top, Some(mem), r.left, r.top, SRCCOPY);
                SelectObject(mem, old);
                let _ = DeleteDC(mem);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn register_class() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW { lpfnWndProc: Some(wnd_proc), hInstance: hinst.into(), lpszClassName: CLASS, ..Default::default() };
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

fn set_circle(hwnd: HWND, origin: (i32, i32), radius: f64) {
    let r = radius.round() as i32;
    unsafe {
        let rgn = CreateEllipticRgn(origin.0 - r, origin.1 - r, origin.0 + r, origin.1 + r);
        // El sistema se queda con la región: no se libera aquí.
        SetWindowRgn(hwnd, Some(rgn), true);
    }
}

/// Una capa en un monitor. Todo ocurre en su propio hilo; `ready` avisa de que ya está visible (o de que falló).
fn run_overlay(
    bounds: Rect,
    preview: String,
    origin: (i32, i32),
    applied: Arc<dyn Fn() -> bool + Send + Sync>,
    ready: std::sync::mpsc::Sender<bool>,
) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let (w, h) = (bounds.w.max(1) as u32, bounds.h.max(1) as u32);
    let pixels = match load_cover(&preview, w, h) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[wm] transición de fondo: no se pudo leer {preview}: {e}");
            let _ = ready.send(false);
            return;
        }
    };
    unsafe {
        register_class();
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        // Ventana propia, sin marco, que no roba el foco y deja pasar los clics, del tamaño del monitor.
        let hwnd = match CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
            CLASS,
            w!(""),
            WS_POPUP,
            bounds.x,
            bounds.y,
            w as i32,
            h as i32,
            None,
            None,
            Some(hinst.into()),
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[wm] transición de fondo: no se pudo crear la capa: {}", e.message());
                let _ = ready.send(false);
                return;
            }
        };
        // La imagen, en un mapa de bits de la capa.
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: w as i32, biHeight: -(h as i32), biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) if !bits.is_null() => b,
            _ => {
                let _ = DestroyWindow(hwnd);
                let _ = ready.send(false);
                return;
            }
        };
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, bitmap.0 as isize);

        // Empieza invisible (círculo de radio 0) y en el fondo del orden de ventanas: encima del escritorio y del fondo de
        // Wallpaper Engine, pero debajo de cualquier ventana normal.
        set_circle(hwnd, origin, 0.0);
        let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), bounds.x, bounds.y, w as i32, h as i32, SWP_NOACTIVATE);
        let _ = ShowWindow(hwnd, SW_SHOWNA);
        let _ = ready.send(true);

        // 1) El círculo se abre.
        let max_radius = full_radius(origin, w as i32, h as i32);
        let start = Instant::now();
        while start.elapsed() < GROW {
            set_circle(hwnd, origin, radius_at(start.elapsed(), max_radius));
            let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            pump();
            std::thread::sleep(Duration::from_millis(8));
        }
        SetWindowRgn(hwnd, None, true);

        // 2) Se queda hasta que Wallpaper Engine tiene el fondo puesto (más un margen para que termine de cargar).
        let wait = Instant::now();
        let mut seen: Option<Instant> = None;
        while wait.elapsed() < MAX_WAIT {
            if seen.is_none() && applied() {
                seen = Some(Instant::now());
            }
            if seen.is_some_and(|s| s.elapsed() >= GRACE) {
                break;
            }
            let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            pump();
            std::thread::sleep(Duration::from_millis(60));
        }

        // 3) Se desvanece; si el sistema no deja, desaparece de golpe.
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED.0 as isize);
        if SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).is_ok() {
            let fade = Instant::now();
            while fade.elapsed() < FADE_OUT {
                let left = 1.0 - fade.elapsed().as_secs_f64() / FADE_OUT.as_secs_f64();
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), (left.clamp(0.0, 1.0) * 255.0) as u8, LWA_ALPHA);
                pump();
                std::thread::sleep(Duration::from_millis(12));
            }
        }
        let _ = DestroyWindow(hwnd);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
    }
}

// ---------------------------------------------------------------- modo «fondo real» (dos fondos a la vez)
//
// Se copia el dibujo del fondo de pantalla que hay AHORA (la ventana que aloja el fondo de Wallpaper Engine, con
// `PrintWindow`: da el fotograma completo y a su calidad, sin ventanas encima ni iconos), se pone esa copia a pantalla
// completa y se le pide a Wallpaper Engine el fondo nuevo. Cuando el nuevo ya se ve por debajo, la copia vieja se
// «abre» con un agujero circular que crece desde el cursor y deja ver el fondo nuevo, en vivo y a su calidad real.
// Mientras tanto conviven los dos fondos, y el viejo se queda congelado hasta que el círculo lo cubre.

/// Cuánto se espera, como máximo, a que el fondo nuevo se vea por debajo antes de abrir el círculo igualmente.
pub const MAX_WAIT_FROZEN: Duration = Duration::from_secs(4);
/// Margen tras anotarse el fondo nuevo, por si no se detecta el cambio (p. ej. se vuelve a poner el mismo fondo).
pub const GRACE_FROZEN: Duration = Duration::from_millis(1200);
/// Cada cuánto se mira si el fondo de debajo ya cambió.
const CHECK_EVERY: Duration = Duration::from_millis(120);
/// Diferencia media por canal (0 a 255) a partir de la cual se considera que el fondo cambió.
const CHANGED: f64 = 10.0;

/// Fotograma BGRA del anfitrión del fondo completo.
pub struct Capture {
    pub w: i32,
    pub h: i32,
    pub pixels: Vec<u8>,
}

/// Puntos de una rejilla de `cols`×`rows` repartidos por una imagen de `w`×`h`.
fn grid(w: i32, h: i32, cols: i32, rows: i32) -> impl Iterator<Item = (i32, i32)> {
    (0..rows).flat_map(move |r| (0..cols).map(move |c| ((c * 2 + 1) * w / (cols * 2), (r * 2 + 1) * h / (rows * 2))))
}

fn at(pixels: &[u8], stride_px: i32, x: i32, y: i32) -> [u8; 3] {
    let i = ((y * stride_px + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

/// ¿Es (casi) todo negro? Una captura fallida de una ventana de DirectX sale así.
pub fn is_blank(pixels: &[u8], w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 || pixels.len() < (w * h * 4) as usize {
        return true;
    }
    let dark = grid(w, h, 32, 18).filter(|&(x, y)| at(pixels, w, x, y).iter().map(|&c| c as u32).sum::<u32>() < 12).count();
    dark * 100 >= 32 * 18 * 98
}

impl Capture {
    /// La parte de la captura que cubre `r` (en coordenadas de pantalla; `origin` es la esquina de la captura).
    pub fn crop(&self, origin: (i32, i32), r: &Rect) -> Option<Vec<u8>> {
        let (x0, y0) = (r.x - origin.0, r.y - origin.1);
        if r.w <= 0 || r.h <= 0 || x0 < 0 || y0 < 0 || x0 + r.w > self.w || y0 + r.h > self.h {
            return None;
        }
        let mut out = Vec::with_capacity((r.w * r.h * 4) as usize);
        for row in 0..r.h {
            let start = (((y0 + row) * self.w + x0) * 4) as usize;
            out.extend_from_slice(&self.pixels[start..start + (r.w * 4) as usize]);
        }
        Some(out)
    }

    /// Diferencia media por canal entre `base` (el recorte `r` de antes) y lo que hay ahora en esa zona.
    pub fn diff_in(&self, origin: (i32, i32), r: &Rect, base: &[u8]) -> f64 {
        let (x0, y0) = (r.x - origin.0, r.y - origin.1);
        if x0 < 0 || y0 < 0 || x0 + r.w > self.w || y0 + r.h > self.h || base.len() < (r.w * r.h * 4) as usize {
            return 0.0;
        }
        let (mut total, mut n) = (0u64, 0u64);
        for (x, y) in grid(r.w, r.h, 32, 18) {
            let a = at(base, r.w, x, y);
            let b = at(&self.pixels, self.w, x0 + x, y0 + y);
            total += (0..3).map(|i| (a[i] as i32 - b[i] as i32).unsigned_abs() as u64).sum::<u64>();
            n += 3;
        }
        total as f64 / n.max(1) as f64
    }
}

/// Copia el fotograma actual del anfitrión del fondo, con su esquina en pantalla.
pub fn capture_host(host: HWND) -> Option<((i32, i32), Capture)> {
    let r = rect_of(host)?;
    if r.w <= 0 || r.h <= 0 || r.w > 16384 || r.h > 16384 {
        return None;
    }
    unsafe {
        let mem = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: r.w, biHeight: -r.h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(mem);
            return None;
        };
        let old = SelectObject(mem, HGDIOBJ(bitmap.0));
        // PW_RENDERFULLCONTENT: también las superficies de DirectX (sin esto salen negras).
        let ok = PrintWindow(host, mem, PRINT_WINDOW_FLAGS(2)).as_bool();
        let mut pixels = vec![0u8; (r.w * r.h * 4) as usize];
        if ok && !bits.is_null() {
            std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), pixels.len());
        }
        SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mem);
        ok.then_some(((r.x, r.y), Capture { w: r.w, h: r.h, pixels }))
    }
}

/// Región de la capa: el rectángulo entero menos un círculo (el agujero por el que se ve el fondo nuevo).
fn set_hole(hwnd: HWND, w: i32, h: i32, origin: (i32, i32), radius: f64) {
    let r = radius.round() as i32;
    unsafe {
        let whole = CreateRectRgn(0, 0, w, h);
        let hole = CreateEllipticRgn(origin.0 - r, origin.1 - r, origin.0 + r, origin.1 + r);
        let result = CreateRectRgn(0, 0, 0, 0);
        CombineRgn(Some(result), Some(whole), Some(hole), RGN_DIFF);
        let _ = DeleteObject(HGDIOBJ(whole.0));
        let _ = DeleteObject(HGDIOBJ(hole.0));
        // El sistema se queda con `result`.
        SetWindowRgn(hwnd, Some(result), true);
    }
}

struct Frozen {
    hwnd: HWND,
    bitmap: HBITMAP,
    bounds: Rect,
    origin: (i32, i32),
    base: Vec<u8>,
}

fn run_frozen(host: isize, wins: Vec<(Rect, Vec<u8>)>, cursor: Option<(i32, i32)>, applied: Arc<dyn Fn() -> bool + Send + Sync>, ready: std::sync::mpsc::Sender<bool>) {
    let host_hwnd = HWND(host as *mut _);
    let mut layers: Vec<Frozen> = Vec::new();
    unsafe {
        register_class();
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        for (bounds, pixels) in wins {
            let (w, h) = (bounds.w, bounds.h);
            let Ok(hwnd) = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT, CLASS, w!(""), WS_POPUP, bounds.x, bounds.y, w, h, None, None, Some(hinst.into()), None) else { continue };
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
                ..Default::default()
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(b) if !bits.is_null() => b,
                _ => {
                    let _ = DestroyWindow(hwnd);
                    continue;
                }
            };
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, bitmap.0 as isize);
            let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), bounds.x, bounds.y, w, h, SWP_NOACTIVATE);
            let _ = ShowWindow(hwnd, SW_SHOWNA);
            layers.push(Frozen { hwnd, bitmap, bounds, origin: origin_in(&bounds, cursor), base: pixels });
        }
        if layers.is_empty() {
            let _ = ready.send(false);
            return;
        }
        // Se deja pintar la copia antes de avisar: en cuanto se avisa se pide el fondo nuevo.
        let settle = Instant::now();
        while settle.elapsed() < Duration::from_millis(60) {
            pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = ready.send(true);

        // 1) Esperar a que el fondo nuevo se vea por debajo.
        let start = Instant::now();
        let mut next_check = Instant::now();
        let mut seen: Option<Instant> = None;
        while start.elapsed() < MAX_WAIT_FROZEN {
            for l in &layers {
                let _ = SetWindowPos(l.hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            pump();
            std::thread::sleep(Duration::from_millis(20));
            if Instant::now() < next_check {
                continue;
            }
            next_check = Instant::now() + CHECK_EVERY;
            if seen.is_none() && applied() {
                seen = Some(Instant::now());
            }
            let Some(since) = seen else { continue };
            let changed = capture_host(host_hwnd).is_some_and(|(origin, cap)| layers.iter().any(|l| cap.diff_in(origin, &l.bounds, &l.base) > CHANGED));
            if changed || since.elapsed() >= GRACE_FROZEN {
                break;
            }
        }

        // 2) El círculo se abre desde el cursor y deja ver el fondo nuevo.
        let max_radius = layers.iter().map(|l| full_radius(l.origin, l.bounds.w, l.bounds.h)).fold(0.0, f64::max);
        let anim = Instant::now();
        while anim.elapsed() < GROW {
            let radius = radius_at(anim.elapsed(), max_radius);
            for l in &layers {
                set_hole(l.hwnd, l.bounds.w, l.bounds.h, l.origin, radius);
                let _ = SetWindowPos(l.hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            pump();
            std::thread::sleep(Duration::from_millis(8));
        }
        for l in layers {
            let _ = DestroyWindow(l.hwnd);
            let _ = DeleteObject(HGDIOBJ(l.bitmap.0));
        }
    }
}

/// Intenta la transición con el fondo real: devuelve cuántas capas se pusieron, o `None` si no se pudo copiar el fondo
/// (entonces se usa la previsualización).
fn play_frozen(overlays: &[Overlay], cursor: Option<(i32, i32)>, applied: &Arc<dyn Fn() -> bool + Send + Sync>) -> Option<usize> {
    let (host, _) = find_host_and_engine()?;
    let (origin, cap) = capture_host(host)?;
    let mut wins = Vec::new();
    for o in overlays {
        let crop = cap.crop(origin, &o.bounds)?;
        if is_blank(&crop, o.bounds.w, o.bounds.h) {
            return None;
        }
        wins.push((o.bounds, crop));
    }
    let n = wins.len();
    let (tx, rx) = std::sync::mpsc::channel();
    let applied = applied.clone();
    let host_id = host.0 as isize;
    std::thread::Builder::new().name("wm-wallpaper-frozen".into()).spawn(move || run_frozen(host_id, wins, cursor, applied, tx)).ok()?;
    rx.recv_timeout(Duration::from_millis(3000)).unwrap_or(false).then_some(n)
}

/// Un monitor y la previsualización que se abre sobre él.
pub struct Overlay {
    pub bounds: Rect,
    pub preview: String,
}

/// Empieza la transición en cada monitor y devuelve cuántas capas quedaron visibles. Vuelve en cuanto están puestas;
/// la animación sigue sola en segundo plano. `applied` dice si Wallpaper Engine ya tiene el fondo nuevo.
pub fn play(overlays: Vec<Overlay>, cursor: Option<(i32, i32)>, applied: Arc<dyn Fn() -> bool + Send + Sync>) -> Result<usize, String> {
    // Con el fondo real (dos fondos a la vez); si no se puede copiar, con la previsualización.
    if let Some(n) = play_frozen(&overlays, cursor, &applied) {
        return Ok(n);
    }
    if overlays.iter().any(|o| o.preview.is_empty()) {
        return Err("no se pudo copiar el fondo actual y este fondo no tiene previsualización".into());
    }
    eprintln!("[wm] transición de fondo: no se pudo copiar el fondo actual, se usa la previsualización");
    let mut waiting = Vec::new();
    for o in overlays {
        let (tx, rx) = std::sync::mpsc::channel();
        let origin = origin_in(&o.bounds, cursor);
        let applied = applied.clone();
        let spawned = std::thread::Builder::new()
            .name("wm-wallpaper-fade".into())
            .spawn(move || run_overlay(o.bounds, o.preview, origin, applied, tx));
        if spawned.is_ok() {
            waiting.push(rx);
        }
    }
    let shown = waiting.into_iter().filter(|rx| rx.recv_timeout(Duration::from_millis(1500)).unwrap_or(false)).count();
    if shown == 0 {
        return Err("no se pudo dibujar la transición".into());
    }
    Ok(shown)
}

/// Cuántas capas de transición hay ahora en pantalla (para comprobaciones).
pub fn count_overlays() -> usize {
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        if class_of(hwnd) == "WMWallpaperFade" {
            *(lparam.0 as *mut usize) += 1;
        }
        true.into()
    }
    let mut n = 0usize;
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut n as *mut _ as isize));
    }
    n
}

/// Esquina y tamaño de una ventana (para comprobaciones).
pub fn rect_of(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r).ok()? };
    Some(Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_curva_empieza_en_cero_termina_en_uno_y_siempre_sube() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert_eq!(ease_out(5.0), 1.0);
        let mut last = -1.0;
        for i in 0..=100 {
            let v = ease_out(i as f64 / 100.0);
            assert!(v >= last);
            last = v;
        }
        assert!(ease_out(0.5) > 0.5, "rápida al principio");
    }

    #[test]
    fn el_radio_llega_al_maximo_justo_al_acabar() {
        assert_eq!(radius_at(Duration::ZERO, 1000.0), 0.0);
        assert!((radius_at(GROW, 1000.0) - 1000.0).abs() < 1e-9);
        assert!((radius_at(GROW * 3, 1000.0) - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn el_circulo_final_cubre_todo_el_monitor_desde_cualquier_origen() {
        for origin in [(0, 0), (1920, 1080), (960, 540), (100, 900)] {
            let r = full_radius(origin, 1920, 1080);
            for corner in [(0, 0), (1920, 0), (0, 1080), (1920, 1080)] {
                let d = (((corner.0 - origin.0).pow(2) + (corner.1 - origin.1).pow(2)) as f64).sqrt();
                assert!(r >= d, "origen {origin:?} esquina {corner:?}");
            }
        }
    }

    #[test]
    fn el_origen_es_el_cursor_si_esta_en_el_monitor_y_si_no_el_centro() {
        let m = Rect::new(1920, 0, 1080, 1920);
        assert_eq!(origin_in(&m, Some((2000, 100))), (80, 100));
        assert_eq!(origin_in(&m, Some((100, 100))), (540, 960), "el cursor está en otro monitor");
        assert_eq!(origin_in(&m, None), (540, 960));
    }

    #[test]
    fn el_recorte_cover_llena_el_monitor_sin_deformar() {
        // 16:10 sobre 16:9: se escala por el ancho y se recorta arriba y abajo.
        let (sw, sh, x, y) = cover(1600, 1000, 1920, 1080);
        assert_eq!((sw, x), (1920, 0));
        assert!(sh >= 1080 && y == (sh - 1080) / 2);
        // Vertical sobre apaisado: se escala por el alto.
        let (sw, sh, x, y) = cover(1000, 1600, 1920, 1080);
        assert!(sw >= 1920 && sh >= 1080 && y == (sh - 1080) / 2 && x == (sw - 1920) / 2);
        // Mismas proporciones: sin recorte.
        assert_eq!(cover(960, 540, 1920, 1080), (1920, 1080, 0, 0));
    }

    /// Imagen BGRA de `w`×`h` donde el píxel (x, y) vale `f(x, y)`.
    fn image(w: i32, h: i32, f: impl Fn(i32, i32) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let [b, g, r] = f(x, y);
                v.extend([b, g, r, 255]);
            }
        }
        v
    }

    #[test]
    fn una_captura_negra_se_reconoce_como_fallida_y_una_con_imagen_no() {
        let black = image(640, 360, |_, _| [0, 0, 0]);
        assert!(is_blank(&black, 640, 360));
        let picture = image(640, 360, |x, y| [(x % 256) as u8, (y % 256) as u8, 90]);
        assert!(!is_blank(&picture, 640, 360));
        assert!(is_blank(&[], 640, 360), "sin datos cuenta como fallida");
    }

    #[test]
    fn recortar_devuelve_justo_la_zona_pedida_y_rechaza_lo_que_no_cabe() {
        // Captura de 100×50 con esquina en pantalla (10, -20): el píxel (x, y) vale [x, y, 0].
        let cap = Capture { w: 100, h: 50, pixels: image(100, 50, |x, y| [x as u8, y as u8, 0]) };
        let crop = cap.crop((10, -20), &Rect::new(30, -10, 20, 10)).unwrap();
        assert_eq!(crop.len(), 20 * 10 * 4);
        // Primer píxel del recorte: x = 30 - 10 = 20, y = -10 + 20 = 10.
        assert_eq!(&crop[0..3], &[20, 10, 0]);
        // Último: x = 39, y = 19.
        assert_eq!(&crop[crop.len() - 4..crop.len() - 1], &[39, 19, 0]);
        assert!(cap.crop((10, -20), &Rect::new(5, 0, 20, 10)).is_none(), "empieza antes de la captura");
        assert!(cap.crop((10, -20), &Rect::new(100, 0, 20, 10)).is_none(), "se sale por la derecha");
    }

    #[test]
    fn el_cambio_de_fondo_se_detecta_pero_el_mismo_fondo_no() {
        let r = Rect::new(0, 0, 320, 180);
        let old = image(320, 180, |x, y| [(x / 2) as u8, (y / 2) as u8, 40]);
        let cap_same = Capture { w: 320, h: 180, pixels: old.clone() };
        assert!(cap_same.diff_in((0, 0), &r, &old) < 1.0);
        // Un parpadeo pequeño (un fondo animado) no cuenta como cambio.
        let flicker = Capture { w: 320, h: 180, pixels: image(320, 180, |x, y| [(x / 2) as u8 + 3, (y / 2) as u8, 40]) };
        assert!(flicker.diff_in((0, 0), &r, &old) < CHANGED);
        // Otro fondo distinto sí.
        let other = Capture { w: 320, h: 180, pixels: image(320, 180, |x, y| [200 - (x / 3) as u8, 255 - (y / 2) as u8, 180]) };
        assert!(other.diff_in((0, 0), &r, &old) > CHANGED);
    }
}
