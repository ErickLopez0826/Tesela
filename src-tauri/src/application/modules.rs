//! API del núcleo para los módulos (§10.4) y comprobación de permisos (§10.3).
//!
//! Los módulos no tocan Windows: piden operaciones de alto nivel por nombre y el núcleo decide si se las
//! da según los permisos CONCEDIDOS (no los pedidos). Un módulo que falla no tumba el núcleo.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::application::channels::{is_valid_channel, ChannelBackend, ChannelMessage, MAX_MESSAGE_CHARS};
use crate::application::system::SystemServices;
use crate::domain::modules::Permission;
use crate::domain::snapshot::StateSnapshot;
use crate::domain::system::PowerAction;
use crate::domain::utility::{is_launchable, MAX_CLIPBOARD_CHARS};

/// Tras tantos fallos seguidos el núcleo desactiva el módulo.
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;
pub const MAX_NOTICE_CHARS: usize = 300;
/// Mensajes que un módulo puede enviar por canales en la ventana de tiempo (anti-inundación).
pub const MAX_CHANNEL_SENDS: usize = 20;
pub const CHANNEL_WINDOW: std::time::Duration = std::time::Duration::from_secs(10);
pub const MAX_DATA_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_DATA_FILES: usize = 100;

/// Eventos del núcleo a los que se puede suscribir un módulo, y el permiso que exigen.
pub const EVENTS: &[(&str, Permission)] =
    &[("workspace_changed", Permission::Windows), ("focus_changed", Permission::Windows)];

/// Qué necesita el núcleo del almacén de módulos (lo implementa la infraestructura).
pub trait ModuleHost: Send + Sync {
    /// Permisos concedidos si el módulo existe y está activo.
    fn granted(&self, id: &str) -> Option<BTreeSet<Permission>>;
    fn display_name(&self, id: &str) -> Option<String>;
    fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String>;
    /// Carpeta de datos propia del módulo.
    fn data_dir(&self, id: &str) -> PathBuf;
    /// Ruta dentro de esa carpeta; error si intenta salirse.
    fn resolve_data_path(&self, id: &str, rel: &str) -> Result<PathBuf, String>;
}

/// Último estado del escritorio, para las consultas `windows.*`.
pub trait StateSource: Send + Sync {
    fn snapshot(&self) -> Option<StateSnapshot>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    PermissionDenied(String),
    UnknownMethod(String),
    BadParams(String),
    Failed(String),
    /// El módulo está desactivado o no existe.
    Disabled(String),
}

impl ApiError {
    pub fn code(&self) -> &'static str {
        match self {
            ApiError::PermissionDenied(_) => "permission_denied",
            ApiError::UnknownMethod(_) => "unknown_method",
            ApiError::BadParams(_) => "bad_params",
            ApiError::Failed(_) => "failed",
            ApiError::Disabled(_) => "disabled",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            ApiError::PermissionDenied(m)
            | ApiError::UnknownMethod(m)
            | ApiError::BadParams(m)
            | ApiError::Failed(m)
            | ApiError::Disabled(m) => m,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

/// Un evento que debe llegar a un módulo concreto.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleEvent {
    pub module: String,
    pub event: String,
    pub payload: Value,
}

/// Permiso que exige cada método; `None` si es libre; `Err` si el método no existe.
pub fn required_permission(method: &str) -> Result<Option<Permission>, ApiError> {
    Ok(match method {
        "core.version" | "events.unsubscribe" => None,
        "events.subscribe" => None, // se comprueba según el evento pedido
        "windows.active_workspace" | "windows.workspaces" | "windows.focused" => Some(Permission::Windows),
        "system.stats" => Some(Permission::System),
        "power.run" => Some(Permission::Power),
        "notify.show" => Some(Permission::Notifications),
        "files.read" | "files.write" | "files.list" | "files.delete" | "files.dir" => Some(Permission::Files),
        "apps.list_open" | "apps.focus" => Some(Permission::Windows),
        "apps.launch" => Some(Permission::Launch),
        "clipboard.read" | "clipboard.write" => Some(Permission::Clipboard),
        "speedtest.start" | "speedtest.status" => Some(Permission::Speedtest),
        "channels.info" | "channels.subscribe" | "channels.unsubscribe" | "channels.send" => Some(Permission::Channels),
        other => return Err(ApiError::UnknownMethod(format!("no existe el método '{other}'"))),
    })
}

pub struct ModuleApi {
    host: Arc<dyn ModuleHost>,
    state: Arc<dyn StateSource>,
    services: Arc<SystemServices>,
    /// (nombre del módulo, texto) -> mostrar un aviso al usuario.
    notify: Arc<dyn Fn(&str, &str) + Send + Sync>,
    core_version: String,
    channels: Arc<dyn ChannelBackend>,
    /// Entrega eventos asíncronos a los módulos (mensajes de canales).
    event_sink: Arc<dyn Fn(ModuleEvent) + Send + Sync>,
    channel_subs: Mutex<HashMap<String, BTreeSet<String>>>,
    channel_sends: Mutex<HashMap<String, std::collections::VecDeque<std::time::Instant>>>,
    subscriptions: Mutex<HashMap<String, BTreeSet<String>>>,
    failures: Mutex<HashMap<String, u32>>,
}

fn str_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, ApiError> {
    params.get(key).and_then(Value::as_str).ok_or_else(|| ApiError::BadParams(format!("falta '{key}' (texto)")))
}

impl ModuleApi {
    pub fn new(
        host: Arc<dyn ModuleHost>,
        state: Arc<dyn StateSource>,
        services: Arc<SystemServices>,
        notify: Arc<dyn Fn(&str, &str) + Send + Sync>,
        channels: Arc<dyn ChannelBackend>,
        event_sink: Arc<dyn Fn(ModuleEvent) + Send + Sync>,
        core_version: &str,
    ) -> ModuleApi {
        ModuleApi {
            host,
            state,
            services,
            notify,
            core_version: core_version.to_string(),
            channels,
            event_sink,
            channel_subs: Mutex::new(HashMap::new()),
            channel_sends: Mutex::new(HashMap::new()),
            subscriptions: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
        }
    }

