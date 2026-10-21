//! Qué está haciendo Claude Code ahora mismo: trabajando, esperando que el usuario responda o parado. Datos puros.
//!
//! Claude Code avisa de lo que ocurre con *hooks* (documentados en su configuración). Un pequeño script de Tesela anota cada
//! aviso en un archivo por sesión; aquí se interpreta ese archivo. Ni credenciales ni red: solo eventos locales.

use serde::{Deserialize, Serialize};

/// Una sesión sin avisos durante este tiempo y «trabajando» se da por parada: si el usuario interrumpe con Esc, Claude Code
/// puede no avisar del final.
pub const WORKING_STALE_SECS: u64 = 300;
/// Esperando al usuario (un permiso, una pregunta) caduca mucho más tarde: la persona puede tardar en volver.
pub const WAITING_STALE_SECS: u64 = 1800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Idle,
    Working,
    Waiting,
}

/// Un subagente en marcha dentro de una sesión (el que lanza Claude para una tarea: `Explore`, `Plan`, uno propio…).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    /// Nombre del tipo de agente.
    #[serde(rename = "type")]
    pub kind: String,
    /// Segundos desde 1970 de su último aviso.
    pub at: u64,
}

/// Lo que anota el script por cada aviso de Claude Code (un archivo por sesión).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookRecord {
    /// `hook_event_name` de Claude Code (`UserPromptSubmit`, `Stop`, `Notification`…).
    pub event: String,
    /// Solo en `Notification`: `permission_prompt`, `idle_prompt`…
    #[serde(default)]
    pub notification_type: Option<String>,
    /// Segundos desde 1970 en que llegó el aviso.
    pub at: u64,
    /// Nombre de la carpeta de trabajo de la sesión.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Desde dónde se usa Claude Code (`CLAUDE_CODE_ENTRYPOINT`: `cli`, `claude-vscode`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Subagentes en marcha.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<AgentRecord>,
}

/// Qué hace un aviso con el estado de su sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Set(Activity),
    /// La sesión terminó.
    End,
    /// No dice nada del estado.
    Ignore,
}

/// Lo que cada evento significa. `UserPromptSubmit`, `PreToolUse` y `PostToolUse` mantienen «trabajando» (también cuando se
/// aprueba un permiso, que vuelve a la actividad); `Stop` y los avisos de «ya terminó» lo dejan parado; un permiso o una pregunta
/// lo ponen en «esperando».
pub fn change_for(event: &str, notification_type: Option<&str>) -> Change {
    match event {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "SubagentStop" => Change::Set(Activity::Working),
        "Stop" | "StopFailure" | "SessionStart" => Change::Set(Activity::Idle),
        "SessionEnd" => Change::End,
        "Notification" => match notification_type {
            Some("permission_prompt" | "elicitation_dialog" | "elicitation_url_dialog" | "agent_needs_input") => Change::Set(Activity::Waiting),
            Some("idle_prompt" | "agent_completed") => Change::Set(Activity::Idle),
            _ => Change::Ignore,
        },
        _ => Change::Ignore,
    }
}

/// El estado de una sesión según su último aviso y cuánto hace de él. `None` si ya terminó o el aviso no cuenta.
pub fn session_activity(record: &HookRecord, now: u64) -> Option<Activity> {
    let activity = match change_for(&record.event, record.notification_type.as_deref()) {
        Change::Set(a) => a,
        Change::End | Change::Ignore => return None,
    };
    let age = now.saturating_sub(record.at);
    Some(match activity {
        Activity::Working if age > WORKING_STALE_SECS => Activity::Idle,
        Activity::Waiting if age > WAITING_STALE_SECS => Activity::Idle,
        other => other,
    })
}

/// El estado de todas las sesiones juntas: si alguna espera al usuario, eso manda (es lo que pide su atención); si no, si alguna trabaja.
pub fn overall(records: &[HookRecord], now: u64) -> Activity {
    let states: Vec<Activity> = records.iter().filter_map(|r| session_activity(r, now)).collect();
    if states.contains(&Activity::Waiting) {
        Activity::Waiting
    } else if states.contains(&Activity::Working) {
        Activity::Working
    } else {
        Activity::Idle
    }
}

/// Un agente en marcha, tal como se enseña en la tarjeta del uso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunningAgent {
    /// La carpeta del proyecto (la sesión) o el tipo de agente (un subagente).
    pub name: String,
    /// De dónde viene: «Claude Code · VS Code», «Subagente»…
    pub source: String,
    pub state: Activity,
    /// Es un subagente de la sesión que va justo antes.
    pub sub: bool,
}

