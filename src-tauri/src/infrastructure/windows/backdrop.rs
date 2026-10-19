//! Fondo «de cristal» de la barra y del panel: una copia de lo que hay detrás, ya desenfocada, que la vista dibuja
//! debajo de su tinte. Windows solo ofrece un desenfoque nativo de intensidad fija, y en el panel además no seguiría sus
//! esquinas redondeadas ni la animación de apertura; con la copia, la intensidad y la transparencia son las que se elijan.
//!
//! - Barra: lo único que hay detrás es el fondo de pantalla, que se copia de la ventana que lo aloja (`PrintWindow`).
//! - Panel: se copia la zona de la pantalla justo antes de mostrarlo (todavía no tapa nada).
//!
//! Las copias se guardan reducidas a la mitad; el desenfoque se aplica al pedirlas (son pocos milisegundos).

use std::collections::HashMap;
use std::sync::Mutex;

use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HGDIOBJ, ROP_CODE, SRCCOPY,
};

use crate::domain::geometry::Rect;
use crate::infrastructure::windows::fade::{capture_host, find_host, is_blank};
use crate::infrastructure::windows::system::media::base64;

/// Zona de pantalla copiada a la mitad de su tamaño (BGRA, de arriba abajo).
#[derive(Clone)]
pub struct Raw {
    /// Zona original, en píxeles de pantalla.
    pub src: Rect,
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

/// Imagen lista para la vista: `width` y `height` son los píxeles de pantalla que cubre (la vista los divide por su escala).
#[derive(Debug, Clone, serde::Serialize)]
pub struct GlassImage {
    pub url: String,
    pub width: i32,
    pub height: i32,
}

// ---------------------------------------------------------------- lógica pura

/// Reduce a la mitad promediando bloques de 2×2 (los bordes impares repiten el último píxel).
pub fn downscale2(px: &[u8], w: i32, h: i32) -> (Vec<u8>, i32, i32) {
    let (nw, nh) = ((w + 1) / 2, (h + 1) / 2);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let (x0, x1) = (x * 2, (x * 2 + 1).min(w - 1));
            let (y0, y1) = (y * 2, (y * 2 + 1).min(h - 1));
            for c in 0..4 {
                let at = |xx: i32, yy: i32| px[((yy * w + xx) * 4 + c) as usize] as u32;
                out[((y * nw + x) * 4 + c) as usize] = ((at(x0, y0) + at(x1, y0) + at(x0, y1) + at(x1, y1) + 2) / 4) as u8;
            }
        }
    }
    (out, nw, nh)
}

/// Una pasada de desenfoque de caja en horizontal o vertical; los bordes repiten el último píxel (sin oscurecerse).
fn box_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let n = (2 * r + 1) as u32;
    let (lines, len) = if horizontal { (h, w) } else { (w, h) };
    let index = |line: usize, i: isize, c: usize| -> usize {
        let i = i.clamp(0, len as isize - 1) as usize;
        if horizontal { (line * w + i) * 4 + c } else { (i * w + line) * 4 + c }
    };
    for line in 0..lines {
        for c in 0..3 {
            let mut sum: u32 = (-(r as isize)..=(r as isize)).map(|k| src[index(line, k, c)] as u32).sum();
            for i in 0..len as isize {
                dst[index(line, i, c)] = ((sum + n / 2) / n) as u8;
                sum += src[index(line, i + r as isize + 1, c)] as u32;
                sum -= src[index(line, i - r as isize, c)] as u32;
            }
        }
    }
}

/// Desenfoque aproximadamente gaussiano (tres pasadas de caja en cada sentido) de radio `r` píxeles.
pub fn blur(px: &mut Vec<u8>, w: i32, h: i32, r: usize) {
    if r == 0 || w <= 0 || h <= 0 {
        return;
    }
    let (wu, hu) = (w as usize, h as usize);
    let mut tmp = vec![0u8; px.len()];
    for _ in 0..3 {
        box_pass(px, &mut tmp, wu, hu, r, true);
        box_pass(&tmp, px, wu, hu, r, false);
    }
}

/// Sigma del desenfoque en píxeles lógicos para una intensidad de 0 a 100.
pub fn sigma_for(intensity: f32) -> f32 {
    intensity.clamp(0.0, 100.0) * 0.3
}

