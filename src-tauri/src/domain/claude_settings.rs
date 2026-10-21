//! Cómo se añaden y se quitan los hooks de Tesela en el `settings.json` de Claude Code. Edición pura sobre el JSON, sin tocar el disco.
//!
//! Formato (documentación de Claude Code): `hooks` → evento → lista de grupos → `hooks` → `{ "type": "command", "command": … }`.
//! Se conserva todo lo que no es de Tesela, también los hooks que ya tuviera el usuario en esos mismos eventos.

use serde_json::{json, Map, Value};

/// Eventos con los que Tesela sabe si Claude Code trabaja, espera o ha parado.
pub const EVENTS: [&str; 10] = ["UserPromptSubmit", "PreToolUse", "PostToolUse", "Notification", "Stop", "StopFailure", "SessionStart", "SessionEnd", "SubagentStart", "SubagentStop"];

/// Parte de la ruta del script que identifica un comando como de Tesela.
const MARKER: &str = "tesela-claude-hook.js";

/// ¿Es este comando el hook de Tesela? (Se compara sin importar `\` o `/` ni mayúsculas.)
pub fn is_ours(command: &str) -> bool {
    command.replace('\\', "/").to_lowercase().contains(MARKER)
}

fn is_ours_handler(handler: &Value) -> bool {
    handler.get("command").and_then(Value::as_str).is_some_and(is_ours)
}

/// El comando que ejecuta Claude Code: Node con el script, con la ruta entre comillas y con `/` (Git Bash no entiende `\`).
pub fn hook_command(script: &str) -> String {
    format!("node \"{}\"", script.replace('\\', "/"))
}

fn handler(command: &str) -> Value {
    // `async`: Claude Code no espera al script, así que nunca lo retrasa.
    json!({ "type": "command", "command": command, "async": true })
}

/// Añade los hooks de Tesela. Si ya estaban, los deja una sola vez. Falla, sin cambiar nada, si el archivo no tiene la forma esperada.
/// Todo se edita en su sitio: las claves del usuario conservan su orden.
pub fn plan_install(settings: &Value, command: &str) -> Result<Value, String> {
    let mut root = match settings {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => return Err("settings.json no es un objeto JSON".into()),
    };
    if matches!(root.get("hooks"), None | Some(Value::Null)) {
        root.insert("hooks".into(), Value::Object(Map::new()));
    }
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Err("«hooks» de settings.json no es un objeto".into());
    };
    for event in EVENTS {
        let ours = json!({ "hooks": [handler(command)] });
        match hooks.get_mut(event) {
            None | Some(Value::Null) => {
                hooks.insert(event.into(), Value::Array(vec![ours]));
            }
            Some(Value::Array(groups)) => {
                // Sin duplicar: se quitan los nuestros que hubiera (de otra ruta o de una instalación anterior) antes de poner el nuevo.
                remove_ours(groups);
                groups.push(ours);
            }
            Some(_) => return Err(format!("«hooks.{event}» de settings.json no es una lista")),
        }
    }
    Ok(Value::Object(root))
}

/// Quita nuestros manejadores de una lista de grupos y los grupos que se queden vacíos por ello. Devuelve si cambió algo.
fn remove_ours(groups: &mut Vec<Value>) -> bool {
    let mut changed = false;
    for group in groups.iter_mut() {
        if let Some(Value::Array(list)) = group.get_mut("hooks") {
            let before = list.len();
            list.retain(|h| !is_ours_handler(h));
            changed |= list.len() != before;
        }
    }
    let before = groups.len();
    groups.retain(|g| !matches!(g.get("hooks"), Some(Value::Array(l)) if l.is_empty()));
    changed || groups.len() != before
}

/// Quita solo lo de Tesela. Lo que se vacía por ello (un evento, o `hooks` entero) también se quita; lo que ya estaba vacío, se deja.
pub fn plan_uninstall(settings: &Value) -> Value {
    let mut out = settings.clone();
    let Some(hooks) = out.get_mut("hooks").and_then(Value::as_object_mut) else { return out };
    let mut touched = false;
    for event in hooks.keys().cloned().collect::<Vec<_>>() {
        let Some(Value::Array(groups)) = hooks.get_mut(&event) else { continue };
        if remove_ours(groups) {
            touched = true;
            if groups.is_empty() {
                hooks.shift_remove(&event);
            }
        }
    }
    if touched && hooks.is_empty() {
        if let Some(root) = out.as_object_mut() {
            root.shift_remove("hooks");
        }
    }
    out
}

