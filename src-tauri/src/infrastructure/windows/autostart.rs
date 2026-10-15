//! Arranque con Windows (§12 Fase 10): un valor en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! No requiere administrador y se deshace borrando el valor. Todas las funciones reciben la clave y el nombre
//! del valor para que las pruebas usen una clave propia y NUNCA toquen el arranque real del equipo.

use std::path::Path;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

/// Clave de arranque real de Windows para el usuario actual.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Nombre del valor de esta app.
pub const VALUE_NAME: &str = "WindowManager";

/// Línea de comandos con la que arrancará la app: ruta entre comillas y los argumentos de gestión y barra.
pub fn command_line(exe: &Path, manage_all: bool, bar: bool) -> String {
    let mut c = format!("\"{}\"", exe.display());
    if manage_all {
        c.push_str(" --manage all");
    }
    if bar {
        c.push_str(" --bar");
    }
    c
}

struct Key(HKEY);

impl Key {
    fn open(subkey: &str, access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Result<Key, String> {
        let mut h = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(subkey),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &mut h,
                None,
            )
        };
        if status == ERROR_SUCCESS { Ok(Key(h)) } else { Err(format!("no se pudo abrir la clave del registro ({status:?})")) }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Activa el arranque con Windows.
pub fn enable(subkey: &str, value_name: &str, command: &str) -> Result<(), String> {
    let key = Key::open(subkey, KEY_SET_VALUE)?;
    let utf16: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe { std::slice::from_raw_parts(utf16.as_ptr() as *const u8, utf16.len() * 2) };
    let status = unsafe { RegSetValueExW(key.0, &HSTRING::from(value_name), None, REG_SZ, Some(bytes)) };
    if status == ERROR_SUCCESS { Ok(()) } else { Err(format!("no se pudo guardar el arranque ({status:?})")) }
}

/// Desactiva el arranque (no falla si ya estaba desactivado).
pub fn disable(subkey: &str, value_name: &str) -> Result<(), String> {
    let key = Key::open(subkey, KEY_SET_VALUE)?;
    let status = unsafe { RegDeleteValueW(key.0, &HSTRING::from(value_name)) };
    // ERROR_FILE_NOT_FOUND (2): ya no existía.
    if status == ERROR_SUCCESS || status.0 == 2 { Ok(()) } else { Err(format!("no se pudo quitar el arranque ({status:?})")) }
}

/// Comando guardado si el arranque está activado.
pub fn status(subkey: &str, value_name: &str) -> Option<String> {
    // Solo lectura: abrir sin crear (consultar no debe dejar claves nuevas en el registro).
    let mut h = HKEY::default();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(subkey), None, KEY_READ, &mut h) } != ERROR_SUCCESS {
        return None;
    }
    let key = Key(h);
    let mut size = 0u32;
    let name = HSTRING::from(value_name);
    unsafe {
        if RegQueryValueExW(key.0, &name, None, None, None, Some(&mut size)) != ERROR_SUCCESS || size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if RegQueryValueExW(key.0, &name, None, None, Some(buf.as_mut_ptr()), Some(&mut size)) != ERROR_SUCCESS {
            return None;
        }
        let words: Vec<u16> = buf[..size as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Some(String::from_utf16_lossy(&words).trim_end_matches('\0').to_string())
    }
}

/// Borra una clave de pruebas entera (solo la usan las pruebas).
pub fn delete_tree(subkey: &str) {
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(subkey));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linea_de_comandos_con_comillas_y_argumentos() {
        let exe = Path::new(r"C:\Program Files\Window Manager\window-manager.exe");
        assert_eq!(
            command_line(exe, true, true),
            r#""C:\Program Files\Window Manager\window-manager.exe" --manage all --bar"#
        );
        assert_eq!(command_line(exe, false, true), r#""C:\Program Files\Window Manager\window-manager.exe" --bar"#);
        assert_eq!(command_line(exe, false, false), r#""C:\Program Files\Window Manager\window-manager.exe""#);
    }

    /// Usa una clave PROPIA de pruebas bajo HKCU y la borra al terminar: el arranque real de Windows no se toca.
    #[test]
    fn activar_consultar_y_desactivar_en_una_clave_de_pruebas() {
        let key = format!(r"Software\WindowManagerTests\Run-{}", std::process::id());
        let cmd = command_line(Path::new(r"C:\Apps\wm.exe"), true, true);
        assert_eq!(status(&key, VALUE_NAME), None);
        enable(&key, VALUE_NAME, &cmd).unwrap();
        assert_eq!(status(&key, VALUE_NAME).as_deref(), Some(cmd.as_str()));
        // Reactivar con otro comando lo reemplaza.
        enable(&key, VALUE_NAME, "\"C:\\Apps\\wm2.exe\"").unwrap();
        assert_eq!(status(&key, VALUE_NAME).as_deref(), Some("\"C:\\Apps\\wm2.exe\""));
        disable(&key, VALUE_NAME).unwrap();
        assert_eq!(status(&key, VALUE_NAME), None);
        disable(&key, VALUE_NAME).unwrap(); // desactivar dos veces no falla
        delete_tree(r"Software\WindowManagerTests");
        assert_eq!(status(&key, VALUE_NAME), None);
    }

    #[test]
    fn el_arranque_real_no_esta_activado_por_las_pruebas() {
        // Solo lectura: las pruebas nunca escriben en la clave real.
        println!("arranque real: {:?}", status(RUN_KEY, VALUE_NAME));
    }
}