/// BMP de 32 bits (de arriba abajo, opaco) listo para un `data:` URL.
pub fn bmp(w: i32, h: i32, bgra: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(54 + bgra.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + bgra.len()) as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&(-h).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(bgra.len() as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for px in bgra.chunks_exact(4) {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}

/// Desenfoca la copia con la intensidad pedida (0 a 100) para una pantalla de escala `scale` y la deja como imagen.
pub fn render(raw: &Raw, intensity: f32, scale: f64) -> GlassImage {
    let (mut px, mut w, mut h) = (raw.px.clone(), raw.w, raw.h);
    // La copia guardada ya está a la mitad: el radio también.
    let mut radius = (sigma_for(intensity) as f64 * scale / 2.0).round() as usize;
    // Con desenfoque medio o fuerte se reduce más: el resultado es el mismo (ya es una mancha), pesa menos al pasarlo a la
    // vista y cuesta mucho menos calcularlo.
    while radius > 5 && w > 8 && h > 8 {
        let (p, nw, nh) = downscale2(&px, w, h);
        px = p;
        w = nw;
        h = nh;
        radius /= 2;
    }
    blur(&mut px, w, h, radius);
    GlassImage { url: format!("data:image/bmp;base64,{}", base64(&bmp(w, h, &px))), width: raw.src.w, height: raw.src.h }
}

// ---------------------------------------------------------------- capturas

/// Copia de una zona de la pantalla tal como se ve ahora (incluidas las ventanas con transparencia), reducida a la mitad.
pub fn capture_screen(r: Rect) -> Option<Raw> {
    if r.w <= 0 || r.h <= 0 || r.w > 8192 || r.h > 8192 {
        return None;
    }
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let mem = CreateCompatibleDC(Some(screen));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: r.w, biHeight: -r.h, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB.0, ..Default::default() },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let result = match CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => {
                let old = SelectObject(mem, HGDIOBJ(bitmap.0));
                let ok = BitBlt(mem, 0, 0, r.w, r.h, Some(screen), r.x, r.y, ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0)).is_ok();
                let mut px = vec![0u8; (r.w * r.h * 4) as usize];
                if ok && !bits.is_null() {
                    std::ptr::copy_nonoverlapping(bits as *const u8, px.as_mut_ptr(), px.len());
                }
                SelectObject(mem, old);
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                ok.then_some(px)
            }
            Err(_) => None,
        };
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        let px = result?;
        let (px, w, h) = downscale2(&px, r.w, r.h);
        Some(Raw { src: r, w, h, px })
    }
}

/// Copia del fondo de pantalla en la zona `r` (la barra), sin ventanas ni iconos encima.
fn capture_wallpaper(r: Rect) -> Option<Raw> {
    let host = find_host()?;
    let (origin, cap) = capture_host(host)?;
    if is_blank(&cap.pixels, cap.w, cap.h) {
        return None;
    }
    let px = cap.crop(origin, &r)?;
    let (px, w, h) = downscale2(&px, r.w, r.h);
    Some(Raw { src: r, w, h, px })
}

static BARS: Mutex<Option<HashMap<String, Raw>>> = Mutex::new(None);
static PANEL: Mutex<Option<Raw>> = Mutex::new(None);
static SEARCH: Mutex<Option<Raw>> = Mutex::new(None);

/// Imagen para la barra `label`, que ocupa `r`. Con `refresh` (o sin copia previa, o si cambió de sitio) se vuelve a copiar el fondo.
pub fn bar_image(label: &str, r: Rect, intensity: f32, scale: f64, refresh: bool) -> Option<GlassImage> {
    let cached = BARS.lock().unwrap().as_ref().and_then(|m| m.get(label)).filter(|raw| raw.src == r).cloned();
    let raw = match cached {
        Some(raw) if !refresh => raw,
        _ => {
            let raw = capture_wallpaper(r)?;
            BARS.lock().unwrap().get_or_insert_with(HashMap::new).insert(label.to_string(), raw.clone());
            raw
        }
    };
    Some(render(&raw, intensity, scale))
}

/// Guarda lo que hay detrás del panel (llamar justo antes de mostrarlo).
pub fn remember_panel(r: Rect) {
    *PANEL.lock().unwrap() = capture_screen(r);
}

pub fn forget_panel() {
    *PANEL.lock().unwrap() = None;
}

/// Imagen guardada en `slot`. Con `refresh` (cambió el fondo de pantalla con la ventana abierta) se vuelve a copiar el fondo
/// de pantalla de su zona: con la ventana encima no se puede copiar la pantalla entera, así que se ven solo el fondo, sin ventanas.
fn slot_image(slot: &Mutex<Option<Raw>>, intensity: f32, scale: f64, refresh: bool) -> Option<GlassImage> {
    if refresh {
        let area = slot.lock().unwrap().as_ref().map(|raw| raw.src);
        if let Some(fresh) = area.and_then(capture_wallpaper) {
            *slot.lock().unwrap() = Some(fresh);
        }
    }
    let raw = slot.lock().unwrap().clone()?;
    Some(render(&raw, intensity, scale))
}

