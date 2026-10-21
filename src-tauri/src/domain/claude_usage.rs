//! Cuánto llevas gastado de tu suscripción de Claude: la ventana de 5 horas y la semanal. Datos puros (sin red ni archivos).
//!
//! La lectura llega de `GET https://api.anthropic.com/api/oauth/usage` (el mismo que usa Claude Code para su `/usage`). Ese
//! endpoint **no está documentado** por Anthropic: aquí se lee de forma defensiva (lo que falta o no cuadra se descarta, nunca se
//! inventa un porcentaje) y la forma exacta se comprobó contra una respuesta real de una cuenta Pro.

use serde::Serialize;
use serde_json::Value;

// ----- Lectura de la respuesta -----

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageWindow {
    /// `five_hour`, `seven_day`, `seven_day_opus`, `seven_day_sonnet`.
    pub id: String,
    pub label: String,
    /// 0 a 100, gastado.
    pub percent: f64,
    /// Segundos desde 1970 en que se reinicia, si la respuesta lo dice.
    pub resets_at: Option<u64>,
}

/// Desglose de la semana por producto (Claude Code, chats…), si la respuesta lo trae.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageShare {
    pub name: String,
    pub percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Usage {
    pub windows: Vec<UsageWindow>,
    pub breakdown: Vec<UsageShare>,
}

/// Ventanas conocidas, en el orden en que se enseñan: campo de la respuesta, identificador y nombre corto.
const KNOWN: [(&str, &str); 4] = [("five_hour", "5 h"), ("seven_day", "Semana"), ("seven_day_opus", "Semana Opus"), ("seven_day_sonnet", "Semana Sonnet")];

/// Equivalencias entre la lista `limits` (la forma más nueva) y los campos de siempre.
fn id_of_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "session" | "five_hour" => Some("five_hour"),
        "weekly_all" | "seven_day" | "weekly" => Some("seven_day"),
        "weekly_opus" | "seven_day_opus" => Some("seven_day_opus"),
        "weekly_sonnet" | "seven_day_sonnet" => Some("seven_day_sonnet"),
        _ => None,
    }
}

/// Un porcentaje válido: número finito, recortado a 0–100.
fn percent_of(v: &Value) -> Option<f64> {
    let n = v.as_f64().filter(|n| n.is_finite())?;
    Some(n.clamp(0.0, 100.0))
}

/// Segundos desde 1970 de una fecha RFC 3339 como `2026-10-21T01:29:59.938784+00:00` (o `Z`). `None` si no se entiende.
pub fn parse_rfc3339(text: &str) -> Option<u64> {
    let text = text.trim();
    let (date, rest) = text.split_once(['T', 't', ' '])?;
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    if d.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    // La hora termina donde empieza el desfase: `Z`, `+hh:mm` o `-hh:mm`.
    let at = rest.find(['Z', 'z', '+', '-']);
    let (clock, zone) = match at {
        Some(i) => rest.split_at(i),
        None => (rest, "Z"),
    };
    let mut c = clock.split(':');
    let hh: i64 = c.next()?.parse().ok()?;
    let mm: i64 = c.next()?.parse().ok()?;
    let ss: i64 = c.next().map(|s| s.split('.').next().unwrap_or("0")).unwrap_or("0").parse().ok()?;
    if c.next().is_some() || !(0..24).contains(&hh) || !(0..60).contains(&mm) || !(0..61).contains(&ss) {
        return None;
    }
    let offset = match zone {
        "Z" | "z" => 0,
        z => {
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let (oh, om) = z[1..].split_once(':').unwrap_or((&z[1..], "0"));
            sign * (oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60)
        }
    };
    // Días desde 1970 (algoritmo de fechas civiles de Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss - offset).ok()
}

fn window_from(id: &str, label: &str, utilization: &Value, resets: Option<&Value>) -> Option<UsageWindow> {
    Some(UsageWindow {
        id: id.into(),
        label: label.into(),
        percent: percent_of(utilization)?,
        resets_at: resets.and_then(Value::as_str).and_then(parse_rfc3339),
    })
}