/// Cómo se llama en pantalla el lugar desde el que se usa Claude Code. Solo se ve lo que corre en este equipo: los chats de la web
/// o de la aplicación no avisan a Tesela.
pub fn origin_label(origin: Option<&str>) -> String {
    match origin {
        None | Some("") => "Claude Code".into(),
        Some("cli") => "Claude Code · terminal".into(),
        Some("claude-vscode") => "Claude Code · VS Code".into(),
        Some("claude-desktop") => "Claude Code · app de escritorio".into(),
        Some(s) if s.contains("jetbrains") => "Claude Code · JetBrains".into(),
        Some(s) if s.starts_with("sdk") => "Claude Agent SDK".into(),
        Some(other) => format!("Claude Code · {other}"),
    }
}

/// Las sesiones que trabajan o esperan (cada una con sus subagentes, si está trabajando), por orden de proyecto.
pub fn running_agents(records: &[HookRecord], now: u64) -> Vec<RunningAgent> {
    let mut sessions: Vec<(&HookRecord, Activity)> = records
        .iter()
        .filter_map(|r| session_activity(r, now).filter(|a| *a != Activity::Idle).map(|a| (r, a)))
        .collect();
    sessions.sort_by(|a, b| a.0.project.cmp(&b.0.project).then(a.0.at.cmp(&b.0.at)));
    let mut out = Vec::new();
    for (record, state) in sessions {
        out.push(RunningAgent {
            name: record.project.clone().unwrap_or_else(|| "Sesión sin nombre".into()),
            source: origin_label(record.origin.as_deref()),
            state,
            sub: false,
        });
        if state == Activity::Working {
            let mut subs: Vec<&AgentRecord> = record.agents.iter().filter(|a| now.saturating_sub(a.at) <= WORKING_STALE_SECS).collect();
            subs.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.at.cmp(&b.at)));
            out.extend(subs.into_iter().map(|a| RunningAgent { name: a.kind.clone(), source: "Subagente".into(), state: Activity::Working, sub: true }));
        }
    }
    out
}

