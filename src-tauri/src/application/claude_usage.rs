//! Caso de uso «uso de Claude»: cuándo preguntar, qué hacer con la respuesta y qué enseñar. Es una función que el usuario activa en Ajustes
//! (desactivada de fábrica): mientras está apagada no se lee la credencial ni se toca la red.
//!
//! Reglas de la lectura:
//! - la credencial es la que Claude Code ya guarda; se lee en cada intento y no se guarda ni se muestra;
//! - un token caducado no se envía (no hay red hasta que Claude Code lo renueve);
//! - como máximo una lectura cada 5 minutos; tras un fallo se espera cada vez más (y lo que pida `Retry-After`);
//! - si algo falla, se conserva la última lectura marcada como vieja: jamás se inventa un porcentaje.

use std::sync::Mutex;

use serde::Serialize;

use crate::domain::claude_usage::{level, parse_usage, retry_delay_secs, token_usable, Credential, Level, Usage, UsageShare, UsageWindow, POLL_SECS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// El servidor contestó con este estado (y, si lo dijo, esperar tantos segundos).
    Status { code: u16, retry_after: Option<u64> },
    /// No hubo respuesta (sin conexión, tiempo agotado…).
    Network(String),
}

/// Lo que el caso de uso necesita del mundo exterior.
pub trait UsagePort: Send {
    /// La credencial de Claude Code; `None` si no hay sesión de suscripción iniciada.
    fn credential(&mut self) -> Option<Credential>;
    /// Pide el uso con ese token y devuelve el cuerpo de la respuesta.
    fn fetch(&mut self, token: &str) -> Result<String, FetchError>;
}

/// Por qué no hay una lectura al día.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Problem {
    /// Claude Code no tiene sesión de suscripción en este equipo.
    SignedOut,
    /// El token caducó: se renueva solo al usar Claude Code.
    Expired,
    /// La sesión fue rechazada (401).
    Rejected,
    /// Acceso denegado (403): no quiere decir que la sesión esté perdida.
    Forbidden,
    /// Demasiadas peticiones (429).
    RateLimited,
    /// Sin conexión.
    Offline,
    /// Cualquier otra cosa: respuesta rara o error del servidor.
    Unexpected,
}