/// Imagen para el panel.
pub fn panel_image(intensity: f32, scale: f64, refresh: bool) -> Option<GlassImage> {
    slot_image(&PANEL, intensity, scale, refresh)
}

/// Guarda lo que hay detrás del buscador (llamar justo antes de mostrarlo). Tiene su propia copia: el buscador y el panel
/// pueden estar en monitores y zonas distintas.
pub fn remember_search(r: Rect) {
    *SEARCH.lock().unwrap() = capture_screen(r);
}

/// Imagen para el buscador.
pub fn search_image(intensity: f32, scale: f64, refresh: bool) -> Option<GlassImage> {
    slot_image(&SEARCH, intensity, scale, refresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(w: i32, h: i32, bgr: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [bgr[0], bgr[1], bgr[2], 255]).collect()
    }

    #[test]
    fn reducir_a_la_mitad_promedia_y_redondea_hacia_arriba_las_medidas_impares() {
        let (px, w, h) = downscale2(&flat(5, 3, [10, 20, 30]), 5, 3);
        assert_eq!((w, h), (3, 2));
        assert_eq!(&px[..4], &[10, 20, 30, 255]);
        // Cuatro píxeles distintos dan su media.
        let four = [0, 0, 0, 255, 100, 100, 100, 255, 100, 100, 100, 255, 200, 200, 200, 255];
        assert_eq!(&downscale2(&four, 2, 2).0[..3], &[100, 100, 100]);
    }

    #[test]
    fn desenfocar_una_imagen_lisa_no_la_cambia_ni_en_los_bordes() {
        let mut px = flat(20, 10, [40, 90, 200]);
        blur(&mut px, 20, 10, 4);
        assert!(px.chunks_exact(4).all(|p| p[..3] == [40, 90, 200]), "los bordes no deben oscurecerse");
    }

    #[test]
    fn desenfocar_reparte_un_punto_brillante_y_conserva_el_brillo_total() {
        let (w, h) = (31, 31);
        let mut px = flat(w, h, [0, 0, 0]);
        let center = ((15 * w + 15) * 4) as usize;
        px[center..center + 3].copy_from_slice(&[255, 255, 255]);
        blur(&mut px, w, h, 3);
        assert!(px[center] < 40, "el punto se difumina");
        assert!(px[center] > 0);
        let near = ((15 * w + 17) * 4) as usize;
        assert!(px[near] > 0, "la luz llega a los vecinos");
        let total: u32 = px.chunks_exact(4).map(|p| p[0] as u32).sum();
        // Con enteros de 8 bits los valores pequeños se redondean, así que el total solo se conserva a grandes rasgos.
        assert!((120..=450).contains(&total), "el brillo total se conserva (aprox.): {total}");
    }

    #[test]
    fn sin_intensidad_no_hay_desenfoque_y_la_intensidad_tiene_tope() {
        assert_eq!(sigma_for(0.0), 0.0);
        assert_eq!(sigma_for(-5.0), 0.0);
        assert_eq!(sigma_for(500.0), sigma_for(100.0));
        assert!(sigma_for(50.0) > sigma_for(10.0));
    }

    #[test]
    fn el_bmp_tiene_cabecera_valida_y_pixeles_opacos() {
        let data = bmp(2, 1, &[1, 2, 3, 0, 4, 5, 6, 0]);
        assert_eq!(&data[..2], b"BM");
        assert_eq!(u32::from_le_bytes(data[2..6].try_into().unwrap()) as usize, data.len());
        assert_eq!(i32::from_le_bytes(data[18..22].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(data[22..26].try_into().unwrap()), -1, "de arriba abajo");
        assert_eq!(&data[54..], &[1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn la_imagen_final_es_un_data_url_con_el_tamano_de_pantalla_original() {
        let raw = Raw { src: Rect::new(10, 20, 8, 6), w: 4, h: 3, px: flat(4, 3, [9, 9, 9]) };
        let img = render(&raw, 50.0, 1.0);
        assert!(img.url.starts_with("data:image/bmp;base64,"));
        assert_eq!((img.width, img.height), (8, 6));
    }

    #[test]
    fn una_copia_real_de_la_pantalla_tiene_el_tamano_pedido() {
        // Lee la pantalla (sin guardarla): solo comprueba las medidas.
        if let Some(raw) = capture_screen(Rect::new(0, 0, 64, 40)) {
            assert_eq!((raw.w, raw.h), (32, 20));
            assert_eq!(raw.px.len(), 32 * 20 * 4);
        }
    }
}