/// Interpreta el archivo de una sesión; `None` si está roto o a medio escribir.
pub fn parse_record(text: &str) -> Option<HookRecord> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(event: &str, kind: Option<&str>, at: u64) -> HookRecord {
        HookRecord { event: event.into(), notification_type: kind.map(str::to_string), at, ..Default::default() }
    }

    fn session(project: &str, origin: Option<&str>, event: &str, at: u64, agents: &[(&str, &str, u64)]) -> HookRecord {
        HookRecord {
            event: event.into(),
            at,
            project: Some(project.into()),
            origin: origin.map(str::to_string),
            agents: agents.iter().map(|(id, kind, at)| AgentRecord { id: (*id).into(), kind: (*kind).into(), at: *at }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn el_origen_se_escribe_para_personas() {
        assert_eq!(origin_label(Some("cli")), "Claude Code · terminal");
        assert_eq!(origin_label(Some("claude-vscode")), "Claude Code · VS Code");
        assert_eq!(origin_label(Some("claude-desktop")), "Claude Code · app de escritorio");
        assert_eq!(origin_label(Some("sdk-ts")), "Claude Agent SDK");
        assert_eq!(origin_label(Some("raro")), "Claude Code · raro");
        assert_eq!(origin_label(None), "Claude Code");
    }

    #[test]
    fn los_agentes_en_marcha_son_las_sesiones_activas_con_sus_subagentes() {
        let now = 10_000;
        let records = [
            session("Tesela", Some("claude-vscode"), "PreToolUse", now - 3, &[("a", "Explore", now - 2), ("b", "Plan", now - WORKING_STALE_SECS - 1)]),
            session("Web", Some("cli"), "Notification", now - 20, &[("c", "Explore", now - 10)]),
            session("Quieta", None, "Stop", now - 1, &[]),
        ];
        let mut with_wait = records.to_vec();
        with_wait[1].notification_type = Some("permission_prompt".into());
        let agents = running_agents(&with_wait, now);
        let view: Vec<(&str, &str, Activity, bool)> = agents.iter().map(|a| (a.name.as_str(), a.source.as_str(), a.state, a.sub)).collect();
        assert_eq!(
            view,
            [
                ("Tesela", "Claude Code · VS Code", Activity::Working, false),
                // El subagente viejo (sin avisos hace más de 5 min) ya no cuenta.
                ("Explore", "Subagente", Activity::Working, true),
                // Una sesión que espera no enseña subagentes.
                ("Web", "Claude Code · terminal", Activity::Waiting, false),
            ]
        );
        assert!(running_agents(&[], now).is_empty());
    }

    #[test]
    fn un_archivo_viejo_sin_proyecto_ni_origen_se_sigue_leyendo() {
        let r = parse_record(r#"{"event":"PreToolUse","at":5}"#).unwrap();
        assert_eq!((r.project, r.origin, r.agents.len()), (None, None, 0));
        let nuevo = parse_record(r#"{"event":"PreToolUse","at":5,"project":"X","origin":"cli","agents":[{"id":"1","type":"Plan","at":4}]}"#).unwrap();
        assert_eq!(nuevo.agents, [AgentRecord { id: "1".into(), kind: "Plan".into(), at: 4 }]);
    }

    #[test]
    fn cada_evento_dice_lo_que_hace() {
        assert_eq!(change_for("UserPromptSubmit", None), Change::Set(Activity::Working));
        assert_eq!(change_for("PreToolUse", None), Change::Set(Activity::Working));
        assert_eq!(change_for("PostToolUse", None), Change::Set(Activity::Working));
        assert_eq!(change_for("Stop", None), Change::Set(Activity::Idle));
        assert_eq!(change_for("StopFailure", None), Change::Set(Activity::Idle));
        assert_eq!(change_for("SessionEnd", None), Change::End);
        assert_eq!(change_for("Notification", Some("permission_prompt")), Change::Set(Activity::Waiting));
        assert_eq!(change_for("Notification", Some("elicitation_dialog")), Change::Set(Activity::Waiting));
        // «Lleva un rato esperando tu siguiente mensaje» es estar parado, no pedir atención.
        assert_eq!(change_for("Notification", Some("idle_prompt")), Change::Set(Activity::Idle));
        assert_eq!(change_for("Notification", Some("auth_success")), Change::Ignore);
        assert_eq!(change_for("Notification", None), Change::Ignore);
        assert_eq!(change_for("Cualquier", None), Change::Ignore);
    }

    #[test]
    fn trabajando_se_da_por_parado_si_no_hay_avisos_en_un_rato() {
        let r = rec("PreToolUse", None, 1_000);
        assert_eq!(session_activity(&r, 1_000 + WORKING_STALE_SECS), Some(Activity::Working));
        assert_eq!(session_activity(&r, 1_000 + WORKING_STALE_SECS + 1), Some(Activity::Idle));
    }

    #[test]
    fn esperando_aguanta_mucho_mas() {
        let r = rec("Notification", Some("permission_prompt"), 1_000);
        assert_eq!(session_activity(&r, 1_000 + 600), Some(Activity::Waiting));
        assert_eq!(session_activity(&r, 1_000 + WAITING_STALE_SECS + 1), Some(Activity::Idle));
    }

    #[test]
    fn una_sesion_terminada_o_un_aviso_que_no_cuenta_no_aportan() {
        assert_eq!(session_activity(&rec("SessionEnd", None, 5), 6), None);
        assert_eq!(session_activity(&rec("Notification", Some("auth_success"), 5), 6), None);
    }

    #[test]
    fn el_estado_total_prefiere_esperar_y_luego_trabajar() {
        let now = 10_000;
        assert_eq!(overall(&[], now), Activity::Idle);
        let work = rec("UserPromptSubmit", None, now - 5);
        let wait = rec("Notification", Some("permission_prompt"), now - 20);
        let done = rec("Stop", None, now - 1);
        assert_eq!(overall(std::slice::from_ref(&done), now), Activity::Idle);
        assert_eq!(overall(&[done.clone(), work.clone()], now), Activity::Working);
        assert_eq!(overall(&[work, wait, done], now), Activity::Waiting);
    }

    #[test]
    fn el_archivo_de_una_sesion_se_lee_con_o_sin_tipo_y_los_rotos_se_descartan() {
        assert_eq!(parse_record(r#"{"event":"Stop","at":12}"#), Some(rec("Stop", None, 12)));
        assert_eq!(parse_record("\u{feff}{\"event\":\"Notification\",\"notification_type\":\"permission_prompt\",\"at\":9}"), Some(rec("Notification", Some("permission_prompt"), 9)));
        assert_eq!(parse_record(r#"{"event":"Stop""#), None);
        assert_eq!(parse_record(""), None);
        assert_eq!(parse_record(r#"{"at":1}"#), None);
    }
}