/// En cuántos de los eventos de Tesela está su hook.
pub fn installed_events(settings: &Value) -> usize {
    let Some(Value::Object(hooks)) = settings.get("hooks") else { return 0 };
    EVENTS
        .iter()
        .filter(|event| {
            hooks.get(**event).and_then(Value::as_array).is_some_and(|groups| {
                groups.iter().any(|g| g.get("hooks").and_then(Value::as_array).is_some_and(|l| l.iter().any(is_ours_handler)))
            })
        })
        .count()
}

/// ¿Están los hooks de Tesela en todos sus eventos?
pub fn is_installed(settings: &Value) -> bool {
    installed_events(settings) == EVENTS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: &str = r#"node "C:/Users/erick/AppData/Local/Tesela/claude/tesela-claude-hook.js""#;

    fn user_hook() -> Value {
        json!({ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo hola" }] })
    }

    #[test]
    fn el_comando_lleva_la_ruta_entre_comillas_y_con_barras_normales() {
        assert_eq!(hook_command(r"C:\Users\erick\AppData\Local\Tesela\claude\tesela-claude-hook.js"), CMD);
        assert!(is_ours(CMD));
        assert!(is_ours(r#"node "C:\Users\X\AppData\Local\TESELA\claude\tesela-claude-hook.js""#));
        assert!(!is_ours("echo hola"));
        assert!(!is_ours(r#"node "C:/otro/claude-hook.js""#));
    }

    #[test]
    fn instalar_en_un_archivo_vacio_pone_todos_los_eventos() {
        for empty in [json!({}), Value::Null] {
            let out = plan_install(&empty, CMD).unwrap();
            assert!(is_installed(&out));
            assert_eq!(installed_events(&out), EVENTS.len());
            let first = &out["hooks"]["Stop"][0]["hooks"][0];
            assert_eq!((first["type"].as_str(), first["command"].as_str(), first["async"].as_bool()), (Some("command"), Some(CMD), Some(true)));
        }
    }

    #[test]
    fn instalar_conserva_todo_lo_que_no_es_de_tesela() {
        let before = json!({ "model": "opus", "permissions": { "allow": ["Bash(ls)"] }, "hooks": { "PreToolUse": [user_hook()], "Otro": [{ "hooks": [] }] } });
        let out = plan_install(&before, CMD).unwrap();
        assert_eq!(out["model"], "opus");
        assert_eq!(out["permissions"], before["permissions"]);
        assert_eq!(out["hooks"]["Otro"], before["hooks"]["Otro"]);
        let pre = out["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0], user_hook());
    }

    #[test]
    fn instalar_dos_veces_o_con_otra_ruta_no_duplica() {
        let once = plan_install(&json!({}), CMD).unwrap();
        let twice = plan_install(&once, CMD).unwrap();
        assert_eq!(once, twice);
        let moved = plan_install(&once, r#"node "D:/x/Tesela/claude/tesela-claude-hook.js""#).unwrap();
        assert_eq!(moved["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(moved["hooks"]["Stop"][0]["hooks"][0]["command"], r#"node "D:/x/Tesela/claude/tesela-claude-hook.js""#);
    }

    #[test]
    fn un_archivo_con_otra_forma_no_se_toca() {
        assert!(plan_install(&json!([1]), CMD).is_err());
        assert!(plan_install(&json!({ "hooks": [] }), CMD).is_err());
        assert!(plan_install(&json!({ "hooks": { "Stop": "x" } }), CMD).is_err());
    }

    #[test]
    fn desinstalar_deja_el_archivo_como_estaba_antes_de_instalar() {
        for before in [
            json!({}),
            json!({ "model": "opus" }),
            json!({ "model": "opus", "hooks": { "PreToolUse": [user_hook()], "Stop": [{ "hooks": [{ "type": "command", "command": "afplay x" }] }] } }),
        ] {
            let installed = plan_install(&before, CMD).unwrap();
            assert_eq!(plan_uninstall(&installed), before, "{before}");
        }
    }

    #[test]
    fn desinstalar_no_borra_los_vacios_que_ya_eran_del_usuario_ni_toca_sin_hooks() {
        let user = json!({ "hooks": { "Stop": [] } });
        assert_eq!(plan_uninstall(&user), user);
        assert_eq!(plan_uninstall(&json!({ "a": 1 })), json!({ "a": 1 }));
        assert_eq!(plan_uninstall(&json!("raro")), json!("raro"));
        assert!(!is_installed(&json!({})));
    }

    #[test]
    fn una_instalacion_a_medias_se_detecta_y_se_puede_quitar() {
        let partial = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": CMD }] }] } });
        assert_eq!(installed_events(&partial), 1);
        assert!(!is_installed(&partial));
        assert_eq!(plan_uninstall(&partial), json!({}));
    }
}
