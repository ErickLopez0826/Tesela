//! Tipos de los módulos de utilidad (§9.2, §9.3, Fase 8): apps abiertas y fijadas, portapapeles, prueba de velocidad.

use serde::Serialize;

/// Una ventana de aplicación abierta, para la lista de "apps abiertas" (§9.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenApp {
    pub handle: isize,
    /// Nombre del ejecutable (p. ej. `Code.exe`).
    pub exe: String,
    /// Ruta completa del ejecutable; sirve para fijarla y abrirla después.
    pub path: String,
    pub title: String,
}

/// Máximos para que un módulo no pueda saturar el portapapeles o abusar de lo que lanza.
pub const MAX_CLIPBOARD_CHARS: usize = 100_000;
pub const MAX_LAUNCH_TARGET_CHARS: usize = 520;

/// ¿Es algo que un módulo puede pedir abrir? Solo programas y accesos directos, con ruta absoluta de Windows.
/// No se aceptan parámetros ni scripts: `apps.launch` abre una aplicación, no ejecuta comandos.
pub fn is_launchable(target: &str) -> Result<(), String> {
    let t = target.trim();
    if t.is_empty() || t.len() > MAX_LAUNCH_TARGET_CHARS {
        return Err("ruta vacía o demasiado larga".into());
    }
    if t.chars().any(|c| c.is_control() || matches!(c, '"' | '<' | '>' | '|' | '*' | '?')) {
        return Err("la ruta tiene caracteres no permitidos".into());
    }
    let bytes = t.as_bytes();
    let absolute = bytes.len() > 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/');
    if !absolute {
        return Err("debe ser una ruta absoluta (p. ej. C:\\Programas\\app.exe)".into());
    }
    if t.split(['\\', '/']).any(|seg| seg == "..") {
        return Err("la ruta no puede contener '..'".into());
    }
    let lower = t.to_lowercase();
    if !(lower.ends_with(".exe") || lower.ends_with(".lnk")) {
        return Err("solo se pueden abrir programas (.exe) y accesos directos (.lnk)".into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedResult {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub ping_ms: f64,
    pub server: String,
    pub isp: String,
    /// `true`: son datos de ejemplo, no una medición (el CLI de Ookla no está instalado).
    pub simulated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SpeedStatus {
    Idle,
    Running,
    Done { result: SpeedResult },
    Failed { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_que_se_pueden_abrir() {
        for ok in ["C:\\Program Files\\App\\app.exe", "D:/juegos/Steam/steam.exe", "C:\\Users\\erick\\Desktop\\Discord.LNK"] {
            assert!(is_launchable(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rutas_que_no() {
        for bad in [
            "",
            "   ",
            "app.exe",
            "\\\\servidor\\share\\app.exe",
            "C:\\a\\..\\b\\app.exe",
            "C:\\a\\script.bat",
            "C:\\a\\doc.txt",
            "C:\\a\\app.exe --flag",
            "C:\\a\\app.exe\" /c calc",
            "C:\\a\\app.exe|calc",
            "cmd.exe",
            "https://ejemplo.com/x.exe",
            "C:\\a\\x\n.exe",
        ] {
            assert!(is_launchable(bad).is_err(), "'{bad}' debería rechazarse");
        }
        assert!(is_launchable(&format!("C:\\{}.exe", "a".repeat(600))).is_err());
    }

    #[test]
    fn estado_de_la_prueba_de_velocidad_serializa_con_etiqueta() {
        let s = serde_json::to_value(SpeedStatus::Failed { error: "x".into() }).unwrap();
        assert_eq!(s["state"], "failed");
        assert_eq!(serde_json::to_value(SpeedStatus::Idle).unwrap()["state"], "idle");
    }
}
