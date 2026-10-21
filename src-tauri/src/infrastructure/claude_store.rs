//! Lo que Tesela guarda y lee para Claude Code: los avisos de sus sesiones y la instalación (reversible) de los hooks en
//! `~/.claude/settings.json`.
//!
//! Reglas: el `settings.json` solo se toca cuando el usuario lo pide; antes de cada cambio se hace una copia con la fecha
//! (`settings.json.tesela-<segundos>.bak`); si el archivo no es JSON válido no se toca; y quitar los hooks quita solo lo de Tesela.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use crate::domain::claude_activity::{overall, parse_record, running_agents, Activity, HookRecord, RunningAgent};
use crate::domain::claude_settings::{hook_command, installed_events, is_installed, plan_install, plan_uninstall};

/// El script que ejecuta Claude Code en cada aviso. Va dentro del programa: así funciona también instalado.
const HOOK_SCRIPT: &str = include_str!("../../../scripts/claude-hook.cjs");

/// Los archivos de sesión más viejos que esto se borran: de sesiones que terminaron sin avisar.
const PRUNE_AFTER_SECS: u64 = 24 * 3600;

/// Dónde vive cada cosa. Se puede cambiar con `TESELA_CLAUDE_DIR` (datos de Tesela) y `TESELA_CLAUDE_HOME` (la carpeta `.claude`),
/// que usan las pruebas para no tocar nunca el `~/.claude` real.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `%LOCALAPPDATA%\Tesela\claude`: el script y los archivos de sesión.
    pub data_dir: PathBuf,
    /// `~/.claude`: donde están `settings.json` y las credenciales de Claude Code.
    pub claude_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Paths {
        let data_dir = std::env::var_os("TESELA_CLAUDE_DIR").map(PathBuf::from).unwrap_or_else(|| {
            let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            base.join("Tesela").join("claude")
        });
        let claude_dir = std::env::var_os("TESELA_CLAUDE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join(".claude"));
        Paths { data_dir, claude_dir }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.claude_dir.join("settings.json")
    }

    pub fn script_file(&self) -> PathBuf {
        self.data_dir.join("tesela-claude-hook.js")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.data_dir.join("sessions")
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ----- Estado de las sesiones -----

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityView {
    pub activity: Activity,
    /// Sesiones con aviso reciente (no terminadas).
    pub sessions: usize,
    /// Lo que está en marcha ahora (sesiones que trabajan o esperan, con sus subagentes).
    pub agents: Vec<RunningAgent>,
}

/// Lee los archivos de sesión y calcula el estado de Claude Code. Un archivo a medio escribir o roto se ignora (llegará otro).
pub fn read_activity(paths: &Paths, now: u64) -> ActivityView {
    let mut records: Vec<HookRecord> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(paths.sessions_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Some(record) = std::fs::read_to_string(&path).ok().and_then(|t| parse_record(&t)) else { continue };
            if now.saturating_sub(record.at) > PRUNE_AFTER_SECS {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            records.push(record);
        }
    }
    let activity = overall(&records, now);
    ActivityView { activity, sessions: records.len(), agents: running_agents(&records, now) }
}

/// Pone al día el script de los hooks si ya estaba instalado y el del programa es más nuevo (lo que anota va creciendo). No toca
/// `settings.json`, y si el script no estaba instalado no lo crea.
pub fn refresh_script(paths: &Paths) {
    let file = paths.script_file();
    if std::fs::read_to_string(&file).is_ok_and(|current| current != HOOK_SCRIPT) {
        let _ = write_atomic(&file, HOOK_SCRIPT);
    }
}

// ----- Instalación de los hooks -----

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HooksStatus {
    /// Los hooks de Tesela están en todos sus eventos.
    pub installed: bool,
    /// Están en algunos, pero no en todos (una instalación a medias): se puede reparar instalando o quitar.
    pub partial: bool,
    /// El `settings.json` que se toca.
    pub settings_path: String,
    /// El script que ejecuta Claude Code.
    pub script_path: String,
    /// Existe `settings.json` y es JSON válido (si no, instalar no se permite).
    pub settings_ok: bool,
}

fn read_settings(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("{} no es un JSON válido ({e}): no se toca", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(format!("no se pudo leer {}: {e}", path.display())),
    }
}

pub fn hooks_status(paths: &Paths) -> HooksStatus {
    let settings = read_settings(&paths.settings_file());
    let (installed, partial) = match &settings {
        Ok(s) => (is_installed(s), installed_events(s) > 0 && !is_installed(s)),
        Err(_) => (false, false),
    };
    HooksStatus {
        installed,
        partial,
        settings_path: paths.settings_file().to_string_lossy().to_string(),
        script_path: paths.script_file().to_string_lossy().to_string(),
        settings_ok: settings.is_ok(),
    }
}

/// Escribe a un archivo temporal y lo renombra: nadie ve nunca un archivo a medias.
fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension(format!("{}.tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("txt")));
    std::fs::write(&tmp, contents).map_err(|e| format!("no se pudo escribir {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo reemplazar {}: {e}", path.display())
    })
}

/// Copia `settings.json` antes de cambiarlo. Devuelve la copia (o `None` si todavía no existía).
fn backup(path: &Path, now: u64) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("settings.json");
    let copy = path.with_file_name(format!("{name}.tesela-{now}.bak"));
    std::fs::copy(path, &copy).map_err(|e| format!("no se pudo hacer la copia de seguridad: {e}"))?;
    Ok(Some(copy))
}

fn save_settings(paths: &Paths, value: &Value, now: u64) -> Result<(), String> {
    backup(&paths.settings_file(), now)?;
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    write_atomic(&paths.settings_file(), &text)
}

/// Pone el script en la carpeta de Tesela y los hooks en `settings.json`.
pub fn install_hooks(paths: &Paths, now: u64) -> Result<HooksStatus, String> {
    // Primero se comprueba que el archivo se puede tocar: si no, no se escribe nada.
    let settings = read_settings(&paths.settings_file())?;
    let command = hook_command(&paths.script_file().to_string_lossy());
    let planned = plan_install(&settings, &command)?;
    write_atomic(&paths.script_file(), HOOK_SCRIPT)?;
    save_settings(paths, &planned, now)?;
    Ok(hooks_status(paths))
}

/// Quita de `settings.json` los hooks de Tesela (y solo ellos). Si no había, no cambia nada ni hace copia.
pub fn uninstall_hooks(paths: &Paths, now: u64) -> Result<HooksStatus, String> {
    let settings = read_settings(&paths.settings_file())?;
    let planned = plan_uninstall(&settings);
    if planned != settings {
        save_settings(paths, &planned, now)?;
    }
    Ok(hooks_status(paths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp(name: &str) -> Paths {
        let base = std::env::temp_dir().join(format!("tesela-claude-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let paths = Paths { data_dir: base.join("data"), claude_dir: base.join("home").join(".claude") };
        std::fs::create_dir_all(&paths.claude_dir).unwrap();
        paths
    }

    fn put(paths: &Paths, id: &str, event: &str, kind: Option<&str>, at: u64) {
        std::fs::create_dir_all(paths.sessions_dir()).unwrap();
        let record = HookRecord { event: event.into(), notification_type: kind.map(str::to_string), at, ..Default::default() };
        std::fs::write(paths.sessions_dir().join(format!("{id}.json")), serde_json::to_string(&record).unwrap()).unwrap();
    }

    fn backups(paths: &Paths) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&paths.claude_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".bak")).collect();
        v.sort();
        v
    }

    /// Estado y número de sesiones, sin el detalle de los agentes.
    fn brief(v: ActivityView) -> (Activity, usize) {
        (v.activity, v.sessions)
    }

    #[test]
    fn sin_carpeta_de_sesiones_todo_esta_parado() {
        let p = temp("vacio");
        assert_eq!(read_activity(&p, 1_000), ActivityView { activity: Activity::Idle, sessions: 0, agents: vec![] });
    }

    #[test]
    fn el_script_viejo_se_pone_al_dia_solo_si_ya_estaba_instalado() {
        let p = temp("refrescar");
        refresh_script(&p);
        assert!(!p.script_file().exists(), "no se instala por su cuenta");
        std::fs::create_dir_all(&p.data_dir).unwrap();
        std::fs::write(p.script_file(), "// versión vieja").unwrap();
        refresh_script(&p);
        assert_eq!(std::fs::read_to_string(p.script_file()).unwrap(), HOOK_SCRIPT);
    }

    #[test]
    fn las_sesiones_se_leen_y_mandan_esperar_sobre_trabajar() {
        let p = temp("sesiones");
        put(&p, "a", "UserPromptSubmit", None, 990);
        assert_eq!(brief(read_activity(&p, 1_000)), (Activity::Working, 1));
        put(&p, "b", "Notification", Some("permission_prompt"), 995);
        assert_eq!(brief(read_activity(&p, 1_000)), (Activity::Waiting, 2));
        put(&p, "b", "PostToolUse", None, 998);
        assert_eq!(read_activity(&p, 1_000).activity, Activity::Working);
        put(&p, "a", "Stop", None, 999);
        put(&p, "b", "Stop", None, 999);
        assert_eq!(read_activity(&p, 1_000).activity, Activity::Idle);
    }

    #[test]
    fn un_archivo_roto_o_ajeno_se_ignora_y_los_muy_viejos_se_borran() {
        let p = temp("roto");
        put(&p, "ok", "UserPromptSubmit", None, 5_000);
        std::fs::write(p.sessions_dir().join("roto.json"), "{\"event\":").unwrap();
        std::fs::write(p.sessions_dir().join("nota.txt"), "x").unwrap();
        put(&p, "viejo", "UserPromptSubmit", None, 100);
        let now = 100 + PRUNE_AFTER_SECS + 10;
        let view = read_activity(&p, now.max(5_000));
        assert_eq!(view.sessions, 1);
        assert!(!p.sessions_dir().join("viejo.json").exists());
        assert!(p.sessions_dir().join("roto.json").exists(), "lo roto se deja: puede estar escribiéndose");
    }

    #[test]
    fn instalar_crea_el_script_los_hooks_y_la_copia_y_desinstalar_deja_el_archivo_como_estaba() {
        let p = temp("instalar");
        let original = json!({ "model": "opus", "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "echo fin" }] }] }, "env": { "A": "1" } });
        std::fs::write(p.settings_file(), serde_json::to_string_pretty(&original).unwrap()).unwrap();
        assert!(!hooks_status(&p).installed);

        let status = install_hooks(&p, 1_700_000_000).unwrap();
        assert!(status.installed && !status.partial && status.settings_ok);
        assert_eq!(std::fs::read_to_string(p.script_file()).unwrap(), HOOK_SCRIPT);
        assert_eq!(backups(&p), ["settings.json.tesela-1700000000.bak"]);
        // La copia es exactamente lo que había antes.
        let copy: Value = serde_json::from_str(&std::fs::read_to_string(p.claude_dir.join("settings.json.tesela-1700000000.bak")).unwrap()).unwrap();
        assert_eq!(copy, original);
        // Lo demás sobrevive, en el mismo orden de claves.
        let now_settings: Value = serde_json::from_str(&std::fs::read_to_string(p.settings_file()).unwrap()).unwrap();
        assert_eq!(now_settings["env"], original["env"]);
        assert_eq!(now_settings.as_object().unwrap().keys().take(3).collect::<Vec<_>>(), ["model", "hooks", "env"]);
        assert_eq!(now_settings["hooks"]["Stop"][0]["hooks"][0]["command"], "echo fin");

        let status = uninstall_hooks(&p, 1_700_000_100).unwrap();
        assert!(!status.installed);
        let after: Value = serde_json::from_str(&std::fs::read_to_string(p.settings_file()).unwrap()).unwrap();
        assert_eq!(after, original);
        assert_eq!(backups(&p).len(), 2);
    }

    #[test]
    fn instalar_sin_settings_lo_crea_y_sin_copia() {
        let p = temp("nuevo");
        install_hooks(&p, 5).unwrap();
        assert!(hooks_status(&p).installed);
        assert!(backups(&p).is_empty(), "no había nada que copiar");
        uninstall_hooks(&p, 6).unwrap();
        let after: Value = serde_json::from_str(&std::fs::read_to_string(p.settings_file()).unwrap()).unwrap();
        assert_eq!(after, json!({}));
    }

    #[test]
    fn un_settings_que_no_es_json_no_se_toca_ni_se_instala_nada() {
        let p = temp("roto-settings");
        std::fs::write(p.settings_file(), "{ \"model\": ").unwrap();
        assert!(install_hooks(&p, 1).unwrap_err().contains("no es un JSON válido"));
        assert!(uninstall_hooks(&p, 1).is_err());
        assert_eq!(std::fs::read_to_string(p.settings_file()).unwrap(), "{ \"model\": ");
        assert!(!p.script_file().exists(), "ni siquiera se copia el script");
        assert!(backups(&p).is_empty());
        let status = hooks_status(&p);
        assert!(!status.settings_ok && !status.installed);
    }

    #[test]
    fn instalar_dos_veces_es_inocuo_y_desinstalar_sin_hooks_no_hace_copia() {
        let p = temp("dos-veces");
        std::fs::write(p.settings_file(), "{\"a\": 1}").unwrap();
        install_hooks(&p, 10).unwrap();
        let first = std::fs::read_to_string(p.settings_file()).unwrap();
        install_hooks(&p, 11).unwrap();
        assert_eq!(std::fs::read_to_string(p.settings_file()).unwrap(), first);
        uninstall_hooks(&p, 12).unwrap();
        let before = backups(&p).len();
        uninstall_hooks(&p, 13).unwrap();
        assert_eq!(backups(&p).len(), before, "no había nada que quitar: no se toca ni se copia");
    }

    #[test]
    fn una_instalacion_a_medias_se_ve_como_tal() {
        let p = temp("medias");
        let partial = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": hook_command(&p.script_file().to_string_lossy()) }] }] } });
        std::fs::write(p.settings_file(), partial.to_string()).unwrap();
        let s = hooks_status(&p);
        assert!(!s.installed && s.partial);
        assert!(install_hooks(&p, 1).unwrap().installed);
    }

    /// De punta a punta con el script de verdad: lo que Claude Code mandaría por stdin llega al estado que lee Tesela.
    #[test]
    fn el_script_instalado_y_la_lectura_de_tesela_se_entienden() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if Command::new("node").arg("--version").output().is_err() {
            eprintln!("node no está instalado: se omite la prueba de punta a punta");
            return;
        }
        let p = temp("punta-a-punta");
        install_hooks(&p, 1).unwrap();
        let send = |json: &str| {
            let mut child = Command::new("node")
                .arg(p.script_file())
                .env("TESELA_CLAUDE_DIR", &p.data_dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(json.as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success() && out.stdout.is_empty() && out.stderr.is_empty());
        };
        let now = now_secs();
        assert_eq!(read_activity(&p, now).activity, Activity::Idle);
        send(r#"{"session_id":"abc","hook_event_name":"UserPromptSubmit","prompt":"hola"}"#);
        assert_eq!(brief(read_activity(&p, now_secs())), (Activity::Working, 1));
        send(r#"{"session_id":"abc","hook_event_name":"Notification","notification_type":"permission_prompt","message":"Claude needs your permission"}"#);
        assert_eq!(read_activity(&p, now_secs()).activity, Activity::Waiting);
        send(r#"{"session_id":"abc","hook_event_name":"PostToolUse","tool_name":"Bash"}"#);
        assert_eq!(read_activity(&p, now_secs()).activity, Activity::Working);
        send(r#"{"session_id":"abc","hook_event_name":"Stop"}"#);
        assert_eq!(read_activity(&p, now_secs()).activity, Activity::Idle);
        send(r#"{"session_id":"abc","hook_event_name":"SessionEnd"}"#);
        assert_eq!(brief(read_activity(&p, now_secs())), (Activity::Idle, 0));
        // Lo que se guardó no contiene el texto del prompt ni mensajes: solo evento, tipo y hora.
        send(r#"{"session_id":"abc","hook_event_name":"UserPromptSubmit","prompt":"secreto"}"#);
        let saved = std::fs::read_to_string(p.sessions_dir().join("abc.json")).unwrap();
        assert!(!saved.contains("secreto"), "{saved}");
    }
}