/// Interpreta la respuesta del endpoint de uso. Falla si no trae ninguna ventana válida (no se inventa nada).
pub fn parse_usage(text: &str) -> Result<Usage, String> {
    let root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("la respuesta de uso no es JSON: {e}"))?;
    let mut windows: Vec<UsageWindow> = Vec::new();
    for (id, label) in KNOWN {
        if let Some(w) = root.get(id).filter(|v| v.is_object()).and_then(|v| window_from(id, label, v.get("utilization")?, v.get("resets_at"))) {
            windows.push(w);
        }
    }
    // La lista `limits` es la forma nueva: completa lo que los campos de siempre no traigan (una ventana recién reiniciada puede faltar en uno).
    if let Some(limits) = root.get("limits").and_then(Value::as_array) {
        for item in limits {
            let Some(id) = item.get("kind").and_then(Value::as_str).and_then(id_of_kind) else { continue };
            if windows.iter().any(|w| w.id == id) {
                continue;
            }
            let label = KNOWN.iter().find(|(k, _)| *k == id).map_or(id, |(_, l)| *l);
            if let Some(w) = item.get("percent").and_then(|p| window_from(id, label, p, item.get("resets_at"))) {
                windows.push(w);
            }
        }
    }
    windows.sort_by_key(|w| KNOWN.iter().position(|(k, _)| *k == w.id).unwrap_or(usize::MAX));
    if windows.is_empty() {
        return Err("la respuesta de uso no trae ninguna ventana".into());
    }
    let breakdown = root
        .pointer("/seven_day_breakdown/rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| Some(UsageShare { name: r.get("display_name")?.as_str()?.to_string(), percent: percent_of(r.get("percent")?)? }))
                .filter(|s| s.percent > 0.0)
                .collect()
        })
        .unwrap_or_default();
    Ok(Usage { windows, breakdown })
}

// ----- Nivel -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Ok,
    Warn,
    Danger,
}

/// Menos de 70 %: bien; de 70 a 89: atención; 90 o más: casi sin margen.
pub fn level(percent: f64) -> Level {
    if percent >= 90.0 {
        Level::Danger
    } else if percent >= 70.0 {
        Level::Warn
    } else {
        Level::Ok
    }
}

// ----- Credencial de Claude Code -----

/// Lo único que se usa de `~/.claude/.credentials.json`. El token nunca se imprime (`Debug` lo oculta) ni se guarda.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    pub access_token: String,
    /// Milisegundos desde 1970 en que caduca.
    pub expires_at_ms: Option<u64>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential").field("access_token", &"[oculto]").field("expires_at_ms", &self.expires_at_ms).finish()
    }
}

/// Lee el bloque de la suscripción (`claudeAiOauth`). `None` si no hay sesión de suscripción iniciada.
pub fn parse_credential(text: &str) -> Option<Credential> {
    let root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let oauth = root.get("claudeAiOauth")?;
    let access_token = oauth.get("accessToken")?.as_str()?.trim().to_string();
    // Un token con espacios, saltos de línea o vacío no es un token: nunca se enviaría tal cual dentro de una cabecera.
    if access_token.is_empty() || access_token.len() > 4096 || access_token.chars().any(|c| c.is_whitespace() || c.is_control() || !c.is_ascii()) {
        return None;
    }
    Some(Credential { access_token, expires_at_ms: oauth.get("expiresAt").and_then(Value::as_u64) })
}

/// Margen antes de caducar en el que ya no se envía el token.
const EXPIRY_MARGIN_MS: u64 = 30_000;

/// ¿Sirve todavía? Un token caducado no se envía: el endpoint contesta 429 y parecería un límite de uso.
pub fn token_usable(credential: &Credential, now_ms: u64) -> bool {
    credential.expires_at_ms.is_none_or(|exp| exp > now_ms + EXPIRY_MARGIN_MS)
}

// ----- Cuándo volver a preguntar -----

/// Mínimo entre dos lecturas (también tras un fallo): el endpoint es de uso interno y no se le insiste.
pub const POLL_SECS: u64 = 300;
const BACKOFF_BASE_SECS: u64 = 60;
const BACKOFF_CAP_SECS: u64 = 900;