    /// Punto único de entrada de los módulos. Comprueba, ejecuta y aísla fallos.
    pub fn call(&self, module: &str, method: &str, params: Value) -> Result<Value, ApiError> {
        let granted = self
            .host
            .granted(module)
            .ok_or_else(|| ApiError::Disabled(format!("el módulo '{module}' no está activo")))?;
        let required = required_permission(method)?;
        if let Some(p) = required {
            if !granted.contains(&p) {
                return Err(ApiError::PermissionDenied(format!("'{method}' necesita el permiso '{p}', que no se concedió")));
            }
        }
        let result = catch_unwind(AssertUnwindSafe(|| self.dispatch(module, method, &params, &granted)));
        match result {
            Ok(Ok(v)) => {
                self.failures.lock().unwrap().remove(module);
                Ok(v)
            }
            // Un error normal (parámetros mal, permiso denegado en un evento...) no cuenta como fallo del módulo.
            Ok(Err(e @ (ApiError::BadParams(_) | ApiError::PermissionDenied(_) | ApiError::UnknownMethod(_)))) => Err(e),
            Ok(Err(e)) => {
                self.register_failure(module);
                Err(e)
            }
            Err(_) => {
                self.register_failure(module);
                Err(ApiError::Failed(format!("'{method}' falló internamente; el núcleo sigue funcionando")))
            }
        }
    }

    fn register_failure(&self, module: &str) {
        let count = {
            let mut f = self.failures.lock().unwrap();
            let c = f.entry(module.to_string()).or_insert(0);
            *c += 1;
            *c
        };
        if count >= MAX_CONSECUTIVE_FAILURES {
            let _ = self.host.set_enabled(module, false);
            self.unsubscribe_all(module);
            self.failures.lock().unwrap().remove(module);
        }
    }

    fn dispatch(&self, module: &str, method: &str, params: &Value, granted: &BTreeSet<Permission>) -> Result<Value, ApiError> {
        match method {
            "core.version" => Ok(json!({ "version": self.core_version })),
            "windows.active_workspace" => {
                let s = self.state.snapshot().ok_or_else(|| ApiError::Failed("el gestor de ventanas no está activo".into()))?;
                let active = s.workspaces.iter().find(|w| w.active).map(|w| w.number);
                Ok(json!({ "number": active }))
            }
            "windows.workspaces" => {
                let s = self.state.snapshot().ok_or_else(|| ApiError::Failed("el gestor de ventanas no está activo".into()))?;
                Ok(json!(s.workspaces.iter().map(|w| json!({
                    "number": w.number, "visible": w.visible, "active": w.active, "window_count": w.window_count
                })).collect::<Vec<_>>()))
            }
            "windows.focused" => {
                let s = self.state.snapshot().ok_or_else(|| ApiError::Failed("el gestor de ventanas no está activo".into()))?;
                Ok(match s.focused {
                    Some(f) => json!({ "exe": f.exe, "title": f.title }),
                    None => Value::Null,
                })
            }
            "system.stats" => {
                let o = self.services.overview.lock().map_err(|e| ApiError::Failed(e.to_string()))?.read();
                Ok(json!({ "cpu_percent": o.cpu_percent, "ram_used": o.ram_used, "ram_total": o.ram_total }))
            }
            "power.run" => {
                let action = PowerAction::from_name(str_param(params, "action")?)
                    .ok_or_else(|| ApiError::BadParams("acción de energía desconocida".into()))?;
                let out = self.services.power.lock().map_err(|e| ApiError::Failed(e.to_string()))?.run(action).map_err(ApiError::Failed)?;
                Ok(json!({ "simulated": out.simulated, "description": out.description }))
            }
            "notify.show" => {
                let text = str_param(params, "text")?;
                let text: String = text.chars().take(MAX_NOTICE_CHARS).collect();
                let name = self.host.display_name(module).unwrap_or_else(|| module.to_string());
                (self.notify)(&name, &text);
                Ok(Value::Null)
            }
            "files.write" => self.files_write(module, params),
            "files.dir" => {
                let dir = self.host.data_dir(module);
                let _ = std::fs::create_dir_all(&dir);
                Ok(json!({ "path": dir.to_string_lossy() }))
            }
            "apps.list_open" => {
                let apps = self.services.apps.lock().map_err(|e| ApiError::Failed(e.to_string()))?.open_apps().map_err(ApiError::Failed)?;
                Ok(json!(apps
                    .iter()
                    .map(|a| json!({ "handle": a.handle, "exe": a.exe, "path": a.path, "title": a.title }))
                    .collect::<Vec<_>>()))
            }
            "apps.focus" => {
                let handle = params.get("handle").and_then(Value::as_i64).ok_or_else(|| ApiError::BadParams("falta 'handle' (número)".into()))?;
                self.services.apps.lock().map_err(|e| ApiError::Failed(e.to_string()))?.focus(handle as isize).map_err(ApiError::Failed)?;
                Ok(Value::Bool(true))
            }
            "apps.launch" => {
                let target = str_param(params, "path")?;
                // Solo programas y accesos directos con ruta absoluta; nunca comandos ni parámetros.
                is_launchable(target).map_err(ApiError::BadParams)?;
                self.services.apps.lock().map_err(|e| ApiError::Failed(e.to_string()))?.launch(target.trim()).map_err(ApiError::Failed)?;
                Ok(Value::Bool(true))
            }
            "clipboard.read" => {
                let text = self.services.clipboard.lock().map_err(|e| ApiError::Failed(e.to_string()))?.read_text().map_err(ApiError::Failed)?;
                Ok(match text {
                    Some(t) => json!({ "text": t.chars().take(MAX_CLIPBOARD_CHARS).collect::<String>() }),
                    None => Value::Null,
                })
            }
            "clipboard.write" => {
                let text = str_param(params, "text")?;
                if text.chars().count() > MAX_CLIPBOARD_CHARS {
                    return Err(ApiError::BadParams(format!("el texto supera {MAX_CLIPBOARD_CHARS} caracteres")));
                }
                self.services.clipboard.lock().map_err(|e| ApiError::Failed(e.to_string()))?.write_text(text).map_err(ApiError::Failed)?;
                Ok(Value::Bool(true))
            }
            "speedtest.start" => {
                // Pedirla dos veces a la vez no es un fallo del módulo.
                self.services.speedtest.lock().map_err(|e| ApiError::Failed(e.to_string()))?.start().map_err(ApiError::BadParams)?;
                Ok(Value::Bool(true))
            }
            "channels.info" => Ok(json!({ "backend": self.channels.kind(), "latency_ms": self.channels.latency_ms() })),
            "channels.subscribe" => {
                let channel = self.channel_param(params)?;
                let host = self.host.clone();
                let sink = self.event_sink.clone();
                let who = module.to_string();
                self.channels
                    .subscribe(
                        module,
                        &channel,
                        Arc::new(move |m: ChannelMessage| {
                            // El permiso se vuelve a comprobar al entregar: si se revocó, el mensaje no llega.
                            if host.granted(&who).is_some_and(|g| g.contains(&Permission::Channels)) {
                                sink(ModuleEvent {
                                    module: who.clone(),
                                    event: "channel_message".into(),
                                    payload: json!({
                                        "channel": m.channel, "from": m.from_module, "text": m.text, "sent_at_ms": m.sent_at_ms
                                    }),
                                });
                            }
                        }),
                    )
                    .map_err(ApiError::Failed)?;
                self.channel_subs.lock().unwrap().entry(module.to_string()).or_default().insert(channel);
                Ok(Value::Bool(true))
            }
            "channels.unsubscribe" => {
                let channel = self.channel_param(params)?;
                self.channels.unsubscribe(module, &channel);
                if let Some(set) = self.channel_subs.lock().unwrap().get_mut(module) {
                    set.remove(&channel);
                }
                Ok(Value::Bool(true))
            }
            "channels.send" => {
                let channel = self.channel_param(params)?;
                let text = str_param(params, "text")?;
                if text.is_empty() || text.chars().count() > MAX_MESSAGE_CHARS {
                    return Err(ApiError::BadParams(format!("el mensaje debe tener entre 1 y {MAX_MESSAGE_CHARS} caracteres")));
                }
                {
                    let mut log = self.channel_sends.lock().unwrap();
                    let q = log.entry(module.to_string()).or_default();
                    let now = std::time::Instant::now();
                    while q.front().is_some_and(|t| now.duration_since(*t) > CHANNEL_WINDOW) {
                        q.pop_front();
                    }
                    if q.len() >= MAX_CHANNEL_SENDS {
                        return Err(ApiError::BadParams("demasiados mensajes seguidos; espera unos segundos".into()));
                    }
                    q.push_back(now);
                }
                let sent_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                self.channels
                    .send(ChannelMessage { channel, from_module: module.to_string(), text: text.to_string(), sent_at_ms })
                    .map_err(ApiError::Failed)?;
                Ok(Value::Bool(true))
            }
            "speedtest.status" => {
                let s = self.services.speedtest.lock().map_err(|e| ApiError::Failed(e.to_string()))?.status();
                serde_json::to_value(s).map_err(|e| ApiError::Failed(e.to_string()))
            }
            "files.read" => {
                let path = self.host.resolve_data_path(module, str_param(params, "path")?).map_err(ApiError::BadParams)?;
                match std::fs::read_to_string(&path) {
                    Ok(content) => Ok(json!({ "content": content })),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
                    Err(e) => Err(ApiError::Failed(e.to_string())),
                }
            }
            "files.delete" => {
                let path = self.host.resolve_data_path(module, str_param(params, "path")?).map_err(ApiError::BadParams)?;
                match std::fs::remove_file(path) {
                    Ok(()) => Ok(Value::Bool(true)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Bool(false)),
                    Err(e) => Err(ApiError::Failed(e.to_string())),
                }
            }
            "files.list" => Ok(json!(self.list_data_files(module))),
            "events.subscribe" => {
                let event = str_param(params, "event")?;
                let needed = EVENTS
                    .iter()
                    .find(|(name, _)| *name == event)
                    .ok_or_else(|| ApiError::BadParams(format!("no existe el evento '{event}'")))?
                    .1;
                if !granted.contains(&needed) {
                    return Err(ApiError::PermissionDenied(format!("el evento '{event}' necesita el permiso '{needed}'")));
                }
                self.subscriptions.lock().unwrap().entry(module.to_string()).or_default().insert(event.to_string());
                Ok(Value::Bool(true))
            }
            "events.unsubscribe" => {
                let event = str_param(params, "event")?;
                if let Some(set) = self.subscriptions.lock().unwrap().get_mut(module) {
                    set.remove(event);
                }
                Ok(Value::Bool(true))
            }
            other => Err(ApiError::UnknownMethod(format!("no existe el método '{other}'"))),
        }
    }

