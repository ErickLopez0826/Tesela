//! Adaptadores del «uso de Claude»: la credencial de Claude Code (solo lectura), la petición HTTPS (WinHTTP, del propio Windows: sin
//! dependencias nuevas, con el almacén de certificados y el proxy del sistema) y el interruptor que guarda la persona.
//!
//! Solo se habla con `api.anthropic.com`, la dirección está fija en el código y el token solo viaja en la cabecera `Authorization`.
//! No se escribe ni se muestra en ningún sitio.

use std::ffi::c_void;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::{
    WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_CUSTOM, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

use crate::application::claude_usage::{FetchError, UsagePort, UsageService};
use crate::domain::claude_usage::{parse_credential, Credential};
use crate::infrastructure::claude_store::{now_secs, Paths};

/// Servidor y ruta fijos: nada de lo que llega de fuera puede cambiar a dónde va el token.
const HOST: &str = "api.anthropic.com";
const PATH: &str = "/api/oauth/usage";
const TIMEOUT_MS: i32 = 15_000;
/// Una respuesta de uso son unos pocos KB: más de esto es otra cosa.
const MAX_BODY: usize = 1_048_576;

// ----- Interruptor -----

fn flag_file(paths: &Paths) -> std::path::PathBuf {
    paths.data_dir.join("usage.json")
}

/// ¿La persona activó la lectura? Sin archivo (o roto), no.
pub fn read_enabled(paths: &Paths) -> bool {
    std::fs::read_to_string(flag_file(paths))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| v.get("enabled")?.as_bool())
        .unwrap_or(false)
}

pub fn write_enabled(paths: &Paths, enabled: bool) -> Result<(), String> {
    let file = flag_file(paths);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::json!({ "enabled": enabled }).to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
}

// ----- Credencial -----

fn read_credential(claude_dir: &Path) -> Option<Credential> {
    parse_credential(&std::fs::read_to_string(claude_dir.join(".credentials.json")).ok()?)
}

// ----- HTTP -----

