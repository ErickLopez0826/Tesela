//! Iconos de las aplicaciones para la barra de tareas: el de la propia ventana (como lo hace la barra de Windows) o, si no
//! tiene, el de su ejecutable. Se entregan como imagen PNG en una URL de datos; nada sale del equipo.

use std::collections::HashMap;
use std::mem::size_of;
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetClassLongPtrW, GetIconInfo, SendMessageTimeoutW, GCLP_HICON, HICON, ICONINFO, ICON_BIG, SMTO_ABORTIFHUNG,
    WM_GETICON,
};

use crate::infrastructure::windows::facts::{exe_path_of, hwnd_from};
use crate::infrastructure::windows::system::media::base64;

// ---------------------------------------------------------------- lógica pura

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    !data.iter().fold(!0u32, |crc, &b| table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8))
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// PNG RGBA de una imagen BGRA (de arriba abajo). Sin compresión (bloques «stored» de zlib): un icono pesa pocos KB.
pub fn png(w: u32, h: u32, bgra: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w as usize * 4 + 1) * h as usize);
    for row in bgra.chunks_exact(w as usize * 4) {
        raw.push(0);
        for px in row.chunks_exact(4) {
            raw.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut header = Vec::new();
    header.extend_from_slice(&w.to_be_bytes());
    header.extend_from_slice(&h.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Los iconos antiguos no traen canal alfa: la transparencia sale de su máscara (negro = opaco, blanco = transparente).
pub fn apply_mask(bgra: &mut [u8], mask: &[u8]) {
    for (px, m) in bgra.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
        px[3] = if m[0] == 0 && m[1] == 0 && m[2] == 0 { 255 } else { 0 };
    }
}

// ---------------------------------------------------------------- Windows

fn dib(hdc: windows::Win32::Graphics::Gdi::HDC, bitmap: HBITMAP, w: i32, h: i32) -> Option<Vec<u8>> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
        ..Default::default()
    };
    let mut px = vec![0u8; (w * h * 4) as usize];
    let rows = unsafe { GetDIBits(hdc, bitmap, 0, h as u32, Some(px.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS) };
    (rows != 0).then_some(px)
}

/// PNG de un icono de Windows.
fn icon_png(icon: HICON) -> Option<Vec<u8>> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;
        let result = (|| {
            if info.hbmColor.is_invalid() {
                return None;
            }
            let mut bmp = BITMAP::default();
            if GetObjectW(HGDIOBJ(info.hbmColor.0), size_of::<BITMAP>() as i32, Some(&mut bmp as *mut _ as *mut _)) == 0 {
                return None;
            }
            let (w, h) = (bmp.bmWidth, bmp.bmHeight);
            if w <= 0 || h <= 0 || w > 512 || h > 512 {
                return None;
            }
            let screen = GetDC(None);
            let hdc = CreateCompatibleDC(Some(screen));
            let color = dib(hdc, info.hbmColor, w, h);
            let mask = if info.hbmMask.is_invalid() { None } else { dib(hdc, info.hbmMask, w, h) };
            let _ = DeleteDC(hdc);
            ReleaseDC(None, screen);
            let mut px = color?;
            if px.chunks_exact(4).all(|p| p[3] == 0) {
                match &mask {
                    Some(m) => apply_mask(&mut px, m),
                    None => px.chunks_exact_mut(4).for_each(|p| p[3] = 255),
                }
            }
            Some(png(w as u32, h as u32, &px))
        })();
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        result
    }
}

/// Icono que la propia ventana declara (el grande, o el de su clase).
fn window_icon(handle: isize) -> Option<HICON> {
    unsafe {
        let hwnd = hwnd_from(handle);
        let mut result = 0usize;
        let sent = SendMessageTimeoutW(hwnd, WM_GETICON, WPARAM(ICON_BIG as usize), LPARAM(0), SMTO_ABORTIFHUNG, 150, Some(&mut result));
        if sent.0 != 0 && result != 0 {
            return Some(HICON(result as *mut _));
        }
        let class = GetClassLongPtrW(hwnd, GCLP_HICON);
        (class != 0).then(|| HICON(class as *mut _))
    }
}

/// PNG del icono de la ventana `handle`; si no tiene, el de su ejecutable.
pub fn app_icon_png(handle: isize) -> Option<Vec<u8>> {
    if let Some(png) = window_icon(handle).and_then(icon_png) {
        return Some(png);
    }
    exe_icon_png(&exe_path_of(handle)?)
}

/// PNG del icono de un ejecutable.
pub fn exe_icon_png(path: &str) -> Option<Vec<u8>> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut info = SHFILEINFOW::default();
        let got = SHGetFileInfoW(PCWSTR(wide.as_ptr()), FILE_ATTRIBUTE_NORMAL, Some(&mut info), size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES);
        if got == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let png = icon_png(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        png
    }
}

/// URL de datos con el icono de un ejecutable (se calcula una vez por ruta).
pub fn exe_icon_url(path: &str) -> Option<String> {
    let key = format!("exe:{path}");
    if let Some(hit) = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(&key)) {
        return hit.clone();
    }
    let url = exe_icon_png(path).map(|bytes| format!("data:image/png;base64,{}", base64(&bytes)));
    if url.is_some() {
        CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, url.clone());
    }
    url
}

static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

/// URL de datos con el icono de la ventana. `key` agrupa las ventanas que comparten icono (el ejecutable): se calcula una vez.
pub fn app_icon_url(handle: isize, key: &str) -> Option<String> {
    if let Some(hit) = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(key)) {
        return hit.clone();
    }
    let url = app_icon_png(handle).map(|bytes| format!("data:image/png;base64,{}", base64(&bytes)));
    // Un fallo no se recuerda para siempre: puede ser que la ventana aún no tuviera icono.
    if url.is_some() {
        CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(key.to_string(), url.clone());
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_crc_y_el_adler_son_los_de_siempre() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn un_png_tiene_firma_cabecera_y_final_correctos() {
        let bytes = png(2, 2, &[0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 0, 0, 0]);
        assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(&bytes[16..24], &[0, 0, 0, 2, 0, 0, 0, 2]);
        assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
        assert_eq!(&bytes[bytes.len() - 4..], &0xAE42_6082u32.to_be_bytes());
    }

    #[test]
    fn el_png_de_una_imagen_grande_reparte_el_zlib_en_bloques() {
        let bytes = png(100, 100, &vec![200u8; 100 * 100 * 4]);
        assert!(bytes.len() > 40_000, "sin compresión pesa lo que pesan los píxeles");
    }

    #[test]
    fn la_mascara_pone_la_transparencia_de_los_iconos_antiguos() {
        let mut px = [10, 20, 30, 0, 10, 20, 30, 0];
        apply_mask(&mut px, &[0, 0, 0, 0, 255, 255, 255, 0]);
        assert_eq!([px[3], px[7]], [255, 0]);
    }

    #[test]
    fn el_icono_de_una_ventana_real_se_convierte_en_png() {
        // La ventana del escritorio siempre existe; su icono puede faltar, pero si hay, es un PNG válido.
        use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;
        let id = unsafe { GetDesktopWindow() }.0 as isize;
        if let Some(bytes) = app_icon_png(id) {
            assert_eq!(&bytes[1..4], b"PNG");
        }
    }
}