    fn list_data_files(&self, module: &str) -> Vec<String> {
        fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(base, &p, out);
                    } else if let Ok(rel) = p.strip_prefix(base) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
        let base = self.host.data_dir(module);
        let mut out = Vec::new();
        walk(&base, &base, &mut out);
        out.sort();
        out
    }

    fn files_write(&self, module: &str, params: &Value) -> Result<Value, ApiError> {
        let rel = str_param(params, "path")?;
        let content = str_param(params, "content")?;
        if content.len() > MAX_DATA_FILE_BYTES {
            return Err(ApiError::BadParams(format!("el archivo supera {MAX_DATA_FILE_BYTES} bytes")));
        }
        let path = self.host.resolve_data_path(module, rel).map_err(ApiError::BadParams)?;
        let existing = self.list_data_files(module);
        if !path.exists() && existing.len() >= MAX_DATA_FILES {
            return Err(ApiError::BadParams(format!("demasiados archivos (máximo {MAX_DATA_FILES})")));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| ApiError::Failed(e.to_string()))?;
        }
        std::fs::write(path, content).map_err(|e| ApiError::Failed(e.to_string()))?;
        Ok(Value::Bool(true))
    }

    // ---------- eventos ----------

    /// Compara el estado anterior con el nuevo y devuelve los eventos que le tocan a cada módulo suscrito
    /// (y que siga teniendo el permiso).
    pub fn events_for_state_change(&self, before: Option<&StateSnapshot>, now: &StateSnapshot) -> Vec<ModuleEvent> {
        let active = |s: &StateSnapshot| s.workspaces.iter().find(|w| w.active).map(|w| w.number);
        let mut produced: Vec<(&str, Value)> = Vec::new();
        if before.map(active) != Some(active(now)) {
            produced.push(("workspace_changed", json!({ "number": active(now) })));
        }
        if before.map(|b| &b.focused) != Some(&now.focused) {
            let payload = match &now.focused {
                Some(f) => json!({ "exe": f.exe, "title": f.title }),
                None => Value::Null,
            };
            produced.push(("focus_changed", payload));
        }
        let subs = self.subscriptions.lock().unwrap().clone();
        let mut out = Vec::new();
        for (module, events) in subs {
            let Some(granted) = self.host.granted(&module) else { continue };
            for (event, payload) in &produced {
                let needed = EVENTS.iter().find(|(n, _)| n == event).map(|e| e.1);
                if events.contains(*event) && needed.is_some_and(|p| granted.contains(&p)) {
                    out.push(ModuleEvent { module: module.clone(), event: event.to_string(), payload: payload.clone() });
                }
            }
        }
        out
    }

    /// Quita todas las suscripciones de un módulo, de eventos y de canales (al desinstalarlo o desactivarlo).
    pub fn unsubscribe_all(&self, module: &str) {
        self.subscriptions.lock().unwrap().remove(module);
        self.channels.unsubscribe_module(module);
        self.channel_subs.lock().unwrap().remove(module);
        self.channel_sends.lock().unwrap().remove(module);
    }

    pub fn channels_of(&self, module: &str) -> BTreeSet<String> {
        self.channel_subs.lock().unwrap().get(module).cloned().unwrap_or_default()
    }

    fn channel_param(&self, params: &Value) -> Result<String, ApiError> {
        let c = str_param(params, "channel")?;
        if !is_valid_channel(c) {
            return Err(ApiError::BadParams("nombre de canal no válido (minúsculas, números, - y _; hasta 64)".into()));
        }
        Ok(c.to_string())
    }

    pub fn subscriptions_of(&self, module: &str) -> BTreeSet<String> {
        self.subscriptions.lock().unwrap().get(module).cloned().unwrap_or_default()
    }

    /// Nº de módulos con alguna suscripción viva.
    pub fn subscribed_modules(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.subscriptions.lock().unwrap().iter().filter(|(_, s)| !s.is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

#[cfg(test)]
pub mod fakes {
    use super::*;
    use crate::application::system::fakes::{Calls, FakePower};
    use crate::application::system::{Guarded, OverviewProvider};
    use crate::domain::snapshot::{FocusedSnapshot, WorkspaceSnapshot};
    use crate::domain::system::SystemOverview;

    pub struct FakeHost {
        pub dir: PathBuf,
        pub grants: Mutex<BTreeMap<String, (BTreeSet<Permission>, bool)>>,
    }

    impl FakeHost {
        pub fn new(dir: PathBuf) -> Arc<FakeHost> {
            Arc::new(FakeHost { dir, grants: Mutex::new(BTreeMap::new()) })
        }
        pub fn grant(&self, id: &str, perms: &[Permission]) {
            self.grants.lock().unwrap().insert(id.to_string(), (perms.iter().copied().collect(), true));
        }
        pub fn is_enabled(&self, id: &str) -> bool {
            self.grants.lock().unwrap().get(id).is_some_and(|g| g.1)
        }
    }

    impl ModuleHost for FakeHost {
        fn granted(&self, id: &str) -> Option<BTreeSet<Permission>> {
            self.grants.lock().unwrap().get(id).filter(|g| g.1).map(|g| g.0.clone())
        }
        fn display_name(&self, id: &str) -> Option<String> {
            Some(format!("Módulo {id}"))
        }
        fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
            self.grants.lock().unwrap().get_mut(id).ok_or("no existe")?.1 = enabled;
            Ok(())
        }
        fn data_dir(&self, id: &str) -> PathBuf {
            self.dir.join(id)
        }
        fn resolve_data_path(&self, id: &str, rel: &str) -> Result<PathBuf, String> {
            if !crate::domain::modules::is_safe_relative_path(rel) {
                return Err(format!("ruta no permitida: {rel}"));
            }
            Ok(self.dir.join(id).join(rel))
        }
    }

    pub struct FakeState(pub Mutex<Option<StateSnapshot>>);
    impl StateSource for FakeState {
        fn snapshot(&self) -> Option<StateSnapshot> {
            self.0.lock().unwrap().clone()
        }
    }

    pub fn snapshot(active: usize, focused: Option<&str>) -> StateSnapshot {
        StateSnapshot {
            monitors: vec![],
            workspaces: (1..=3)
                .map(|n| WorkspaceSnapshot { number: n, monitor: 1, visible: n == active, active: n == active, window_count: n })
                .collect(),
            focused: focused.map(|t| FocusedSnapshot { exe: "app.exe".into(), title: t.into() }),
            paused: false,
            mode: None,
            windows: vec![],
        }
    }

    pub struct PanickingOverview;
    impl OverviewProvider for PanickingOverview {
        fn read(&mut self) -> SystemOverview {
            panic!("el adaptador de estadísticas explotó");
        }
    }

    pub struct OkOverview;
    impl OverviewProvider for OkOverview {
        fn read(&mut self) -> SystemOverview {
            SystemOverview {
                hardware: Default::default(),
                cpu_percent: 12.0,
                gpu_percent: None,
                ram_used: 1,
                ram_total: 2,
                disks: vec![],
                net_down: 0,
                net_up: 0,
                uptime_secs: 1,
                temperature_c: None,
                top_processes: vec![],
            }
        }
    }

    pub struct FakeClipboard(pub Calls);
    impl crate::application::system::ClipboardControl for FakeClipboard {
        fn read_text(&mut self) -> Result<Option<String>, String> {
            self.0.log("clip:read");
            Ok(Some("texto copiado".into()))
        }
        fn write_text(&mut self, t: &str) -> Result<(), String> {
            self.0.log(format!("clip:write:{t}"));
            Ok(())
        }
    }

    pub struct FakeApps(pub Calls);
    impl crate::application::system::AppsControl for FakeApps {
        fn open_apps(&mut self) -> Result<Vec<crate::domain::utility::OpenApp>, String> {
            Ok(vec![crate::domain::utility::OpenApp {
                handle: 42,
                exe: "code.exe".into(),
                path: "C:\\Apps\\code.exe".into(),
                title: "Editor".into(),
            }])
        }
        fn focus(&mut self, h: isize) -> Result<(), String> {
            self.0.log(format!("focus:{h}"));
            Ok(())
        }
        fn launch(&mut self, t: &str) -> Result<(), String> {
            self.0.log(format!("launch:{t}"));
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct FakeSpeed(Option<bool>);
    impl crate::application::system::SpeedTestControl for FakeSpeed {
        fn start(&mut self) -> Result<(), String> {
            if self.0 == Some(true) {
                return Err("ya hay una prueba en curso".into());
            }
            self.0 = Some(true);
            Ok(())
        }
        fn status(&mut self) -> crate::domain::utility::SpeedStatus {
            match self.0 {
                None => crate::domain::utility::SpeedStatus::Idle,
                Some(_) => crate::domain::utility::SpeedStatus::Running,
            }
        }
    }

    pub struct Rig {
        pub api: ModuleApi,
        pub host: Arc<FakeHost>,
        pub state: Arc<FakeState>,
        pub power_calls: Calls,
        pub clip_calls: Calls,
        pub app_calls: Calls,
        pub notices: Arc<Mutex<Vec<(String, String)>>>,
        pub events: Arc<Mutex<Vec<ModuleEvent>>>,
        pub channels: Arc<crate::application::channels::LocalChannelBackend>,
    }

    pub fn rig(name: &str, overview: Box<dyn OverviewProvider>) -> Rig {
        use crate::application::system::{MediaControl, RadioControl, VolumeControl, WallpaperControl};
        use crate::domain::system::{ActionOutcome, MediaCommand, MediaInfo, RadioKind, RadioState, VolumeState};
        struct NoMedia;
        impl MediaControl for NoMedia {
            fn now_playing(&mut self) -> Result<Option<MediaInfo>, String> {
                Ok(None)
            }
            fn send(&mut self, _: MediaCommand) -> Result<ActionOutcome, String> {
                Err("no".into())
            }
        }
        struct NoRadios;
        impl RadioControl for NoRadios {
            fn states(&mut self) -> Vec<RadioState> {
                vec![]
            }
            fn set(&mut self, _: RadioKind, _: bool) -> Result<ActionOutcome, String> {
                Err("no".into())
            }
        }
        struct NoWall;
        impl WallpaperControl for NoWall {
            fn current(&mut self) -> Result<Option<String>, String> {
                Ok(None)
            }
            fn set(&mut self, _: &str) -> Result<ActionOutcome, String> {
                Err("no".into())
            }
        }
        struct NoVol;
        impl VolumeControl for NoVol {
            fn get(&mut self) -> Result<VolumeState, String> {
                Err("no".into())
            }
            fn set_percent(&mut self, _: f32) -> Result<ActionOutcome, String> {
                Err("no".into())
            }
            fn set_muted(&mut self, _: bool) -> Result<ActionOutcome, String> {
                Err("no".into())
            }
        }
        let dir = std::env::temp_dir().join(format!("wm-api-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let power_calls = Calls::default();
        let clip_calls = Calls::default();
        let app_calls = Calls::default();
        let services = Arc::new(SystemServices {
            media: Mutex::new(Box::new(NoMedia)),
            // El adaptador real de energía está detrás del modo seguro, como en producción.
            power: Mutex::new(Box::new(Guarded::new(FakePower(power_calls.clone()), false))),
            radios: Mutex::new(Box::new(NoRadios)),
            network: Mutex::new(Box::new(crate::application::system::NoNetwork)),
            wallpaper: Mutex::new(Box::new(NoWall)),
            engine: Mutex::new(Box::new(crate::infrastructure::windows::system::wallpaper_engine::NoEngine)),
            volume: Mutex::new(Box::new(NoVol)),
            overview: Mutex::new(overview),
            clipboard: Mutex::new(Box::new(FakeClipboard(clip_calls.clone()))),
            apps: Mutex::new(Box::new(FakeApps(app_calls.clone()))),
            speedtest: Mutex::new(Box::new(FakeSpeed::default())),
        });
        let host = FakeHost::new(dir);
        let state = Arc::new(FakeState(Mutex::new(Some(snapshot(1, Some("Editor"))))));
        let notices: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
        let n2 = notices.clone();
        let events: Arc<Mutex<Vec<ModuleEvent>>> = Arc::default();
        let e2 = events.clone();
        let channels = crate::application::channels::LocalChannelBackend::new(std::time::Duration::from_millis(20));
        let api = ModuleApi::new(
            host.clone(),
            state.clone(),
            services,
            Arc::new(move |m, t| n2.lock().unwrap().push((m.to_string(), t.to_string()))),
            Arc::new(channels.clone()),
            Arc::new(move |e| e2.lock().unwrap().push(e)),
            "0.1.0",
        );
        Rig { api, host, state, power_calls, clip_calls, app_calls, notices, events, channels }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::*;
    use super::*;

    fn rig_ok(name: &str) -> Rig {
        rig(name, Box::new(OkOverview))
    }

    #[test]
    fn un_modulo_sin_el_permiso_no_accede_a_esa_funcion() {
        let r = rig_ok("denied");
        r.host.grant("m", &[Permission::Notifications]); // sin windows, power, files, system
        for (method, params) in [
            ("windows.active_workspace", json!(null)),
            ("windows.workspaces", json!(null)),
            ("windows.focused", json!(null)),
            ("system.stats", json!(null)),
            ("power.run", json!({"action": "shutdown"})),
            ("files.write", json!({"path": "a.txt", "content": "x"})),
            ("files.read", json!({"path": "a.txt"})),
            ("files.list", json!(null)),
            ("files.delete", json!({"path": "a.txt"})),
        ] {
            let e = r.api.call("m", method, params).unwrap_err();
            assert_eq!(e.code(), "permission_denied", "{method}: {e}");
            assert!(e.message().contains("no se concedió"), "{e}");
        }
        assert!(r.power_calls.all().is_empty(), "la energía nunca se tocó");
        assert!(!r.api.host.granted("m").unwrap().contains(&Permission::Files));
        // Y lo concedido sí funciona.
        r.api.call("m", "notify.show", json!({"text": "hola"})).unwrap();
        assert_eq!(r.notices.lock().unwrap()[0], ("Módulo m".to_string(), "hola".to_string()));
    }

    #[test]
    fn los_eventos_tambien_exigen_permiso() {
        let r = rig_ok("events-denied");
        r.host.grant("m", &[Permission::Files]);
        let e = r.api.call("m", "events.subscribe", json!({"event": "workspace_changed"})).unwrap_err();
        assert_eq!(e.code(), "permission_denied");
        assert!(r.api.subscriptions_of("m").is_empty());
        let e = r.api.call("m", "events.subscribe", json!({"event": "no_existe"})).unwrap_err();
        assert_eq!(e.code(), "bad_params");
    }

    #[test]
    fn metodos_desconocidos_y_modulos_inactivos() {
        let r = rig_ok("unknown");
        r.host.grant("m", &Permission::ALL);
        assert_eq!(r.api.call("m", "sistema.formatear", json!(null)).unwrap_err().code(), "unknown_method");
        assert_eq!(r.api.call("fantasma", "core.version", json!(null)).unwrap_err().code(), "disabled");
        r.host.set_enabled("m", false).unwrap();
        assert_eq!(r.api.call("m", "core.version", json!(null)).unwrap_err().code(), "disabled");
    }

    #[test]
    fn consultas_de_ventanas_con_permiso() {
        let r = rig_ok("windows");
        r.host.grant("m", &[Permission::Windows]);
        assert_eq!(r.api.call("m", "core.version", json!(null)).unwrap(), json!({"version": "0.1.0"}));
        assert_eq!(r.api.call("m", "windows.active_workspace", json!(null)).unwrap(), json!({"number": 1}));
        assert_eq!(r.api.call("m", "windows.focused", json!(null)).unwrap(), json!({"exe": "app.exe", "title": "Editor"}));
        let ws = r.api.call("m", "windows.workspaces", json!(null)).unwrap();
        assert_eq!(ws.as_array().unwrap().len(), 3);
        *r.state.0.lock().unwrap() = None;
        assert_eq!(r.api.call("m", "windows.active_workspace", json!(null)).unwrap_err().code(), "failed");
    }

    #[test]
    fn la_energia_sigue_sujeta_al_modo_seguro() {
        let r = rig_ok("power");
        r.host.grant("m", &[Permission::Power]);
        let out = r.api.call("m", "power.run", json!({"action": "shutdown"})).unwrap();
        assert_eq!(out["simulated"], json!(true));
        assert!(r.power_calls.all().is_empty(), "el permiso del módulo no salta el modo seguro del núcleo");
        assert_eq!(r.api.call("m", "power.run", json!({"action": "borrar-todo"})).unwrap_err().code(), "bad_params");
    }

    #[test]
    fn archivos_solo_en_su_carpeta() {
        let r = rig_ok("files");
        r.host.grant("m", &[Permission::Files]);
        r.host.grant("otro", &[Permission::Files]);
        r.api.call("m", "files.write", json!({"path": "notas/a.txt", "content": "hola"})).unwrap();
        assert_eq!(r.api.call("m", "files.read", json!({"path": "notas/a.txt"})).unwrap(), json!({"content": "hola"}));
        assert_eq!(r.api.call("m", "files.read", json!({"path": "no-existe.txt"})).unwrap(), Value::Null);
        assert_eq!(r.api.call("m", "files.list", json!(null)).unwrap(), json!(["notas/a.txt"]));
        // Otro módulo no ve los archivos de este.
        assert_eq!(r.api.call("otro", "files.list", json!(null)).unwrap(), json!([]));
        for bad in ["../otro/x.txt", "/etc/x", "C:\\x.txt", "a/../../b"] {
            let e = r.api.call("m", "files.write", json!({"path": bad, "content": "x"})).unwrap_err();
            assert_eq!(e.code(), "bad_params", "'{bad}'");
        }
        assert_eq!(r.api.call("m", "files.delete", json!({"path": "notas/a.txt"})).unwrap(), json!(true));
        assert_eq!(r.api.call("m", "files.delete", json!({"path": "notas/a.txt"})).unwrap(), json!(false));
    }

    #[test]
    fn limites_de_archivos() {
        let r = rig_ok("limits");
        r.host.grant("m", &[Permission::Files]);
        let big = "x".repeat(MAX_DATA_FILE_BYTES + 1);
        assert_eq!(r.api.call("m", "files.write", json!({"path": "g.txt", "content": big})).unwrap_err().code(), "bad_params");
        for i in 0..MAX_DATA_FILES {
            r.api.call("m", "files.write", json!({"path": format!("f{i}.txt"), "content": "x"})).unwrap();
        }
        assert_eq!(r.api.call("m", "files.write", json!({"path": "uno-mas.txt", "content": "x"})).unwrap_err().code(), "bad_params");
        // Reescribir uno existente sí se puede.
        r.api.call("m", "files.write", json!({"path": "f0.txt", "content": "y"})).unwrap();
    }

    #[test]
    fn avisos_acotados() {
        let r = rig_ok("notices");
        r.host.grant("m", &[Permission::Notifications]);
        r.api.call("m", "notify.show", json!({"text": "a".repeat(5000)})).unwrap();
        assert_eq!(r.notices.lock().unwrap()[0].1.chars().count(), MAX_NOTICE_CHARS);
        assert_eq!(r.api.call("m", "notify.show", json!({})).unwrap_err().code(), "bad_params");
    }

    #[test]
    fn eventos_solo_a_los_suscritos_con_permiso() {
        let r = rig_ok("events");
        r.host.grant("a", &[Permission::Windows]);
        r.host.grant("b", &[Permission::Windows]);
        r.host.grant("c", &[Permission::Files]);
        r.api.call("a", "events.subscribe", json!({"event": "workspace_changed"})).unwrap();
        r.api.call("a", "events.subscribe", json!({"event": "focus_changed"})).unwrap();
        r.api.call("b", "events.subscribe", json!({"event": "focus_changed"})).unwrap();
        let before = snapshot(1, Some("Editor"));
        let now = snapshot(2, Some("Editor"));
        let ev = r.api.events_for_state_change(Some(&before), &now);
        assert_eq!(ev, vec![ModuleEvent { module: "a".into(), event: "workspace_changed".into(), payload: json!({"number": 2}) }]);
        let now2 = snapshot(2, Some("Navegador"));
        let ev = r.api.events_for_state_change(Some(&now), &now2);
        assert_eq!(ev.len(), 2, "a y b reciben el cambio de foco");
        assert!(ev.iter().all(|e| e.event == "focus_changed" && e.payload["title"] == json!("Navegador")));
        assert!(r.api.events_for_state_change(Some(&now2), &now2).is_empty(), "sin cambios no hay eventos");
        // Si se le revoca el permiso, deja de recibirlos aunque siga suscrito.
        r.host.grant("a", &[Permission::Files]);
        assert!(r.api.events_for_state_change(Some(&now2), &snapshot(3, Some("X"))).iter().all(|e| e.module != "a"));
    }

    #[test]
    fn desinstalar_o_desactivar_no_deja_suscripciones() {
        let r = rig_ok("unsub");
        r.host.grant("a", &[Permission::Windows]);
        r.api.call("a", "events.subscribe", json!({"event": "workspace_changed"})).unwrap();
        assert_eq!(r.api.subscribed_modules().len(), 1);
        r.api.unsubscribe_all("a");
        assert!(r.api.subscribed_modules().is_empty());
        assert!(r.api.events_for_state_change(Some(&snapshot(1, None)), &snapshot(2, None)).is_empty());
        r.api.call("a", "events.subscribe", json!({"event": "workspace_changed"})).unwrap();
        r.api.call("a", "events.unsubscribe", json!({"event": "workspace_changed"})).unwrap();
        assert!(r.api.subscriptions_of("a").is_empty());
    }

    #[test]
    fn un_modulo_que_falla_no_tumba_el_nucleo_y_acaba_desactivado() {
        let r = rig("fails", Box::new(PanickingOverview));
        r.host.grant("roto", &[Permission::System, Permission::Windows]);
        r.host.grant("sano", &[Permission::Windows]);
        r.api.call("roto", "events.subscribe", json!({"event": "workspace_changed"})).unwrap();
        for i in 1..=MAX_CONSECUTIVE_FAILURES {
            let e = r.api.call("roto", "system.stats", json!(null)).unwrap_err();
            assert_eq!(e.code(), "failed", "intento {i}: {e}");
            // El núcleo sigue atendiendo a los demás en cada fallo.
            assert!(r.api.call("sano", "windows.active_workspace", json!(null)).is_ok());
        }
        assert!(!r.host.is_enabled("roto"), "tras {MAX_CONSECUTIVE_FAILURES} fallos seguidos se desactiva");
        assert_eq!(r.api.call("roto", "core.version", json!(null)).unwrap_err().code(), "disabled");
        assert!(r.api.subscriptions_of("roto").is_empty(), "y no deja suscripciones");
        assert!(r.host.is_enabled("sano"));
    }

    #[test]
    fn un_exito_reinicia_la_cuenta_de_fallos() {
        let r = rig("recover", Box::new(PanickingOverview));
        r.host.grant("m", &[Permission::System, Permission::Windows]);
        for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
            let _ = r.api.call("m", "system.stats", json!(null));
        }
        r.api.call("m", "windows.active_workspace", json!(null)).unwrap();
        for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
            let _ = r.api.call("m", "system.stats", json!(null));
        }
        assert!(r.host.is_enabled("m"));
    }

    #[test]
    fn los_errores_de_parametros_no_cuentan_como_fallo_del_modulo() {
        let r = rig_ok("params");
        r.host.grant("m", &[Permission::Files]);
        for _ in 0..(MAX_CONSECUTIVE_FAILURES + 2) {
            assert_eq!(r.api.call("m", "files.read", json!({})).unwrap_err().code(), "bad_params");
        }
        assert!(r.host.is_enabled("m"));
    }

    // ---------- Fase 8: utilidades ----------

    #[test]
    fn las_funciones_de_utilidad_exigen_su_permiso() {
        let r = rig_ok("util-denied");
        r.host.grant("m", &[Permission::Notifications]);
        for (method, params) in [
            ("files.dir", json!(null)),
            ("apps.list_open", json!(null)),
            ("apps.focus", json!({"handle": 1})),
            ("apps.launch", json!({"path": "C:\\Apps\\code.exe"})),
            ("clipboard.read", json!(null)),
            ("clipboard.write", json!({"text": "x"})),
            ("speedtest.start", json!(null)),
            ("speedtest.status", json!(null)),
        ] {
            assert_eq!(r.api.call("m", method, params).unwrap_err().code(), "permission_denied", "{method}");
        }
        assert!(r.clip_calls.all().is_empty() && r.app_calls.all().is_empty(), "ningún adaptador se tocó");
    }

    #[test]
    fn apps_abiertas_y_enfocar_usan_el_permiso_de_ventanas() {
        let r = rig_ok("apps");
        r.host.grant("m", &[Permission::Windows]);
        let list = r.api.call("m", "apps.list_open", json!(null)).unwrap();
        assert_eq!(list[0]["exe"], "code.exe");
        assert_eq!(list[0]["handle"], 42);
        r.api.call("m", "apps.focus", json!({"handle": 42})).unwrap();
        assert_eq!(r.app_calls.all(), vec!["focus:42"]);
        assert_eq!(r.api.call("m", "apps.focus", json!({})).unwrap_err().code(), "bad_params");
        // Con solo `windows` no se puede abrir nada.
        assert_eq!(r.api.call("m", "apps.launch", json!({"path": "C:\\Apps\\code.exe"})).unwrap_err().code(), "permission_denied");
    }

    #[test]
    fn abrir_apps_solo_programas_con_ruta_absoluta() {
        let r = rig_ok("launch");
        r.host.grant("m", &[Permission::Launch]);
        r.api.call("m", "apps.launch", json!({"path": "  C:\\Program Files\\Discord\\Discord.exe "})).unwrap();
        assert_eq!(r.app_calls.all(), vec!["launch:C:\\Program Files\\Discord\\Discord.exe"]);
        for bad in ["cmd.exe", "C:\\a\\x.bat", "C:\\a\\..\\b.exe", "C:\\a\\x.exe /c calc", "powershell -c calc", ""] {
            assert_eq!(r.api.call("m", "apps.launch", json!({"path": bad})).unwrap_err().code(), "bad_params", "'{bad}'");
        }
        assert_eq!(r.app_calls.all().len(), 1, "lo rechazado nunca llegó al sistema");
    }

    #[test]
    fn portapapeles_leer_y_escribir() {
        let r = rig_ok("clip");
        r.host.grant("m", &[Permission::Clipboard]);
        assert_eq!(r.api.call("m", "clipboard.read", json!(null)).unwrap(), json!({"text": "texto copiado"}));
        r.api.call("m", "clipboard.write", json!({"text": "hola"})).unwrap();
        assert_eq!(r.clip_calls.all(), vec!["clip:read", "clip:write:hola"]);
        let big = "x".repeat(MAX_CLIPBOARD_CHARS + 1);
        assert_eq!(r.api.call("m", "clipboard.write", json!({"text": big})).unwrap_err().code(), "bad_params");
    }

    #[test]
    fn prueba_de_velocidad_se_inicia_y_se_consulta() {
        let r = rig_ok("speed");
        r.host.grant("m", &[Permission::Speedtest]);
        assert_eq!(r.api.call("m", "speedtest.status", json!(null)).unwrap(), json!({"state": "idle"}));
        r.api.call("m", "speedtest.start", json!(null)).unwrap();
        assert_eq!(r.api.call("m", "speedtest.status", json!(null)).unwrap(), json!({"state": "running"}));
        // Pedirla de nuevo mientras corre no es un fallo del módulo.
        for _ in 0..(MAX_CONSECUTIVE_FAILURES + 1) {
            assert_eq!(r.api.call("m", "speedtest.start", json!(null)).unwrap_err().code(), "bad_params");
        }
        assert!(r.host.is_enabled("m"));
    }

    #[test]
    fn la_carpeta_de_datos_del_modulo_se_puede_consultar() {
        let r = rig_ok("dir");
        r.host.grant("m", &[Permission::Files]);
        let p = r.api.call("m", "files.dir", json!(null)).unwrap();
        assert!(p["path"].as_str().unwrap().ends_with('m'), "{p}");
        assert!(std::path::Path::new(p["path"].as_str().unwrap()).is_dir());
    }

    // ---------- Fase 9: canales ----------

    fn wait_events(r: &Rig, n: usize) {
        for _ in 0..200 {
            if r.events.lock().unwrap().len() >= n {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("llegaron {} de {n} eventos", r.events.lock().unwrap().len());
    }

    #[test]
    fn los_canales_exigen_su_permiso() {
        let r = rig_ok("chan-denied");
        r.host.grant("m", &[Permission::Files]);
        for (method, params) in [
            ("channels.info", json!(null)),
            ("channels.subscribe", json!({"channel": "pareja"})),
            ("channels.unsubscribe", json!({"channel": "pareja"})),
            ("channels.send", json!({"channel": "pareja", "text": "hola"})),
        ] {
            assert_eq!(r.api.call("m", method, params).unwrap_err().code(), "permission_denied", "{method}");
        }
        assert_eq!(r.channels.subscriber_count(), 0);
    }

    #[test]
    fn dos_modulos_se_envian_mensajes_con_retardo_razonable() {
        let r = rig_ok("chan-pair");
        r.host.grant("a", &[Permission::Channels]);
        r.host.grant("b", &[Permission::Channels]);
        assert_eq!(r.api.call("a", "channels.info", json!(null)).unwrap(), json!({"backend": "local", "latency_ms": 20}));
        r.api.call("a", "channels.subscribe", json!({"channel": "pareja"})).unwrap();
        r.api.call("b", "channels.subscribe", json!({"channel": "pareja"})).unwrap();
        let t0 = std::time::Instant::now();
        r.api.call("a", "channels.send", json!({"channel": "pareja", "text": "hola amor"})).unwrap();
        wait_events(&r, 2);
        assert!(t0.elapsed() >= std::time::Duration::from_millis(20) && t0.elapsed() < std::time::Duration::from_millis(500));
        let ev = r.events.lock().unwrap().clone();
        let to_b = ev.iter().find(|e| e.module == "b").unwrap();
        assert_eq!(to_b.event, "channel_message");
        assert_eq!(to_b.payload["text"], "hola amor");
        assert_eq!(to_b.payload["from"], "a");
        assert_eq!(to_b.payload["channel"], "pareja");
        assert!(ev.iter().any(|e| e.module == "a"), "quien envía también lo recibe (filtra por su identidad)");
    }

    #[test]
    fn validacion_y_limite_de_mensajes() {
        let r = rig_ok("chan-limits");
        r.host.grant("m", &[Permission::Channels]);
        for bad in ["Mal Canal", "", "a/b", &"x".repeat(65)] {
            assert_eq!(r.api.call("m", "channels.subscribe", json!({"channel": bad})).unwrap_err().code(), "bad_params", "'{bad}'");
        }
        assert_eq!(r.api.call("m", "channels.send", json!({"channel": "c", "text": ""})).unwrap_err().code(), "bad_params");
        let long = "x".repeat(crate::application::channels::MAX_MESSAGE_CHARS + 1);
        assert_eq!(r.api.call("m", "channels.send", json!({"channel": "c", "text": long})).unwrap_err().code(), "bad_params");
        for _ in 0..MAX_CHANNEL_SENDS {
            r.api.call("m", "channels.send", json!({"channel": "c", "text": "hola"})).unwrap();
        }
        let e = r.api.call("m", "channels.send", json!({"channel": "c", "text": "otro"})).unwrap_err();
        assert_eq!(e.code(), "bad_params");
        assert!(e.message().contains("demasiados"), "{e}");
        assert!(r.host.is_enabled("m"), "inundar no desactiva el módulo, solo se frena");
    }

    #[test]
    fn revocar_el_permiso_o_desinstalar_corta_la_entrega() {
        let r = rig_ok("chan-revoke");
        r.host.grant("a", &[Permission::Channels]);
        r.host.grant("b", &[Permission::Channels]);
        r.api.call("a", "channels.subscribe", json!({"channel": "c"})).unwrap();
        r.api.call("b", "channels.subscribe", json!({"channel": "c"})).unwrap();
        assert_eq!(r.api.channels_of("a").len(), 1);
        // a pierde el permiso: ya no recibe aunque siga suscrito.
        r.host.grant("a", &[Permission::Files]);
        r.api.call("b", "channels.send", json!({"channel": "c", "text": "x"})).unwrap();
        wait_events(&r, 1);
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(r.events.lock().unwrap().iter().all(|e| e.module == "b"));
        // Desinstalar: no quedan suscripciones.
        r.api.unsubscribe_all("a");
        r.api.unsubscribe_all("b");
        assert_eq!(r.channels.subscriber_count(), 0);
        assert!(r.api.channels_of("a").is_empty() && r.api.channels_of("b").is_empty());
    }
}