impl Problem {
    fn message(&self, detail: &str) -> String {
        match self {
            Problem::SignedOut => "Claude Code no tiene sesión iniciada con tu suscripción en este equipo.".into(),
            Problem::Expired => "La sesión de Claude Code caducó. Se renueva sola al usar Claude Code.".into(),
            Problem::Rejected => "Claude rechazó la sesión de Claude Code. Vuelve a iniciar sesión en Claude Code.".into(),
            Problem::Forbidden => "Claude negó el acceso (403). Puede ser la red o la cuenta; la sesión no tiene por qué estar perdida.".into(),
            Problem::RateLimited => "Claude pidió esperar antes de volver a preguntar.".into(),
            Problem::Offline => "Sin conexión con Claude.".into(),
            Problem::Unexpected => format!("Respuesta inesperada de Claude{}.", if detail.is_empty() { String::new() } else { format!(" ({detail})") }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowView {
    #[serde(flatten)]
    pub window: UsageWindow,
    pub level: Level,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// La función está apagada.
    Off,
    /// Encendida, todavía sin ninguna lectura.
    Waiting,
    /// Lectura al día.
    Ok,
    /// Hay una lectura anterior, pero el último intento falló.
    Stale,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageView {
    pub enabled: bool,
    pub status: Status,
    pub windows: Vec<WindowView>,
    pub breakdown: Vec<UsageShare>,
    /// Segundos desde 1970 de la última lectura buena.
    pub fetched_at: Option<u64>,
    pub problem: Option<Problem>,
    pub message: Option<String>,
}

#[derive(Default)]
struct State {
    enabled: bool,
    reading: Option<(Usage, u64)>,
    problem: Option<(Problem, String)>,
    failures: u32,
    /// No preguntar antes de este momento (segundos desde 1970).
    next_attempt: u64,
    /// El servidor pidió esperar hasta aquí: ni siquiera una actualización manual lo salta.
    blocked_until: u64,
    last_attempt: u64,
    in_flight: bool,
}

pub struct UsageService {
    state: Mutex<State>,
}

/// Una actualización manual no insiste antes de este tiempo desde el último intento.
const MANUAL_FLOOR_SECS: u64 = 60;
/// Con el token caducado se vuelve a mirar el archivo (sin red) a este ritmo: Claude Code puede renovarlo en cualquier momento.
const EXPIRED_RECHECK_SECS: u64 = 60;

impl UsageService {
    pub fn new(enabled: bool) -> UsageService {
        UsageService { state: Mutex::new(State { enabled, ..State::default() }) }
    }

    pub fn is_enabled(&self) -> bool {
        self.state.lock().unwrap().enabled
    }

    /// Enciende o apaga. Al apagar se olvida lo leído; al encender se lee en el siguiente aviso del reloj.
    pub fn set_enabled(&self, enabled: bool) {
        let mut s = self.state.lock().unwrap();
        s.enabled = enabled;
        if enabled {
            s.next_attempt = 0;
            s.blocked_until = 0;
            s.failures = 0;
        } else {
            *s = State { enabled: false, in_flight: s.in_flight, ..State::default() };
        }
    }

    pub fn view(&self) -> UsageView {
        let s = self.state.lock().unwrap();
        let (windows, breakdown, fetched_at) = match &s.reading {
            Some((usage, at)) => (usage.windows.iter().map(|w| WindowView { window: w.clone(), level: level(w.percent) }).collect(), usage.breakdown.clone(), Some(*at)),
            None => (Vec::new(), Vec::new(), None),
        };
        let status = match (s.enabled, s.reading.is_some(), s.problem.is_some()) {
            (false, _, _) => Status::Off,
            (true, false, _) => Status::Waiting,
            (true, true, false) => Status::Ok,
            (true, true, true) => Status::Stale,
        };
        UsageView {
            enabled: s.enabled,
            status,
            windows,
            breakdown,
            fetched_at,
            problem: s.problem.as_ref().map(|(p, _)| p.clone()),
            message: s.problem.as_ref().map(|(_, m)| m.clone()),
        }
    }

    /// Hace una lectura si toca. `manual`: la pidió la persona (se salta la espera normal, no la que pidió el servidor).
    pub fn attempt(&self, port: &mut dyn UsagePort, now_secs: u64, now_ms: u64, manual: bool) {
        {
            let mut s = self.state.lock().unwrap();
            let due = if manual { now_secs >= s.last_attempt + MANUAL_FLOOR_SECS } else { now_secs >= s.next_attempt };
            if !s.enabled || s.in_flight || now_secs < s.blocked_until || !due {
                return;
            }
            s.in_flight = true;
            s.last_attempt = now_secs;
        }
        let outcome = self.read(port, now_ms);
        let mut s = self.state.lock().unwrap();
        s.in_flight = false;
        if !s.enabled {
            return; // se apagó mientras se leía: no se guarda nada
        }
        match outcome {
            Ok(usage) => {
                s.reading = Some((usage, now_secs));
                s.problem = None;
                s.failures = 0;
                s.next_attempt = now_secs + POLL_SECS;
            }
            Err((problem, detail, retry_after)) => {
                let delay = match problem {
                    // Sin sesión o con el token caducado no hay petición: solo se vuelve a mirar el archivo.
                    Problem::SignedOut | Problem::Expired => EXPIRED_RECHECK_SECS,
                    _ => {
                        s.failures += 1;
                        retry_delay_secs(s.failures, retry_after)
                    }
                };
                if let Some(wait) = retry_after {
                    s.blocked_until = now_secs + wait;
                }
                s.next_attempt = now_secs + delay;
                let message = problem.message(&detail);
                s.problem = Some((problem, message));
            }
        }
    }

    /// La lectura en sí, sin tocar el estado (no se mantiene el candado durante la red).
    fn read(&self, port: &mut dyn UsagePort, now_ms: u64) -> Result<Usage, (Problem, String, Option<u64>)> {
        let Some(mut credential) = port.credential() else { return Err((Problem::SignedOut, String::new(), None)) };
        if !token_usable(&credential, now_ms) {
            return Err((Problem::Expired, String::new(), None));
        }
        let mut retried = false;
        loop {
            match port.fetch(&credential.access_token) {
                Ok(body) => return parse_usage(&body).map_err(|e| (Problem::Unexpected, e, None)),
                // 401: puede que Claude Code acabe de renovar el token. Se vuelve a leer el archivo y, si cambió, se reintenta una vez.
                Err(FetchError::Status { code: 401, .. }) if !retried => {
                    retried = true;
                    match port.credential() {
                        Some(fresh) if fresh.access_token != credential.access_token && token_usable(&fresh, now_ms) => credential = fresh,
                        _ => return Err((Problem::Rejected, String::new(), None)),
                    }
                }
                Err(FetchError::Status { code: 401, .. }) => return Err((Problem::Rejected, String::new(), None)),
                Err(FetchError::Status { code: 403, .. }) => return Err((Problem::Forbidden, String::new(), None)),
                Err(FetchError::Status { code: 429, retry_after }) => return Err((Problem::RateLimited, String::new(), retry_after.or(Some(0)).filter(|n| *n > 0))),
                Err(FetchError::Status { code, .. }) => return Err((Problem::Unexpected, format!("HTTP {code}"), None)),
                Err(FetchError::Network(_)) => return Err((Problem::Offline, String::new(), None)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"five_hour": {"utilization": 35, "resets_at": "2026-10-21T01:29:59Z"}, "seven_day": {"utilization": 91.5, "resets_at": "2026-10-26T04:59:59Z"}}"#;

    struct Fake {
        credential: Vec<Option<Credential>>,
        replies: Vec<Result<String, FetchError>>,
        fetched_with: Vec<String>,
        credential_reads: usize,
    }

    impl Fake {
        fn new(replies: Vec<Result<String, FetchError>>) -> Fake {
            Fake { credential: vec![Some(cred("tok-1", Some(9_000_000_000_000)))], replies, fetched_with: vec![], credential_reads: 0 }
        }
    }

    impl UsagePort for Fake {
        fn credential(&mut self) -> Option<Credential> {
            self.credential_reads += 1;
            let i = (self.credential_reads - 1).min(self.credential.len() - 1);
            self.credential[i].clone()
        }
        fn fetch(&mut self, token: &str) -> Result<String, FetchError> {
            self.fetched_with.push(token.into());
            self.replies.remove(0)
        }
    }

    fn cred(token: &str, expires: Option<u64>) -> Credential {
        Credential { access_token: token.into(), expires_at_ms: expires }
    }

    fn status(code: u16) -> FetchError {
        FetchError::Status { code, retry_after: None }
    }

    const NOW: u64 = 1_000_000;
    const NOW_MS: u64 = NOW * 1000;

    fn on() -> UsageService {
        UsageService::new(true)
    }

    #[test]
    fn apagado_no_lee_la_credencial_ni_toca_la_red() {
        let svc = UsageService::new(false);
        let mut port = Fake::new(vec![]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        svc.attempt(&mut port, NOW, NOW_MS, true);
        assert_eq!((port.credential_reads, port.fetched_with.len()), (0, 0));
        assert_eq!(svc.view().status, Status::Off);
    }

    #[test]
    fn una_lectura_buena_se_enseña_con_su_nivel_y_espera_cinco_minutos() {
        let svc = on();
        assert_eq!(svc.view().status, Status::Waiting);
        let mut port = Fake::new(vec![Ok(BODY.into()), Ok(BODY.into())]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        let v = svc.view();
        assert_eq!((v.status, v.fetched_at, v.problem.is_none()), (Status::Ok, Some(NOW), true));
        assert_eq!(v.windows.iter().map(|w| (w.window.id.as_str(), w.level)).collect::<Vec<_>>(), [("five_hour", Level::Ok), ("seven_day", Level::Danger)]);
        assert_eq!(port.fetched_with, ["tok-1"]);
        // Antes de los 5 minutos no vuelve a preguntar; pasados, sí.
        svc.attempt(&mut port, NOW + POLL_SECS - 1, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 1);
        svc.attempt(&mut port, NOW + POLL_SECS, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 2);
    }

    #[test]
    fn sin_sesion_o_con_el_token_caducado_no_hay_peticion() {
        let svc = on();
        let mut port = Fake::new(vec![]);
        port.credential = vec![None];
        svc.attempt(&mut port, NOW, NOW_MS, false);
        assert_eq!(svc.view().problem, Some(Problem::SignedOut));
        assert!(port.fetched_with.is_empty());

        let svc = on();
        let mut port = Fake::new(vec![]);
        port.credential = vec![Some(cred("viejo", Some(NOW_MS - 1)))];
        svc.attempt(&mut port, NOW, NOW_MS, false);
        assert_eq!(svc.view().problem, Some(Problem::Expired));
        assert!(port.fetched_with.is_empty(), "un token caducado nunca se envía");
        // Se vuelve a mirar el archivo al minuto, sin esperar los 5 minutos.
        port.credential = vec![Some(cred("nuevo", Some(NOW_MS + 3_600_000)))];
        port.replies = vec![Ok(BODY.into())];
        svc.attempt(&mut port, NOW + EXPIRED_RECHECK_SECS, NOW_MS + 60_000, false);
        assert_eq!(svc.view().status, Status::Ok);
        assert_eq!(port.fetched_with, ["nuevo"]);
    }

    #[test]
    fn si_falla_se_conserva_la_ultima_lectura_marcada_como_vieja_y_se_espera_cada_vez_mas() {
        let svc = on();
        let mut port = Fake::new(vec![Ok(BODY.into()), Err(FetchError::Network("x".into())), Err(status(500)), Ok(BODY.into())]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        let t1 = NOW + POLL_SECS;
        svc.attempt(&mut port, t1, NOW_MS, false);
        let v = svc.view();
        assert_eq!((v.status, v.problem, v.fetched_at), (Status::Stale, Some(Problem::Offline), Some(NOW)));
        assert_eq!(v.windows.len(), 2, "la lectura anterior sigue ahí");
        // Primer fallo: 60 s. Segundo: 120 s.
        svc.attempt(&mut port, t1 + 59, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 2);
        let t2 = t1 + 60;
        svc.attempt(&mut port, t2, NOW_MS, false);
        assert_eq!(svc.view().problem, Some(Problem::Unexpected));
        assert!(svc.view().message.unwrap().contains("HTTP 500"));
        svc.attempt(&mut port, t2 + 119, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 3);
        svc.attempt(&mut port, t2 + 120, NOW_MS, false);
        assert_eq!((svc.view().status, svc.view().fetched_at), (Status::Ok, Some(t2 + 120)));
    }

    #[test]
    fn un_429_con_retry_after_se_respeta_aunque_se_pida_a_mano() {
        let svc = on();
        let mut port = Fake::new(vec![Err(FetchError::Status { code: 429, retry_after: Some(3600) }), Ok(BODY.into())]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        assert_eq!(svc.view().problem, Some(Problem::RateLimited));
        svc.attempt(&mut port, NOW + 600, NOW_MS, true);
        svc.attempt(&mut port, NOW + 3599, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 1, "ni a mano ni sola antes de la hora que pidió");
        svc.attempt(&mut port, NOW + 3600, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 2);
    }

    #[test]
    fn un_401_se_reintenta_una_vez_si_claude_code_acaba_de_renovar_el_token() {
        let svc = on();
        let mut port = Fake::new(vec![Err(status(401)), Ok(BODY.into())]);
        port.credential = vec![Some(cred("tok-1", Some(NOW_MS + 3_600_000))), Some(cred("tok-2", Some(NOW_MS + 3_600_000)))];
        svc.attempt(&mut port, NOW, NOW_MS, false);
        assert_eq!(port.fetched_with, ["tok-1", "tok-2"]);
        assert_eq!(svc.view().status, Status::Ok);

        // Si el token sigue siendo el mismo, es un rechazo de verdad y no se insiste.
        let svc = on();
        let mut port = Fake::new(vec![Err(status(401)), Err(status(401))]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 1);
        assert_eq!(svc.view().problem, Some(Problem::Rejected));
    }

    #[test]
    fn el_403_no_se_toma_por_sesion_perdida() {
        let svc = on();
        let mut port = Fake::new(vec![Err(status(403))]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        let v = svc.view();
        assert_eq!(v.problem, Some(Problem::Forbidden));
        assert!(v.message.unwrap().contains("no tiene por qué estar perdida"));
    }

    #[test]
    fn una_respuesta_sin_ventanas_es_un_error_y_no_borra_la_lectura_anterior() {
        let svc = on();
        let mut port = Fake::new(vec![Ok(BODY.into()), Ok("{}".into())]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        svc.attempt(&mut port, NOW + POLL_SECS, NOW_MS, false);
        let v = svc.view();
        assert_eq!((v.status, v.problem, v.windows.len()), (Status::Stale, Some(Problem::Unexpected), 2));
    }

    #[test]
    fn actualizar_a_mano_se_salta_la_espera_normal_pero_no_insiste_cada_segundo() {
        let svc = on();
        let mut port = Fake::new(vec![Ok(BODY.into()), Ok(BODY.into()), Ok(BODY.into())]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        svc.attempt(&mut port, NOW + 10, NOW_MS, true);
        assert_eq!(port.fetched_with.len(), 1, "demasiado pronto");
        svc.attempt(&mut port, NOW + MANUAL_FLOOR_SECS, NOW_MS, true);
        assert_eq!(port.fetched_with.len(), 2);
    }

    #[test]
    fn apagar_olvida_la_lectura_y_encender_vuelve_a_leer_enseguida() {
        let svc = on();
        let mut port = Fake::new(vec![Ok(BODY.into()), Ok(BODY.into())]);
        svc.attempt(&mut port, NOW, NOW_MS, false);
        svc.set_enabled(false);
        let v = svc.view();
        assert_eq!((v.status, v.windows.len(), v.fetched_at), (Status::Off, 0, None));
        svc.attempt(&mut port, NOW + POLL_SECS, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 1);
        svc.set_enabled(true);
        svc.attempt(&mut port, NOW + 1, NOW_MS, false);
        assert_eq!(port.fetched_with.len(), 2);
    }
}
