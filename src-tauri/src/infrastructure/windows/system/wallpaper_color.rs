//! Color dominante del fondo de pantalla, para que la interfaz pueda adaptarse a él. Se decodifica la imagen con WIC
//! (el decodificador de Windows) y se reduce a una miniatura de 24×24; nada sale del equipo.

use windows::core::{GUID, HSTRING};
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

const SIDE: u32 = 24;

fn hue_and_strength(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let sat = if max > 0.0 { d / max } else { 0.0 };
    let hue = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    ((hue + 360.0) % 360.0, sat, max / 255.0)
}

/// Color que mejor representa una miniatura BGRA: se busca el TONO más presente (peso = viveza × claridad, así un fondo
/// oscuro y apagado sigue dando su matiz y no un gris) y se promedian los píxeles de ese tono. Si la imagen no tiene
/// color (blanco y negro) se devuelve el promedio simple.
pub fn dominant(bgra: &[u8]) -> Option<[u8; 3]> {
    const BINS: usize = 36;
    let (mut ar, mut ag, mut ab, mut n) = (0f64, 0f64, 0f64, 0f64);
    let mut histogram = [0f64; BINS];
    let px: Vec<(f64, f64, f64)> = bgra.chunks_exact(4).map(|p| (p[2] as f64, p[1] as f64, p[0] as f64)).collect();
    for &(r, g, b) in &px {
        let (hue, sat, val) = hue_and_strength(r, g, b);
        ar += r;
        ag += g;
        ab += b;
        n += 1.0;
        histogram[((hue / 10.0) as usize).min(BINS - 1)] += sat * val.max(0.15);
    }
    if n == 0.0 {
        return None;
    }
    let total: f64 = histogram.iter().sum();
    if total < 0.02 * n {
        return Some([(ar / n) as u8, (ag / n) as u8, (ab / n) as u8]);
    }
    // El tono ganador, sumando sus dos vecinos para que un degradado no se reparta entre casillas.
    let best = (0..BINS)
        .max_by(|&x, &y| {
            let score = |i: usize| histogram[(i + BINS - 1) % BINS] + histogram[i] + histogram[(i + 1) % BINS];
            score(x).partial_cmp(&score(y)).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);
    let (mut r, mut g, mut b, mut w) = (0f64, 0f64, 0f64, 0f64);
    for &(pr, pg, pb) in &px {
        let (hue, sat, val) = hue_and_strength(pr, pg, pb);
        let bin = ((hue / 10.0) as usize).min(BINS - 1);
        let near = (bin + BINS - best) % BINS;
        if near <= 1 || near == BINS - 1 {
            let weight = sat * val.max(0.15);
            r += pr * weight;
            g += pg * weight;
            b += pb * weight;
            w += weight;
        }
    }
    if w <= 0.0 {
        return Some([(ar / n) as u8, (ag / n) as u8, (ab / n) as u8]);
    }
    Some([(r / w) as u8, (g / w) as u8, (b / w) as u8])
}

pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn thumbnail(path: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let e = |e: windows::core::Error| e.to_string();
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(e)?;
        let decoder = factory
            .CreateDecoderFromFilename(&HSTRING::from(path), None::<*const GUID>, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(e)?;
        let frame = decoder.GetFrame(0).map_err(e)?;
        let convert = factory.CreateFormatConverter().map_err(e)?;
        convert
            .Initialize(&frame, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)
            .map_err(e)?;
        let scaler = factory.CreateBitmapScaler().map_err(e)?;
        scaler.Initialize(&convert, SIDE, SIDE, WICBitmapInterpolationModeFant).map_err(e)?;
        let mut buf = vec![0u8; (SIDE * SIDE * 4) as usize];
        scaler.CopyPixels(std::ptr::null(), SIDE * 4, &mut buf).map_err(e)?;
        Ok(buf)
    }
}

/// Último resultado: la interfaz lo pide desde varias ventanas cada pocos segundos y la imagen casi nunca cambia.
static LAST: std::sync::Mutex<Option<(String, Option<std::time::SystemTime>, String)>> = std::sync::Mutex::new(None);

/// Color dominante (`#rrggbb`) de la imagen indicada. Si la imagen es la misma (misma ruta y misma fecha de
/// modificación) que en la última llamada, se devuelve el resultado guardado sin decodificarla de nuevo.
pub fn color_of(path: &str) -> Result<String, String> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Some((p, m, color)) = LAST.lock().unwrap().as_ref() {
        if p == path && *m == modified && modified.is_some() {
            return Ok(color.clone());
        }
    }
    let owned = path.to_string();
    // Hilo propio: así no se mezcla con el modo COM que traiga el hilo que llama.
    let pixels = std::thread::Builder::new()
        .name("wm-wallpaper-color".into())
        .spawn(move || thumbnail(&owned))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "el hilo del color falló".to_string())??;
    let color = dominant(&pixels).map(hex).ok_or_else(|| "imagen vacía".to_string())?;
    *LAST.lock().unwrap() = Some((path.to_string(), modified, color.clone()));
    Ok(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(b: u8, g: u8, r: u8, count: usize) -> Vec<u8> {
        (0..count).flat_map(|_| [b, g, r, 255]).collect()
    }

    #[test]
    fn un_color_unico_se_devuelve_tal_cual() {
        assert_eq!(dominant(&solid(200, 100, 50, 16)), Some([50, 100, 200]));
    }

    #[test]
    fn el_color_vivo_pesa_mas_que_el_gris() {
        let mut px = solid(128, 128, 128, 90);
        px.extend(solid(255, 20, 20, 10)); // azul intenso
        let [r, _, b] = dominant(&px).unwrap();
        assert!(b > r + 60, "debería predominar el azul: r={r} b={b}");
    }

    #[test]
    fn un_fondo_oscuro_y_apagado_conserva_su_matiz() {
        // Casi negro con un toque de azul: no debe salir un gris sin matiz.
        let [r, _, b] = dominant(&solid(60, 40, 38, 50)).unwrap();
        assert!(b > r + 15, "debería seguir siendo azulado: r={r} b={b}");
        // Dos tonos: gana el que más hay, aunque sea más tenue que el otro.
        let mut px = solid(70, 45, 40, 80); // azul apagado, mucho
        px.extend(solid(30, 30, 200, 10)); // rojo vivo, poco
        let [r, _, b] = dominant(&px).unwrap();
        assert!(b > r, "r={r} b={b}");
    }

    #[test]
    fn en_gris_se_usa_el_promedio() {
        assert_eq!(dominant(&solid(100, 100, 100, 8)), Some([100, 100, 100]));
        assert_eq!(dominant(&[]), None);
    }

    #[test]
    fn el_hex_tiene_seis_cifras() {
        assert_eq!(hex([255, 0, 10]), "#ff000a");
    }
}