/// Cuánto esperar tras el fallo número `failures` (1, 2, 3…): 1 min, 2, 4… hasta 15 min. Si el servidor pidió esperar (`Retry-After`),
/// se respeta aunque pase del tope; nunca acorta la espera.
pub fn retry_delay_secs(failures: u32, retry_after: Option<u64>) -> u64 {
    let backoff = BACKOFF_BASE_SECS.saturating_mul(1u64 << failures.saturating_sub(1).min(10)).min(BACKOFF_CAP_SECS);
    backoff.max(retry_after.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorte de una respuesta real (cuenta Pro, 20-oct-2026) con los campos que no se usan.
    const REAL: &str = r#"{
      "five_hour": {"utilization": 35, "resets_at": "2026-10-21T01:29:59.938784+00:00", "limit_dollars": null},
      "seven_day": {"utilization": 24, "resets_at": "2026-10-26T04:59:59.938806+00:00", "limit_dollars": null},
      "seven_day_opus": null, "seven_day_sonnet": null, "tangelo": null,
      "extra_usage": {"is_enabled": true, "used_credits": 0},
      "limits": [
        {"kind": "session", "group": "session", "percent": 35, "resets_at": "2026-10-21T01:29:59.938784+00:00"},
        {"kind": "weekly_all", "group": "weekly", "percent": 24, "resets_at": "2026-10-26T04:59:59.938806+00:00"}
      ],
      "seven_day_breakdown": {"rows": [
        {"key": "claude_code", "display_name": "Claude Code", "percent": 88},
        {"key": "chat", "display_name": "Chats", "percent": 1},
        {"key": "cowork", "display_name": "Cowork", "percent": 11},
        {"key": "other", "display_name": "Other", "percent": 0}
      ]}
    }"#;

    #[test]
    fn la_respuesta_real_se_lee_sin_duplicar_las_ventanas() {
        let u = parse_usage(REAL).unwrap();
        assert_eq!(u.windows.len(), 2, "{u:?}");
        assert_eq!((u.windows[0].id.as_str(), u.windows[0].label.as_str(), u.windows[0].percent), ("five_hour", "5 h", 35.0));
        assert_eq!((u.windows[1].id.as_str(), u.windows[1].label.as_str(), u.windows[1].percent), ("seven_day", "Semana", 24.0));
        assert_eq!(u.windows[0].resets_at, Some(1_792_546_199));
        assert_eq!(u.windows[1].resets_at, Some(1_792_990_799));
        // El desglose omite lo que está en 0 %.
        assert_eq!(u.breakdown.iter().map(|s| (s.name.as_str(), s.percent)).collect::<Vec<_>>(), [("Claude Code", 88.0), ("Chats", 1.0), ("Cowork", 11.0)]);
    }

    #[test]
    fn una_ventana_que_falta_en_los_campos_se_completa_con_la_lista_limits() {
        let text = r#"{"five_hour": null, "seven_day": {"utilization": 10, "resets_at": "2026-10-26T04:59:59Z"},
            "limits": [{"kind": "session", "percent": 50, "resets_at": "2026-10-21T01:29:59Z"}, {"kind": "otra_cosa", "percent": 5}]}"#;
        let u = parse_usage(text).unwrap();
        assert_eq!(u.windows.iter().map(|w| (w.id.as_str(), w.percent)).collect::<Vec<_>>(), [("five_hour", 50.0), ("seven_day", 10.0)]);
    }

    #[test]
    fn los_valores_raros_se_recortan_o_se_descartan() {
        let u = parse_usage(r#"{"five_hour": {"utilization": 130.5}, "seven_day": {"utilization": -4}, "seven_day_opus": {"utilization": "mucho"}, "seven_day_sonnet": {"resets_at": "x"}}"#).unwrap();
        assert_eq!(u.windows.iter().map(|w| (w.id.as_str(), w.percent, w.resets_at)).collect::<Vec<_>>(), [("five_hour", 100.0, None), ("seven_day", 0.0, None)]);
    }

    #[test]
    fn sin_ventanas_validas_o_con_basura_es_un_error_y_nunca_un_porcentaje_inventado() {
        for bad in ["", "no es json", "[]", "null", "{}", r#"{"five_hour": null, "limits": []}"#, r#"{"five_hour": {"utilization": null}}"#] {
            assert!(parse_usage(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn las_fechas_rfc3339_se_leen_con_cualquier_desfase() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2026-10-21T01:29:59.938784+00:00"), Some(1_792_546_199));
        assert_eq!(parse_rfc3339("2026-10-20T19:29:59-06:00"), Some(1_792_546_199));
        assert_eq!(parse_rfc3339("2026-10-21T03:29:59+02:00"), Some(1_792_546_199));
        assert_eq!(parse_rfc3339("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(parse_rfc3339("2000-03-01T00:00:00Z"), Some(951_868_800));
        for bad in ["", "ayer", "2026-13-01T00:00:00Z", "2026-10-21", "2026-10-21T25:00:00Z", "2026-10-21T01:00Z+x", "1969-12-31T23:59:59Z"] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn los_umbrales_son_70_y_90() {
        assert_eq!(level(0.0), Level::Ok);
        assert_eq!(level(69.9), Level::Ok);
        assert_eq!(level(70.0), Level::Warn);
        assert_eq!(level(89.9), Level::Warn);
        assert_eq!(level(90.0), Level::Danger);
        assert_eq!(level(100.0), Level::Danger);
    }

    #[test]
    fn la_credencial_se_lee_y_su_token_no_aparece_al_imprimirla() {
        let c = parse_credential(r#"{"claudeAiOauth": {"accessToken": "sk-ant-oat01-SECRETO", "refreshToken": "otro", "expiresAt": 1792560000000, "subscriptionType": "pro"}, "mcpOAuth": {}}"#).unwrap();
        assert_eq!(c.expires_at_ms, Some(1_792_560_000_000));
        assert_eq!(c.access_token, "sk-ant-oat01-SECRETO");
        let shown = format!("{c:?}");
        assert!(!shown.contains("SECRETO") && shown.contains("[oculto]"), "{shown}");
    }

    #[test]
    fn una_credencial_sin_suscripcion_o_con_un_token_raro_no_sirve() {
        for bad in [
            "",
            "{}",
            r#"{"mcpOAuth": {}}"#,
            r#"{"claudeAiOauth": {}}"#,
            r#"{"claudeAiOauth": {"accessToken": ""}}"#,
            r#"{"claudeAiOauth": {"accessToken": "con espacio"}}"#,
            "{\"claudeAiOauth\": {\"accessToken\": \"a\\nb\"}}",
            r#"{"claudeAiOauth": {"accessToken": "ñandú"}}"#,
        ] {
            assert!(parse_credential(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn un_token_caducado_o_a_punto_de_caducar_no_se_envia() {
        let c = |exp| Credential { access_token: "t".into(), expires_at_ms: exp };
        let now = 1_000_000;
        assert!(token_usable(&c(Some(now + 60_000)), now));
        assert!(!token_usable(&c(Some(now + 30_000)), now));
        assert!(!token_usable(&c(Some(now - 1)), now));
        assert!(token_usable(&c(None), now), "sin fecha de caducidad se intenta");
    }

    #[test]
    fn tras_un_fallo_se_espera_cada_vez_mas_con_tope_y_se_respeta_retry_after() {
        assert_eq!(retry_delay_secs(1, None), 60);
        assert_eq!(retry_delay_secs(2, None), 120);
        assert_eq!(retry_delay_secs(3, None), 240);
        assert_eq!(retry_delay_secs(5, None), 900);
        assert_eq!(retry_delay_secs(40, None), 900);
        assert_eq!(retry_delay_secs(1, Some(3600)), 3600, "Retry-After manda aunque pase el tope");
        assert_eq!(retry_delay_secs(1, Some(10)), 60, "y nunca acorta la espera");
        assert_eq!(retry_delay_secs(0, None), 60);
    }
}