/// Un manejador de WinHTTP que se cierra solo.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void, what: &str) -> Result<Handle, FetchError> {
        if raw.is_null() {
            Err(FetchError::Network(format!("WinHTTP no pudo {what}")))
        } else {
            Ok(Handle(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}

/// `Retry-After` en segundos (la forma con fecha no se interpreta: se usa la espera normal).
pub fn parse_retry_after(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|n| *n <= 86_400)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// GET https://api.anthropic.com/api/oauth/usage con el token. Devuelve el cuerpo si el estado es 200.
fn http_get(token: &str) -> Result<String, FetchError> {
    let net = |what: &str, e: windows::core::Error| FetchError::Network(format!("{what}: {e}"));
    unsafe {
        let session = Handle::new(WinHttpOpen(w!("Tesela"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0), "abrir la sesión")?;
        WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS).map_err(|e| net("tiempos", e))?;
        let connection = Handle::new(WinHttpConnect(session.0, &HSTRING::from(HOST), INTERNET_DEFAULT_HTTPS_PORT, 0), "conectar")?;
        let request = Handle::new(
            WinHttpOpenRequest(connection.0, w!("GET"), &HSTRING::from(PATH), PCWSTR::null(), PCWSTR::null(), std::ptr::null(), WINHTTP_FLAG_SECURE),
            "preparar la petición",
        )?;
        let headers = format!("Authorization: Bearer {token}\r\nanthropic-beta: oauth-2025-04-20\r\nAccept: application/json\r\n");
        WinHttpAddRequestHeaders(request.0, &wide(&headers), WINHTTP_ADDREQ_FLAG_ADD).map_err(|e| net("cabeceras", e))?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| net("enviar", e))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| net("recibir", e))?;

        let mut code: u32 = 0;
        let mut size = size_of::<u32>() as u32;
        WinHttpQueryHeaders(request.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, PCWSTR::null(), Some(&mut code as *mut u32 as *mut c_void), &mut size, std::ptr::null_mut())
            .map_err(|e| net("estado", e))?;
        if code != 200 {
            let mut buf = [0u16; 64];
            let mut len = (buf.len() * 2) as u32;
            let retry_after = WinHttpQueryHeaders(request.0, WINHTTP_QUERY_CUSTOM, w!("Retry-After"), Some(buf.as_mut_ptr() as *mut c_void), &mut len, std::ptr::null_mut())
                .ok()
                .and_then(|_| parse_retry_after(&String::from_utf16_lossy(&buf[..(len as usize / 2).min(buf.len())])));
            return Err(FetchError::Status { code: code as u16, retry_after });
        }

        let mut body: Vec<u8> = Vec::new();
        loop {
            let mut available: u32 = 0;
            WinHttpQueryDataAvailable(request.0, &mut available).map_err(|e| net("leer", e))?;
            if available == 0 {
                break;
            }
            if body.len() + available as usize > MAX_BODY {
                return Err(FetchError::Network("respuesta demasiado grande".into()));
            }
            let start = body.len();
            body.resize(start + available as usize, 0);
            let mut read: u32 = 0;
            WinHttpReadData(request.0, body[start..].as_mut_ptr() as *mut c_void, available, &mut read).map_err(|e| net("leer", e))?;
            body.truncate(start + read as usize);
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}

/// El puerto real: la credencial de `~/.claude` y la petición HTTPS.
pub struct LivePort {
    pub paths: Paths,
}

impl UsagePort for LivePort {
    fn credential(&mut self) -> Option<Credential> {
        read_credential(&self.paths.claude_dir)
    }

    fn fetch(&mut self, token: &str) -> Result<String, FetchError> {
        http_get(token)
    }
}

// ----- Servicio global y hilo de lectura -----

static SERVICE: OnceLock<Arc<UsageService>> = OnceLock::new();

/// El servicio de la aplicación (se crea con el interruptor guardado).
pub fn service() -> Arc<UsageService> {
    SERVICE.get_or_init(|| Arc::new(UsageService::new(read_enabled(&Paths::from_env())))).clone()
}

/// Hilo que, mientras la función esté activada, lee cuando toca. Apagada, no hace nada (ni siquiera lee la credencial).
pub fn start_poller() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let svc = service();
    let _ = std::thread::Builder::new().name("wm-claude-usage".into()).spawn(move || {
        let mut port = LivePort { paths: Paths::from_env() };
        loop {
            std::thread::sleep(Duration::from_secs(15));
            if svc.is_enabled() {
                let now = now_secs();
                svc.attempt(&mut port, now, now * 1000, false);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_acepta_segundos_razonables_y_descarta_el_resto() {
        assert_eq!(parse_retry_after("120"), Some(120));
        assert_eq!(parse_retry_after(" 3600 \r\n"), Some(3600));
        for bad in ["", "Wed, 21 Oct 2026 07:28:00 GMT", "-5", "abc", "999999999"] {
            assert_eq!(parse_retry_after(bad), None, "{bad}");
        }
    }

    #[test]
    fn el_interruptor_sin_archivo_o_roto_esta_apagado_y_se_guarda() {
        let base = std::env::temp_dir().join(format!("tesela-usage-flag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let paths = Paths { data_dir: base.join("data"), claude_dir: base.join(".claude") };
        assert!(!read_enabled(&paths));
        write_enabled(&paths, true).unwrap();
        assert!(read_enabled(&paths));
        write_enabled(&paths, false).unwrap();
        assert!(!read_enabled(&paths));
        std::fs::write(flag_file(&paths), "{ roto").unwrap();
        assert!(!read_enabled(&paths));
    }

    #[test]
    fn la_credencial_se_lee_del_archivo_de_claude_code_y_sin_archivo_no_hay() {
        let base = std::env::temp_dir().join(format!("tesela-usage-cred-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        assert!(read_credential(&base).is_none());
        std::fs::write(base.join(".credentials.json"), r#"{"claudeAiOauth":{"accessToken":"abc","expiresAt":5}}"#).unwrap();
        assert_eq!(read_credential(&base).unwrap().expires_at_ms, Some(5));
    }

    /// Con la cuenta real y una petición de verdad. No imprime el token ni la respuesta completa. Solo a mano:
    /// `cargo test --lib live_claude_usage -- --ignored --nocapture`
    #[test]
    #[ignore = "hace una petición real a api.anthropic.com con tu credencial de Claude Code"]
    fn live_claude_usage_responde_con_la_cuenta_real() {
        let mut port = LivePort { paths: Paths::from_env() };
        let svc = UsageService::new(true);
        let now = now_secs();
        svc.attempt(&mut port, now, now * 1000, false);
        let view = svc.view();
        println!("estado: {:?} | problema: {:?}", view.status, view.problem);
        for w in &view.windows {
            println!("  {} = {:.0} % (reinicia en {:?} s)", w.window.label, w.window.percent, w.window.resets_at.map(|r| r.saturating_sub(now)));
        }
        println!("  desglose: {:?}", view.breakdown.iter().map(|s| format!("{} {:.0} %", s.name, s.percent)).collect::<Vec<_>>());
    }
}
