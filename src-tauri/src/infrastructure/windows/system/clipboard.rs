//! Portapapeles de texto (Win32). Respeta a los gestores de contraseñas: si quien copió pidió que el contenido
//! no entre en historiales (`ExcludeClipboardContentFromMonitorProcessing` o `CanIncludeInClipboardHistory = 0`),
//! `read_text` devuelve `None`.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

use crate::application::system::ClipboardControl;
use crate::domain::utility::MAX_CLIPBOARD_CHARS;

pub struct Win32Clipboard;

/// Abre el portapapeles con reintentos (otra aplicación puede tenerlo abierto un instante).
struct Open;

impl Open {
    fn acquire() -> Result<Open, String> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Open);
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        Err("el portapapeles está ocupado".into())
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn format_available(name: PCWSTR) -> bool {
    unsafe {
        let id = RegisterClipboardFormatW(name);
        id != 0 && IsClipboardFormatAvailable(id).is_ok()
    }
}

/// `CanIncludeInClipboardHistory` es un DWORD: 0 = no incluir.
fn history_forbidden() -> bool {
    unsafe {
        let id = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        if id == 0 || IsClipboardFormatAvailable(id).is_err() {
            return false;
        }
        match GetClipboardData(id) {
            Ok(h) if !h.is_invalid() => {
                let g = HGLOBAL(h.0);
                let p = GlobalLock(g) as *const u32;
                let forbidden = !p.is_null() && *p == 0;
                let _ = GlobalUnlock(g);
                forbidden
            }
            _ => false,
        }
    }
}

impl ClipboardControl for Win32Clipboard {
    fn read_text(&mut self) -> Result<Option<String>, String> {
        let _open = Open::acquire()?;
        unsafe {
            if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_err() {
                return Ok(None);
            }
            if format_available(w!("ExcludeClipboardContentFromMonitorProcessing")) || history_forbidden() {
                return Ok(None);
            }
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).map_err(|e| e.to_string())?;
            if handle.is_invalid() {
                return Ok(None);
            }
            let g = HGLOBAL(handle.0);
            let ptr = GlobalLock(g) as *const u16;
            if ptr.is_null() {
                return Ok(None);
            }
            let mut len = 0usize;
            while *ptr.add(len) != 0 && len < MAX_CLIPBOARD_CHARS * 2 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(g);
            Ok(Some(text))
        }
    }

    fn write_text(&mut self, text: &str) -> Result<(), String> {
        if text.chars().count() > MAX_CLIPBOARD_CHARS {
            return Err(format!("el texto supera {MAX_CLIPBOARD_CHARS} caracteres"));
        }
        let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = utf16.len() * 2;
        let _open = Open::acquire()?;
        unsafe {
            EmptyClipboard().map_err(|e| e.to_string())?;
            let g = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| e.to_string())?;
            let dst = GlobalLock(g) as *mut u16;
            if dst.is_null() {
                let _ = GlobalFree(Some(g));
                return Err("no se pudo reservar memoria".into());
            }
            std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
            let _ = GlobalUnlock(g);
            // Si SetClipboardData funciona, el sistema es dueño de la memoria.
            if SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(g.0))).is_err() {
                let _ = GlobalFree(Some(g));
                return Err("Windows rechazó el texto".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo lectura: no se toca el portapapeles real en las pruebas automáticas.
    #[test]
    fn leer_el_portapapeles_real_no_falla() {
        let r = Win32Clipboard.read_text();
        println!("portapapeles: {} caracteres", r.as_ref().ok().and_then(|t| t.as_ref()).map(|t| t.chars().count()).unwrap_or(0));
        assert!(r.is_ok(), "{r:?}");
    }

    /// Escribe, lee y RESTAURA el texto que hubiera. Es una prueba manual (`-- --ignored`) porque toca el portapapeles.
    #[test]
    #[ignore]
    fn escribir_y_leer_con_restauracion() {
        let mut c = Win32Clipboard;
        let before = c.read_text().unwrap();
        c.write_text("prueba wm ñandú 🙂").unwrap();
        assert_eq!(c.read_text().unwrap().as_deref(), Some("prueba wm ñandú 🙂"));
        if let Some(t) = before {
            c.write_text(&t).unwrap();
        }
    }
}
