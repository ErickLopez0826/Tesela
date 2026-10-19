//! Controlador: traduce eventos, atajos y comandos en cambios del `Desktop` y los aplica a la plataforma.

use crate::application::ports::{WindowPlatform, DENIED};
use crate::domain::actions::Action;
use crate::domain::config::ValidConfig;
use crate::domain::desktop::{Desktop, Settings, TrackOptions};
use crate::domain::effects::{Effect, WindowId};
use crate::domain::manageable::{is_manageable, Scope};
use crate::domain::rules::{apply_rules, normalize_exe, ExclusionList, WindowRule};
use std::time::{Duration, Instant};
use crate::domain::shortcuts::Keymap;
use crate::domain::snapshot::{FocusedSnapshot, StateSnapshot, WindowSnapshot};

/// Nombre del modo de teclado que activa `EnterResizeMode`.
const RESIZE_MODE: &str = "resize";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Opened(WindowId),
    Closed(WindowId),
    Focused(WindowId),
    Minimized(WindowId),
    Restored(WindowId),
    MoveEnded(WindowId),
    /// Pedido (p. ej. desde el módulo de apps): enfocar una ventana, cambiando de workspace si hace falta.
    FocusWindow(WindowId),
    /// Acción disparada por un atajo o por la interfaz.
    Action(Action),
    /// Atajo de emergencia: pausa y restaura; una segunda vez reanuda.
    Emergency,
    /// La lista de exclusiones cambió (desde el panel de exclusiones, §6.3).
    SetExclusions(ExclusionList),
    /// Revisión periódica (monitores y pantallas completas).
    Tick,
    /// Cierre de la app: restaura y deja de gestionar.
    Shutdown,
    /// Se acaban de abrir aplicaciones de arranque: sus ventanas nuevas se mandan a su workspace.
    ExpectStartup(Vec<StartupExpect>),
}

/// Una aplicación de arranque cuya ventana se espera: la ventana nueva de ese ejecutable va a `workspace` (0..8)
/// durante `ttl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupExpect {
    /// Nombre normalizado del ejecutable (sin `.exe`, en minúsculas).
    pub exe: String,
    pub workspace: usize,
    pub ttl: Duration,
    /// También llevar a `workspace` las ventanas de esa aplicación que ya estaban abiertas.
    pub move_existing: bool,
}

/// Lee y valida la configuración actual (archivo en producción).
pub type ConfigLoader = Box<dyn Fn() -> Result<ValidConfig, Vec<String>> + Send>;

pub struct Controller<P: WindowPlatform> {
    desk: Desktop,
    platform: P,
    scope: Scope,
    settings: Settings,
    keymap: Keymap,
    exclusions: ExclusionList,
    rules: Vec<WindowRule>,
    /// Aplicaciones de arranque a la espera de su ventana: (ejecutable, workspace, hasta cuándo).
    startup: Vec<(String, usize, Instant)>,
    mode: Option<String>,
    loader: Option<ConfigLoader>,
    paused: bool,
    exit_requested: bool,
    last_state: Option<StateSnapshot>,
}

impl<P: WindowPlatform> Controller<P> {
    pub fn new(platform: P, scope: Scope) -> Self {
        Self::with_config(platform, scope, Settings::default(), Keymap::default())
    }

    pub fn with_config(platform: P, scope: Scope, settings: Settings, keymap: Keymap) -> Self {
        Self::with_all(platform, scope, settings, keymap, ExclusionList::default(), vec![])
    }

    pub fn with_all(
        platform: P,
        scope: Scope,
        settings: Settings,
        keymap: Keymap,
        exclusions: ExclusionList,
        rules: Vec<WindowRule>,
    ) -> Self {
        let desk = Desktop::new(settings.clone(), platform.monitors());
        let mut c = Controller {
            desk,
            platform,
            scope,
            settings,
            keymap,
            exclusions,
            rules,
            startup: Vec::new(),
            mode: None,
            loader: None,
            paused: false,
            exit_requested: false,
            last_state: None,
        };
        // Primero los atajos (disponibles de inmediato, incluida la salida), luego se adoptan las ventanas.
        c.apply_hotkeys();
        c.adopt_existing();
        c.sync_bar();
        c.publish_state();
        c
    }

    pub fn set_loader(&mut self, loader: ConfigLoader) {
        self.loader = Some(loader);
    }

    pub fn desktop(&self) -> &Desktop {
        &self.desk
    }

    pub fn platform(&self) -> &P {
        &self.platform
    }

    #[cfg(test)]
    pub fn platform_mut(&mut self) -> &mut P {
        &mut self.platform
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn current_mode(&self) -> Option<&str> {
        self.mode.as_deref()
    }

    /// `true` si el controlador terminó porque el usuario pidió salir de la app (alt+shift+e).
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    fn adopt_existing(&mut self) {
        for id in self.platform.top_level_windows() {
            self.try_add(id);
        }
    }

    fn try_add(&mut self, id: WindowId) {
        if self.desk.entry(id).is_some() {
            return;
        }
        let Some(facts) = self.platform.facts(id) else { return };
        if facts.minimized || !is_manageable(&facts, &self.scope) {
            return;
        }
        let outcome = apply_rules(&self.rules, &facts);
        let excluded = self.exclusions.matches(&facts);
        // Las reglas de ventana mandan; si no hay ninguna, una aplicación de arranque va a su workspace.
        let workspace = outcome.workspace.or_else(|| if excluded { None } else { self.startup_workspace(&facts.exe) });
        let opts = TrackOptions {
            excluded,
            fullscreen: facts.fullscreen,
            monitor: facts.monitor,
            workspace,
            float: outcome.float,
        };
        let fx = self.desk.track_window(id, opts);
        self.run(fx);
    }

    /// Workspace (0..8) de una aplicación de arranque cuya ventana aún se espera; olvida las que caducaron.
    fn startup_workspace(&mut self, exe: &str) -> Option<usize> {
        let now = Instant::now();
        self.startup.retain(|(_, _, until)| *until > now);
        let exe = normalize_exe(exe);
        let found = self.startup.iter().find(|(e, _, _)| *e == exe).map(|(_, ws, _)| *ws);
        if let Some(ws) = found {
            eprintln!("[wm] arranque: «{exe}» → workspace {}", ws + 1);
        }
        found
    }

    /// Publica el estado para la interfaz solo cuando cambió.
    fn publish_state(&mut self) {
        let (monitors, workspaces) = self.desk.snapshot();
        let focused = self
            .desk
            .focused_window()
            .and_then(|id| self.platform.facts(id))
            .map(|f| FocusedSnapshot { exe: f.exe, title: f.title });
        let focused_id = self.desk.focused_window();
        let windows = self
            .desk
            .taskbar_windows()
            .into_iter()
            .filter_map(|(id, ws, minimized)| {
                let f = self.platform.facts(id)?;
                Some(WindowSnapshot {
                    id,
                    workspace: ws + 1,
                    title: f.title,
                    exe: normalize_exe(&f.exe),
                    focused: Some(id) == focused_id && ws == self.desk.active_workspace(),
                    minimized,
                })
            })
            .collect();
        let state = StateSnapshot { monitors, workspaces, focused, paused: self.paused, mode: self.mode.clone(), windows };
        if self.last_state.as_ref() != Some(&state) {
            self.platform.publish_state(&state);
            self.last_state = Some(state);
        }
    }

    /// Aplica los cambios de visibilidad de la barra (§6.4).
    fn sync_bar(&mut self) {
        let fx = self.desk.sync_bar();
        self.run(fx);
    }

    /// Revisa las ventanas seguidas: ¿entraron o salieron de pantalla completa por su cuenta?
    fn check_fullscreen(&mut self) {
        for (id, user) in self.desk.tracked_windows() {
            if user {
                continue; // la pantalla completa pedida con el atajo la controla la app
            }
            match self.platform.fullscreen_state(id) {
                Some((fs, monitor)) => {
                    let fx = self.desk.set_fullscreen(id, fs, monitor, false);
                    self.run(fx);
                }
                None => {
                    // La ventana ya no existe y no llegó el evento de cierre.
                    let fx = self.desk.remove_window(id);
                    self.platform.forget(id);
                    self.run(fx);
                }
            }
        }
    }

    /// Suelta las ventanas que deberían verse pero Windows ya oculta (apps que se esconden en la bandeja, ventanas
    /// ocultas por el sistema al cambiar de escritorio virtual...): sin esto dejan un hueco invisible en el mosaico.
    /// Si vuelven a mostrarse llega su evento y se incorporan de nuevo.
    fn drop_ghost_windows(&mut self) {
        for id in self.desk.shown_windows() {
            let Some(facts) = self.platform.facts(id) else { continue };
            if !facts.visible || facts.cloaked {
                let fx = self.desk.remove_window(id);
                self.run(fx);
            }
        }
    }

    /// Reevalúa todas las ventanas con la lista de exclusiones actual.
    fn reapply_exclusions(&mut self) {
        for id in self.desk.all_windows() {
            let Some(facts) = self.platform.facts(id) else { continue };
            let excluded = self.exclusions.matches(&facts);
            let fx = self.desk.set_excluded(id, excluded);
            self.run(fx);
        }
    }

    fn run(&mut self, effects: Vec<Effect>) {
        let mut handle = None;
        let effects = match self.desk.take_transition() {
            Some(t) => {
                // Las ventanas que entran se colocan ya (siguen ocultas): la animación las dibuja en su sitio final.
                let (early, rest): (Vec<Effect>, Vec<Effect>) =
                    effects.into_iter().partition(|e| matches!(e, Effect::Place { id, .. } if t.inn.contains(id)));
                self.drain(early.into());
                handle = self.platform.slide_begin(&t);
                rest
            }
            None => effects,
        };
        self.drain(effects.into());
        if let Some(h) = handle {
            self.platform.slide_end(h);
        }
    }

    fn drain(&mut self, mut queue: std::collections::VecDeque<Effect>) {
        while let Some(e) = queue.pop_front() {
            let Err(err) = self.platform.apply(&e) else { continue };
            eprintln!("[wm] no se pudo aplicar {e:?}: {err}");
            // Una ventana que Windows no deja mover (p. ej. el Administrador de tareas, que corre con más
            // privilegios) no puede ocupar un hueco del mosaico: se excluye y el resto se reparte sin ella.
            if let (Effect::Place { id, .. }, true) = (&e, err.starts_with(DENIED)) {
                let title = self.platform.facts(*id).map(|f| f.title).unwrap_or_default();
                self.platform.notify(&format!("«{title}» no se puede mover (¿se ejecuta como administrador?): se deja fuera del mosaico."));
                queue.extend(self.desk.set_excluded(*id, true));
            }
        }
    }

    fn check_monitors(&mut self) {
        let current = self.platform.monitors();
        if current != self.desk.monitors() {
            let fx = self.desk.set_monitors(current);
            self.run(fx);
        }
    }

    /// Registra los atajos que corresponden al estado actual (pausa, modo o globales).
    fn apply_hotkeys(&mut self) {
        let bindings = if self.paused {
            self.keymap.pause_bindings()
        } else if let Some(m) = self.mode.as_deref().and_then(|m| self.keymap.mode(m)) {
            m.to_vec()
        } else {
            self.keymap.global().to_vec()
        };
        let mut failed = self.platform.set_hotkeys(&bindings);
        // El del buscador se explica aparte: es el más probable que choque con otro programa y se cambia en el archivo.
        let search_chords: Vec<String> = bindings.iter().filter(|(_, a)| *a == Action::ToggleSearch).map(|(c, _)| c.to_string()).collect();
        let search_failed: Vec<String> = failed.iter().filter(|c| search_chords.contains(c)).cloned().collect();
        failed.retain(|c| !search_failed.contains(c));
        if !search_failed.is_empty() {
            self.platform.notify(&format!(
                "El atajo del buscador ({}) ya lo usa otro programa. Cámbialo en config.toml (acción «toggle_search») y recarga con alt+shift+r.",
                search_failed.join(", ")
            ));
        }
        if !failed.is_empty() {
            self.platform.notify(&format!("No se pudieron registrar estos atajos (¿otro programa los usa?): {}", failed.join(", ")));
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if paused == self.paused {
            return;
        }
        self.paused = paused;
        self.mode = None;
        if paused {
            self.platform.restore_all();
        } else {
            self.desk = Desktop::new(self.settings.clone(), self.platform.monitors());
            self.adopt_existing();
        }
        self.apply_hotkeys();
    }

    fn reload_config(&mut self) {
        let Some(loader) = &self.loader else {
            self.platform.notify("No hay archivo de configuración que recargar");
            return;
        };
        match loader() {
            Ok(valid) => {
                self.keymap = valid.keymap;
                self.rules = valid.rules;
                self.settings = valid.settings.clone();
                self.mode = None;
                let fx = self.desk.update_settings(valid.settings);
                self.run(fx);
                self.apply_hotkeys();
                self.platform.notify("Configuración recargada");
            }
            Err(errors) => {
                // §13: con configuración inválida se conserva la anterior y se avisa.
                self.platform.notify(&format!(
                    "Configuración inválida; se conservan los atajos anteriores. {}",
                    errors.join(" | ")
                ));
            }
        }
    }

    fn run_action(&mut self, action: Action) {
        match action {
            Action::EnterResizeMode => {
                if self.keymap.mode(RESIZE_MODE).is_some() {
                    self.mode = Some(RESIZE_MODE.into());
                    self.apply_hotkeys();
                }
            }
            Action::ExitResizeMode => {
                self.mode = None;
                self.apply_hotkeys();
            }
            Action::TogglePause => self.set_paused(!self.paused),
            Action::ReloadConfig => self.reload_config(),
            Action::OpenTerminal => self.platform.open_terminal(),
            Action::TogglePanel => {
                let monitor = self.desk.active_monitor();
                self.platform.toggle_panel(monitor);
            }
            Action::ToggleSearch => self.platform.toggle_search(),
            Action::Exit => {
                self.exit_requested = true;
            }
            other => {
                let fx = self.desk.apply_action(&other);
                self.run(fx);
            }
        }
    }

    /// Procesa un mensaje. Devuelve `false` cuando el controlador debe terminar.
    pub fn handle(&mut self, msg: Message) -> bool {
        if std::env::var("WM_TRACE").is_ok() && !matches!(msg, Message::Tick) {
            eprintln!("[wm-trace] {msg:?}");
        }
        match msg {
            Message::Emergency => self.set_paused(!self.paused),
            Message::Shutdown => {
                self.platform.restore_all();
                let _ = self.platform.set_hotkeys(&[]);
                return false;
            }
            Message::Action(Action::TogglePause) => self.set_paused(!self.paused),
            // El buscador no toca ventanas: también funciona con el gestor en pausa.
            Message::Action(Action::ToggleSearch) => self.platform.toggle_search(),
            Message::Action(Action::Exit) => {
                self.platform.restore_all();
                let _ = self.platform.set_hotkeys(&[]);
                self.exit_requested = true;
                return false;
            }
            _ if self.paused => {}
            Message::ExpectStartup(list) => {
                let now = Instant::now();
                for e in list {
                    let ws = e.workspace.min(8);
                    if e.move_existing {
                        let ids: Vec<WindowId> = self
                            .desk
                            .tracked_windows()
                            .into_iter()
                            .map(|(id, _)| id)
                            .filter(|&id| !self.desk.is_excluded(id) && self.platform.facts(id).is_some_and(|f| normalize_exe(&f.exe) == e.exe))
                            .collect();
                        for id in ids {
                            let fx = self.desk.move_window_quietly(id, ws);
                            self.run(fx);
                        }
                    }
                    self.startup.retain(|(exe, _, _)| *exe != e.exe);
                    self.startup.push((e.exe, ws, now + e.ttl));
                }
            }
            Message::Opened(id) => self.try_add(id),
            Message::Closed(id) => {
                let fx = self.desk.remove_window(id);
                self.platform.forget(id);
                self.run(fx);
            }
            Message::Focused(id) => self.desk.note_focus(id),
            Message::Minimized(id) => {
                let fx = self.desk.set_minimized(id, true);
                self.run(fx);
            }
            Message::Restored(id) => {
                if self.desk.entry(id).is_some() {
                    let fx = self.desk.set_minimized(id, false);
                    self.run(fx);
                } else {
                    self.try_add(id);
                }
            }
            Message::MoveEnded(_) => {
                let fx = self.desk.relayout();
                self.run(fx);
            }
            Message::FocusWindow(id) => {
                let fx = self.desk.focus_window(id);
                self.run(fx);
            }
            Message::Action(a) => self.run_action(a),
            Message::SetExclusions(list) => {
                self.exclusions = list;
                self.reapply_exclusions();
            }
            Message::Tick => {
                // Mientras se espera la ventana de una aplicación de arranque se recogen también las que nacieron sin
                // título o aún sin mostrar (Electron): su evento de «mostrada» ya pasó y no se volvería a evaluar.
                let now = Instant::now();
                self.startup.retain(|(_, _, until)| *until > now);
                if !self.startup.is_empty() {
                    self.adopt_existing();
                }
                self.check_monitors();
                self.check_fullscreen();
                self.drop_ghost_windows();
            }
        }
        self.sync_bar();
        self.publish_state();
        true
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use crate::application::ports::WindowPlatform;
    use crate::domain::desktop::Monitor;
    use crate::domain::geometry::Rect;
    use crate::domain::manageable::WindowFacts;
    use crate::domain::shortcuts::Chord;
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    pub struct FakeWindow {
        pub facts: WindowFacts,
        pub rect: Rect,
        pub original: Rect,
        pub hidden: bool,
        pub touched: bool,
    }

    pub struct FakePlatform {
        pub monitors: Vec<Monitor>,
        pub windows: HashMap<WindowId, FakeWindow>,
        pub registered: Vec<(Chord, Action)>,
        pub notices: Vec<String>,
        pub terminals: u32,
        pub focus_log: Vec<WindowId>,
        pub closed: Vec<WindowId>,
        pub apply_count: usize,
        pub bar_log: Vec<(u32, bool)>,
        pub states: Vec<StateSnapshot>,
        pub panel_log: Vec<u32>,
        /// Cuántas veces se pidió abrir o cerrar el buscador.
        pub search_toggles: u32,
        pub minimize_log: Vec<WindowId>,
        /// Combinaciones que el sistema "rechaza" al registrar (simula otro programa que las usa).
        pub reject: Vec<String>,
        /// Ventanas que el sistema "no deja mover" (simula programas con más privilegios).
        pub deny_move: Vec<WindowId>,
        /// Si `true`, `slide_begin` simula una animación y deja constancia en `slide_log`.
        pub slide_enabled: bool,
        /// «begin salen=[ocultas] entran=[ocultas]» y «end»: el estado de las ventanas en cada momento.
        pub slide_log: Vec<String>,
    }

    impl FakePlatform {
        pub fn new() -> Self {
            FakePlatform {
                monitors: vec![Monitor::new(0, Rect::new(0, 0, 1000, 600), true)],
                windows: HashMap::new(),
                registered: vec![],
                notices: vec![],
                terminals: 0,
                focus_log: vec![],
                closed: vec![],
                apply_count: 0,
                bar_log: vec![],
                states: vec![],
                panel_log: vec![],
                search_toggles: 0,
                minimize_log: vec![],
                reject: vec![],
                deny_move: vec![],
                slide_enabled: false,
                slide_log: vec![],
            }
        }

        pub fn spawn(&mut self, id: WindowId, title: &str) {
            let rect = Rect::new(37 + id as i32, 41, 300, 200);
            self.windows.insert(
                id,
                FakeWindow {
                    facts: WindowFacts {
                        handle: id,
                        title: title.into(),
                        class: "Form".into(),
                        visible: true,
                        ..Default::default()
                    },
                    rect,
                    original: rect,
                    hidden: false,
                    touched: false,
                },
            );
        }

        pub fn destroy(&mut self, id: WindowId) {
            self.windows.remove(&id);
        }
    }

    impl WindowPlatform for FakePlatform {
        fn facts(&self, id: WindowId) -> Option<WindowFacts> {
            self.windows.get(&id).map(|w| w.facts.clone())
        }
        fn top_level_windows(&self) -> Vec<WindowId> {
            let mut v: Vec<_> = self.windows.keys().copied().collect();
            v.sort();
            v
        }
        fn monitors(&self) -> Vec<Monitor> {
            self.monitors.clone()
        }
        fn apply(&mut self, e: &Effect) -> Result<(), String> {
            self.apply_count += 1;
            match e {
                Effect::Place { id, rect } => {
                    if self.deny_move.contains(id) {
                        return Err(format!("{}: simulado", crate::application::ports::DENIED));
                    }
                    let w = self.windows.get_mut(id).ok_or("no existe")?;
                    if !w.touched {
                        w.original = w.rect;
                        w.touched = true;
                    }
                    w.rect = *rect;
                }
                Effect::Hide(id) => {
                    let w = self.windows.get_mut(id).ok_or("no existe")?;
                    w.touched = true;
                    w.hidden = true;
                }
                Effect::Show(id) => self.windows.get_mut(id).ok_or("no existe")?.hidden = false,
                Effect::Focus(id) => self.focus_log.push(*id),
                Effect::Close(id) => self.closed.push(*id),
                Effect::Minimize(id) => self.minimize_log.push(*id),
                Effect::Restore(_) => {}
                Effect::SetBar { monitor, visible } => self.bar_log.push((*monitor, *visible)),
            }
            Ok(())
        }
        fn forget(&mut self, _id: WindowId) {}
        fn restore_all(&mut self) {
            for w in self.windows.values_mut().filter(|w| w.touched) {
                w.rect = w.original;
                w.hidden = false;
            }
        }
        fn fullscreen_state(&self, id: WindowId) -> Option<(bool, Option<u32>)> {
            self.windows.get(&id).map(|w| (w.facts.fullscreen, w.facts.monitor))
        }
        fn set_hotkeys(&mut self, bindings: &[(Chord, Action)]) -> Vec<String> {
            let (ok, bad): (Vec<_>, Vec<_>) = bindings.iter().partition(|(c, _)| !self.reject.contains(&c.to_string()));
            self.registered = ok.into_iter().copied().collect();
            bad.into_iter().map(|(c, _)| c.to_string()).collect()
        }
        fn open_terminal(&mut self) {
            self.terminals += 1;
        }
        fn publish_state(&mut self, state: &StateSnapshot) {
            self.states.push(state.clone());
        }
        fn toggle_panel(&mut self, monitor: u32) {
            self.panel_log.push(monitor);
        }
        fn toggle_search(&mut self) {
            self.search_toggles += 1;
        }
        fn notify(&mut self, text: &str) {
            self.notices.push(text.to_string());
        }
        fn slide_begin(&mut self, t: &crate::domain::effects::Transition) -> Option<crate::application::ports::SlideHandle> {
            if !self.slide_enabled {
                return None;
            }
            let hidden = |ids: &[WindowId]| ids.iter().map(|id| self.windows[id].hidden).collect::<Vec<_>>();
            let line = format!("begin salen_ocultas={:?} entran_ocultas={:?}", hidden(&t.out), hidden(&t.inn));
            self.slide_log.push(line);
            Some(crate::application::ports::SlideHandle(Box::new(())))
        }
        fn slide_end(&mut self, _h: crate::application::ports::SlideHandle) {
            self.slide_log.push("end".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakePlatform;
    use super::*;
    use crate::domain::config::{Config, KeybindingConfig};
    use crate::domain::shortcuts::Chord;

    fn ctl_with(n: isize) -> Controller<FakePlatform> {
        let mut p = FakePlatform::new();
        for i in 1..=n {
            p.spawn(i, &format!("WMTEST {i}"));
        }
        Controller::new(p, Scope::TitlePrefix("WMTEST".into()))
    }

    fn act(c: &mut Controller<FakePlatform>, a: Action) {
        c.handle(Message::Action(a));
    }

    /// Invariante de la Fase 1: ninguna ventana gestionada se pierde ni queda fuera de pantalla.
    fn assert_no_lost_windows(c: &Controller<FakePlatform>) {
        let area = c.platform().monitors[0].work_area;
        let active = c.desktop().active_workspace();
        for (id, w) in &c.platform().windows {
            match c.desktop().workspace_of(*id) {
                Some(ws) if ws == active => {
                    assert!(!w.hidden, "ventana {id} activa está oculta");
                    assert!(area.contains_rect(&w.rect), "ventana {id} fuera de pantalla: {:?}", w.rect);
                }
                Some(_) => assert!(w.hidden, "ventana {id} de otro workspace debería estar oculta"),
                None => {}
            }
        }
    }

    #[test]
    fn cambiar_de_workspace_anima_con_las_viejas_visibles_y_las_nuevas_ocultas_y_retira_al_final() {
        let mut p = FakePlatform::new();
        p.slide_enabled = true;
        p.spawn(1, "WMTEST 1");
        let mut c = Controller::new(p, Scope::TitlePrefix("WMTEST".into()));
        act(&mut c, Action::MoveToWorkspace(2)); // la ventana se va al 2 y la app la sigue
        act(&mut c, Action::FocusWorkspace(1)); // el 1 está vacío: solo sale la ventana
        assert_eq!(c.platform().slide_log.last().map(String::as_str), Some("end"));
        assert!(c.platform().slide_log.iter().any(|l| l == "begin salen_ocultas=[false] entran_ocultas=[]"), "{:?}", c.platform().slide_log);
        c.platform_mut().slide_log.clear();
        act(&mut c, Action::FocusWorkspace(2)); // vuelve: solo entra
        assert_eq!(
            c.platform().slide_log,
            vec!["begin salen_ocultas=[] entran_ocultas=[true]".to_string(), "end".to_string()],
            "al empezar la entrante aún está oculta; al acabar se retira la capa"
        );
        assert!(!c.platform().windows[&1].hidden, "al final la ventana nueva se ve");
        assert_no_lost_windows(&c);
    }

    #[test]
    fn con_salientes_y_entrantes_las_viejas_siguen_visibles_al_empezar() {
        let mut p = FakePlatform::new();
        p.slide_enabled = true;
        p.spawn(1, "WMTEST 1");
        p.spawn(2, "WMTEST 2");
        let mut c = Controller::new(p, Scope::TitlePrefix("WMTEST".into()));
        act(&mut c, Action::MoveToWorkspace(2)); // la enfocada se va al 2; la otra se queda en el 1
        c.platform_mut().slide_log.clear();
        act(&mut c, Action::FocusWorkspace(1));
        assert_eq!(
            c.platform().slide_log,
            vec!["begin salen_ocultas=[false] entran_ocultas=[true]".to_string(), "end".to_string()]
        );
        assert_no_lost_windows(&c);
    }

    #[test]
    fn sin_animacion_disponible_el_cambio_se_hace_igual() {
        let mut c = ctl_with(2); // el doble no anima por defecto
        act(&mut c, Action::MoveToWorkspace(2));
        act(&mut c, Action::FocusWorkspace(2));
        assert!(c.platform().slide_log.is_empty());
        assert_no_lost_windows(&c);
    }

    #[test]
    fn adopta_solo_ventanas_del_alcance() {
        let mut p = FakePlatform::new();
        p.spawn(1, "WMTEST 1");
        p.spawn(2, "Chrome del usuario");
        let c = Controller::new(p, Scope::TitlePrefix("WMTEST".into()));
        assert_eq!(c.desktop().windows_in(0), vec![1]);
        assert!(!c.platform().windows[&2].touched, "no debe tocar ventanas fuera del alcance");
    }

    #[test]
    fn abrir_cerrar_y_cambiar_sin_perder_ventanas() {
        let mut c = ctl_with(3);
        assert_no_lost_windows(&c);
        c.platform.spawn(4, "WMTEST 4");
        c.handle(Message::Opened(4));
        assert_no_lost_windows(&c);
        act(&mut c, Action::FocusWorkspace(2));
        assert_no_lost_windows(&c);
        c.platform.spawn(5, "WMTEST 5");
        c.handle(Message::Opened(5));
        act(&mut c, Action::FocusWorkspace(1));
        assert_no_lost_windows(&c);
        c.platform.destroy(2);
        c.handle(Message::Closed(2));
        assert_no_lost_windows(&c);
        act(&mut c, Action::MoveToWorkspace(3));
        assert_no_lost_windows(&c);
        act(&mut c, Action::CycleFocus);
        assert_no_lost_windows(&c);
    }

    #[test]
    fn emergencia_restaura_posiciones_originales() {
        let mut c = ctl_with(3);
        act(&mut c, Action::FocusWorkspace(2));
        act(&mut c, Action::FocusWorkspace(1));
        c.handle(Message::Emergency);
        assert!(c.is_paused());
        for w in c.platform().windows.values() {
            assert_eq!(w.rect, w.original);
            assert!(!w.hidden);
        }
        // En pausa no se gestiona nada.
        c.platform.spawn(9, "WMTEST 9");
        c.handle(Message::Opened(9));
        assert!(c.desktop().workspace_of(9).is_none());
    }

    #[test]
    fn emergencia_con_ventanas_en_otro_workspace_las_muestra() {
        let mut c = ctl_with(2);
        act(&mut c, Action::FocusWorkspace(4));
        assert!(c.platform().windows[&1].hidden);
        c.handle(Message::Emergency);
        assert!(!c.platform().windows[&1].hidden);
    }

    #[test]
    fn segunda_emergencia_reanuda() {
        let mut c = ctl_with(2);
        c.handle(Message::Emergency);
        c.handle(Message::Emergency);
        assert!(!c.is_paused());
        assert_eq!(c.desktop().windows_in(0).len(), 2);
        assert_no_lost_windows(&c);
    }

    #[test]
    fn shutdown_restaura_y_termina() {
        let mut c = ctl_with(2);
        assert!(!c.handle(Message::Shutdown));
        for w in c.platform().windows.values() {
            assert_eq!(w.rect, w.original);
        }
        assert!(c.platform().registered.is_empty(), "libera los atajos");
    }

    #[test]
    fn ventana_cerrada_antes_de_abrirse_no_falla() {
        let mut c = ctl_with(1);
        c.handle(Message::Opened(77));
        assert!(c.desktop().workspace_of(77).is_none());
    }

    #[test]
    fn minimizar_y_restaurar_reacomoda_el_resto() {
        let mut c = ctl_with(3);
        c.handle(Message::Minimized(2));
        assert_eq!(c.desktop().windows_in(0), vec![1, 3]);
        c.handle(Message::Restored(2));
        assert_eq!(c.desktop().windows_in(0), vec![1, 2, 3]);
        assert_no_lost_windows(&c);
    }

    #[test]
    fn cambio_de_monitores_se_detecta_en_el_tick() {
        use crate::domain::desktop::Monitor;
        use crate::domain::geometry::Rect;
        let mut c = ctl_with(2);
        c.platform.monitors = vec![Monitor::new(0, Rect::new(0, 0, 800, 500), true)];
        c.handle(Message::Tick);
        for w in c.platform().windows.values() {
            assert!(c.platform().monitors[0].work_area.contains_rect(&w.rect));
        }
    }

    // ---------- Fase 3: atajos y configuración ----------

    fn config_with(bindings: &[(&str, &[&str], Option<&str>)]) -> Config {
        Config {
            keybindings: bindings
                .iter()
                .map(|(a, k, m)| KeybindingConfig {
                    action: a.to_string(),
                    keys: k.iter().map(|s| s.to_string()).collect(),
                    mode: m.map(String::from),
                })
                .collect(),
            ..Default::default()
        }
    }

    fn ctl_cfg(n: isize, cfg: &Config) -> Controller<FakePlatform> {
        let valid = cfg.validate().unwrap();
        let mut p = FakePlatform::new();
        for i in 1..=n {
            p.spawn(i, &format!("WMTEST {i}"));
        }
        Controller::with_config(p, Scope::TitlePrefix("WMTEST".into()), valid.settings, valid.keymap)
    }

    /// Simula pulsar una combinación: solo funciona si está registrada ahora mismo.
    fn press(c: &mut Controller<FakePlatform>, chord: &str) -> bool {
        let chord = Chord::parse(chord).unwrap();
        let Some((_, action)) = c.platform.registered.iter().find(|(ch, _)| *ch == chord).copied() else {
            return false;
        };
        c.handle(Message::Action(action));
        true
    }

    #[test]
    fn el_modo_redimensionar_cambia_los_atajos_registrados() {
        let cfg = config_with(&[
            ("resize_mode", &["alt+r"], None),
            ("focus_left", &["alt+h"], None),
            ("resize_width_-2", &["h"], Some("resize")),
            ("exit_resize_mode", &["escape", "enter"], Some("resize")),
        ]);
        let mut c = ctl_cfg(2, &cfg);
        assert!(press(&mut c, "alt+h"));
        assert!(!press(&mut c, "h"), "fuera del modo no está registrada");
        assert!(press(&mut c, "alt+r"));
        assert_eq!(c.current_mode(), Some("resize"));
        assert!(!press(&mut c, "alt+h"), "dentro del modo solo valen las del modo");
        assert!(press(&mut c, "h"));
        assert!(press(&mut c, "enter"));
        assert_eq!(c.current_mode(), None);
        assert!(press(&mut c, "alt+h"));
        // Escape también sale.
        press(&mut c, "alt+r");
        assert!(press(&mut c, "escape"));
        assert_eq!(c.current_mode(), None);
    }

    #[test]
    fn pausar_libera_todos_los_atajos_menos_el_de_pausa() {
        let cfg = config_with(&[("toggle_pause", &["alt+shift+p"], None), ("focus_left", &["alt+h"], None)]);
        let mut c = ctl_cfg(2, &cfg);
        c.platform.windows.get_mut(&1).unwrap().hidden = false;
        assert!(press(&mut c, "alt+shift+p"));
        assert!(c.is_paused());
        assert!(!press(&mut c, "alt+h"));
        assert_eq!(c.platform().registered.len(), 1);
        assert!(press(&mut c, "alt+shift+p"));
        assert!(!c.is_paused());
        assert!(press(&mut c, "alt+h"));
        assert_no_lost_windows(&c);
    }

    #[test]
    fn recargar_con_atajos_validos_los_aplica() {
        let cfg = config_with(&[("focus_left", &["alt+h"], None), ("reload_config", &["alt+shift+r"], None)]);
        let mut c = ctl_cfg(2, &cfg);
        c.set_loader(Box::new(|| {
            config_with(&[("focus_right", &["alt+l"], None), ("reload_config", &["alt+shift+r"], None)]).validate()
        }));
        assert!(press(&mut c, "alt+shift+r"));
        assert!(!press(&mut c, "alt+h"));
        assert!(press(&mut c, "alt+l"));
        assert!(c.platform().notices.last().unwrap().contains("recargada"));
    }

    #[test]
    fn recargar_con_atajos_invalidos_conserva_los_anteriores_y_avisa() {
        let cfg = config_with(&[("focus_left", &["alt+h"], None), ("reload_config", &["alt+shift+r"], None)]);
        let mut c = ctl_cfg(2, &cfg);
        c.set_loader(Box::new(|| {
            config_with(&[("focus_right", &["alt+l"], None), ("move_right", &["alt+l"], None)]).validate()
        }));
        assert!(press(&mut c, "alt+shift+r"));
        assert!(press(&mut c, "alt+h"), "conserva los anteriores");
        assert!(!press(&mut c, "alt+l"));
        let notice = c.platform().notices.last().unwrap();
        assert!(notice.contains("inválida") && notice.contains("alt+l"), "{notice}");
    }

    #[test]
    fn atajo_que_el_sistema_rechaza_se_avisa() {
        let cfg = config_with(&[("focus_left", &["alt+h"], None), ("focus_right", &["alt+l"], None)]);
        let valid = cfg.validate().unwrap();
        let mut p = FakePlatform::new();
        p.reject = vec!["alt+h".into()];
        let c = Controller::with_config(p, Scope::All, valid.settings, valid.keymap);
        assert_eq!(c.platform().registered.len(), 1);
        assert!(c.platform().notices[0].contains("alt+h"));
    }

    #[test]
    fn abrir_terminal_y_salir() {
        let cfg = config_with(&[("open_terminal", &["alt+enter"], None), ("exit", &["alt+shift+e"], None)]);
        let mut c = ctl_cfg(1, &cfg);
        assert!(press(&mut c, "alt+enter"));
        assert_eq!(c.platform().terminals, 1);
        assert!(!c.exit_requested());
        press(&mut c, "alt+shift+e");
        assert!(c.exit_requested());
        for w in c.platform().windows.values() {
            assert_eq!(w.rect, w.original, "salir restaura las ventanas");
        }
    }

    // ---------- Cada atajo de la sección 8 ejecuta su acción ----------

    /// Acciones previas para dejar el escritorio en un estado donde la acción probada tenga efecto visible.
    fn prelude(action: &Action) -> Vec<Action> {
        use crate::domain::layout::{Dir, Orientation};
        match *action {
            Action::FocusDir(Dir::Right) | Action::MoveDir(Dir::Right) => vec![Action::FocusDir(Dir::Left)],
            // Hay un monitor debajo pero ninguno encima: subir apila la ventana sobre su vecina.
            Action::FocusDir(Dir::Up) => vec![Action::MoveDir(Dir::Up), Action::FocusDir(Dir::Down)],
            Action::MoveDir(Dir::Up) => vec![],
            Action::FocusDir(Dir::Down) | Action::MoveDir(Dir::Down) => vec![Action::MoveDir(Dir::Up)],
            Action::Resize { orientation: Orientation::Y, .. } | Action::ToggleStack => vec![Action::MoveDir(Dir::Up)],
            Action::FocusWorkspace(1)
            | Action::NextWorkspace
            | Action::PrevWorkspace
            | Action::RecentWorkspace => vec![Action::FocusWorkspace(2)],
            Action::MoveToWorkspace(1) => vec![Action::MoveToWorkspace(2)],
            Action::MoveWorkspaceDir(Dir::Left) => vec![Action::MoveWorkspaceDir(Dir::Right)],
            Action::MoveWorkspaceDir(Dir::Up) => vec![Action::MoveWorkspaceDir(Dir::Down)],
            _ => vec![],
        }
    }

    fn snapshot(c: &Controller<FakePlatform>) -> String {
        let mut wins: Vec<_> = c.platform().windows.iter().map(|(id, w)| (*id, w.rect, w.hidden)).collect();
        wins.sort_by_key(|w| w.0);
        format!(
            "{wins:?}|ws={}|focus={:?}|closed={:?}|min={:?}|term={}|notices={}|paused={}|mode={:?}|applies={}|exit={}|panel={:?}|search={}",
            c.desktop().active_workspace(),
            c.platform().focus_log,
            c.platform().closed,
            c.platform().minimize_log,
            c.platform().terminals,
            c.platform().notices.len(),
            c.is_paused(),
            c.current_mode(),
            c.platform().apply_count,
            c.exit_requested(),
            c.platform().panel_log,
            c.platform().search_toggles,
        )
    }

    #[test]
    fn cada_atajo_global_de_la_seccion_8_ejecuta_su_accion() {
        use crate::domain::desktop::Monitor;
        use crate::domain::geometry::Rect;
        use crate::infrastructure::config_file::{parse_config, DEFAULT_CONFIG};

        let valid = parse_config(DEFAULT_CONFIG).unwrap();
        let globals = valid.keymap.global().to_vec();
        assert!(globals.len() >= 58);
        let mut checked = 0;
        for (chord, action) in globals {
            let mut p = FakePlatform::new();
            p.monitors = vec![
                Monitor::new(0, Rect::new(0, 0, 3000, 1000), true),
                Monitor::new(1, Rect::new(3000, 0, 1000, 3000), false),
                Monitor::new(2, Rect::new(0, 1000, 3000, 1000), false),
            ];
            for i in 1..=4 {
                p.spawn(i, &format!("WMTEST {i}"));
            }
            let mut c = Controller::with_config(p, Scope::TitlePrefix("WMTEST".into()), valid.settings.clone(), valid.keymap.clone());
            c.set_loader(Box::new(|| Err(vec!["sin cambios".into()])));
            for a in prelude(&action) {
                c.handle(Message::Action(a));
            }
            let before = snapshot(&c);
            assert!(c.platform.registered.iter().any(|(ch, _)| *ch == chord), "{chord} no está registrada");
            let pressed = press(&mut c, &chord.to_string());
            assert!(pressed);
            assert_ne!(before, snapshot(&c), "el atajo {chord} ({}) no tuvo ningún efecto", action.name());
            checked += 1;
        }
        assert!(checked >= 58, "se comprobaron {checked} atajos");
    }

    /// Caso real: mover Google al segundo monitor y volver a pulsar en esa dirección no debe arrastrar a otra ventana.
    #[test]
    fn mover_a_otro_monitor_y_repetir_no_arrastra_a_otra_ventana() {
        use crate::domain::desktop::Monitor;
        use crate::domain::geometry::Rect;
        use crate::domain::layout::Dir;
        let mut p = FakePlatform::new();
        p.monitors = vec![
            Monitor::new(0, Rect::new(0, 0, 1000, 600), true),
            Monitor::new(1, Rect::new(1000, 0, 1000, 600), true),
        ];
        p.spawn(1, "WMTEST vscode");
        p.spawn(2, "WMTEST google");
        let mut c = Controller::new(p, Scope::TitlePrefix("WMTEST".into()));
        c.handle(Message::Focused(2));
        // Hasta llegar al tope derecho del monitor 1 y cruzar al 2.
        for _ in 0..3 {
            act(&mut c, Action::MoveDir(Dir::Right));
        }
        c.handle(Message::Focused(2)); // el sistema confirma el foco de la ventana movida
        let on_second = c.platform().windows[&2].rect.x >= 1000;
        assert!(on_second, "la ventana movida está en el segundo monitor");
        let focus_before = c.platform().focus_log.len();
        act(&mut c, Action::MoveDir(Dir::Right));
        act(&mut c, Action::MoveDir(Dir::Right));
        assert!(c.platform().windows[&1].rect.x < 1000, "la otra ventana se queda en su monitor");
        assert!(c.platform().windows[&2].rect.x >= 1000);
        assert_eq!(c.platform().focus_log.len(), focus_before, "no hay más cambios de foco");
    }

    #[test]
    fn una_ventana_que_windows_oculta_no_deja_un_hueco_vacio() {
        let mut p = FakePlatform::new();
        p.spawn(1, "WMTEST uno");
        p.spawn(2, "WMTEST dos");
        let mut c = Controller::new(p, Scope::TitlePrefix("WMTEST".into()));
        assert!(c.desktop().entry(2).is_some());
        // La app se esconde en la bandeja sin destruir la ventana.
        c.platform.windows.get_mut(&2).unwrap().facts.visible = false;
        c.handle(Message::Tick);
        assert!(c.desktop().entry(2).is_none(), "ya no ocupa sitio");
        let area = c.platform().monitors[0].work_area;
        assert!(c.platform().windows[&1].rect.w >= area.w - 40, "la otra ocupa todo el ancho");
        // Al volver a mostrarse regresa al mosaico.
        c.platform.windows.get_mut(&2).unwrap().facts.visible = true;
        c.handle(Message::Opened(2));
        assert!(c.desktop().entry(2).is_some());
    }

    // ---------- Fase 4: exclusiones y pantalla completa ----------

    fn excluding(exes: &[&str]) -> ExclusionList {
        let mut l = ExclusionList::default();
        for e in exes {
            l.add(e, None);
        }
        l
    }

    fn with_exe(p: &mut FakePlatform, id: WindowId, exe: &str, title: &str) {
        p.spawn(id, title);
        p.windows.get_mut(&id).unwrap().facts.exe = exe.into();
        p.windows.get_mut(&id).unwrap().facts.monitor = Some(0);
    }

    fn ctl_excl(p: FakePlatform, list: ExclusionList) -> Controller<FakePlatform> {
        Controller::with_all(p, Scope::TitlePrefix("WMTEST".into()), Settings::default(), Keymap::default(), list, vec![])
    }

    #[test]
    fn un_juego_y_una_app_excluida_no_se_tocan_y_las_demas_se_reparten_el_ancho() {
        let mut p = FakePlatform::new();
        with_exe(&mut p, 1, "editor.exe", "WMTEST editor");
        with_exe(&mut p, 2, "juego.exe", "WMTEST juego");
        with_exe(&mut p, 3, "app.exe", "WMTEST app");
        with_exe(&mut p, 4, "chat.exe", "WMTEST chat");
        let mut c = ctl_excl(p, excluding(&["juego.exe", "chat"]));
        for id in [2, 4] {
            let w = &c.platform().windows[&id];
            assert!(!w.touched && w.rect == w.original, "la excluida {id} no se toca");
        }
        let widths: Vec<i32> = [1, 3].iter().map(|id| c.platform().windows[id].rect.w).collect();
        assert_eq!(widths, vec![500, 500], "las gestionadas se reparten todo el ancho");
        // Operaciones posteriores tampoco la tocan.
        act(&mut c, Action::FocusWorkspace(2));
        act(&mut c, Action::FocusWorkspace(1));
        c.handle(Message::MoveEnded(2));
        c.handle(Message::Emergency);
        c.handle(Message::Emergency);
        assert!(!c.platform().windows[&2].touched);
        // Cerrar la excluida: sin estado residual.
        c.platform.destroy(2);
        c.handle(Message::Closed(2));
        assert!(c.desktop().entry(2).is_none());
        assert_eq!(c.desktop().all_windows(), vec![1, 3, 4]);
    }

    #[test]
    fn marcar_y_desmarcar_exclusiones_en_caliente() {
        let mut p = FakePlatform::new();
        with_exe(&mut p, 1, "a.exe", "WMTEST a");
        with_exe(&mut p, 2, "b.exe", "WMTEST b");
        let mut c = ctl_excl(p, ExclusionList::default());
        assert_eq!(c.platform().windows[&1].rect.w, 500);
        let before = c.platform().windows[&2].rect;
        c.handle(Message::SetExclusions(excluding(&["b.exe"])));
        assert_eq!(c.platform().windows[&1].rect.w, 1000, "la otra ocupa todo el ancho");
        assert_eq!(c.platform().windows[&2].rect, before, "la excluida se queda donde está");
        c.handle(Message::SetExclusions(ExclusionList::default()));
        assert_eq!(c.platform().windows[&1].rect.w, 500);
    }

    #[test]
    fn pantalla_completa_detectada_oculta_la_barra_y_al_salir_reaparece() {
        let mut p = FakePlatform::new();
        with_exe(&mut p, 1, "a.exe", "WMTEST a");
        with_exe(&mut p, 2, "juego.exe", "WMTEST juego");
        let mut c = ctl_excl(p, excluding(&["juego"]));
        assert!(c.platform().bar_log.is_empty());
        c.platform.windows.get_mut(&2).unwrap().facts.fullscreen = true;
        c.handle(Message::Tick);
        assert_eq!(c.platform().bar_log, vec![(0, false)]);
        c.handle(Message::Tick);
        assert_eq!(c.platform().bar_log.len(), 1, "sin cambios no repite");
        c.platform.windows.get_mut(&2).unwrap().facts.fullscreen = false;
        c.handle(Message::Tick);
        assert_eq!(c.platform().bar_log, vec![(0, false), (0, true)]);
        // Una gestionada también oculta la barra.
        c.platform.windows.get_mut(&1).unwrap().facts.fullscreen = true;
        c.handle(Message::Tick);
        assert_eq!(c.platform().bar_log.last(), Some(&(0, false)));
    }

    #[test]
    fn el_foco_en_una_excluida_impide_cerrar_otra_ventana_por_error() {
        let mut p = FakePlatform::new();
        with_exe(&mut p, 1, "a.exe", "WMTEST a");
        with_exe(&mut p, 2, "juego.exe", "WMTEST juego");
        let mut c = ctl_excl(p, excluding(&["juego"]));
        c.handle(Message::Focused(2));
        act(&mut c, Action::CloseWindow);
        assert!(c.platform().closed.is_empty());
        c.handle(Message::Focused(1));
        act(&mut c, Action::CloseWindow);
        assert_eq!(c.platform().closed, vec![1]);
    }

    #[test]
    fn una_ventana_que_no_se_puede_mover_no_ocupa_hueco() {
        let mut p = FakePlatform::new();
        p.spawn(1, "WMTEST a");
        p.spawn(2, "WMTEST administrador");
        p.spawn(3, "WMTEST b");
        p.deny_move.push(2);
        let mut c = Controller::new(p, Scope::TitlePrefix("WMTEST".into()));
        // La ventana intocable se excluye y las otras dos se reparten el ancho entera.
        let (a, b) = (c.platform().windows[&1].rect, c.platform().windows[&3].rect);
        assert_eq!(a.w + b.w, c.platform().monitors[0].work_area.w);
        assert!(c.platform().notices.iter().any(|n| n.contains("no se puede mover")));
        c.handle(Message::Opened(3));
    }

    #[test]
    fn reglas_de_ventana_mueven_a_workspace_y_flotan() {
        use crate::domain::rules::WindowRule;
        let mut p = FakePlatform::new();
        with_exe(&mut p, 1, "code.exe", "WMTEST code");
        with_exe(&mut p, 2, "javaw.exe", "WMTEST mc");
        with_exe(&mut p, 3, "otro.exe", "WMTEST otro");
        let rules = vec![
            WindowRule { exe: Some("code".into()), title_contains: None, workspace: Some(3), float: false },
            WindowRule { exe: Some("javaw".into()), title_contains: None, workspace: None, float: true },
        ];
        let c = Controller::with_all(
            p,
            Scope::TitlePrefix("WMTEST".into()),
            Settings::default(),
            Keymap::default(),
            ExclusionList::default(),
            rules,
        );
        assert_eq!(c.desktop().workspace_of(1), Some(2));
        assert!(c.platform().windows[&1].hidden, "va a un workspace oculto");
        assert_eq!(c.desktop().entry(2).unwrap().kind, crate::domain::desktop::Kind::Floating);
        assert_eq!(c.platform().windows[&3].rect.w, 1000, "la tercera queda sola en el layout");
    }

    // ---------- Aplicaciones de arranque ----------

    fn expect(exe: &str, workspace: usize, ttl: Duration) -> Message {
        Message::ExpectStartup(vec![StartupExpect { exe: exe.into(), workspace, ttl, move_existing: false }])
    }

    #[test]
    fn una_app_de_arranque_va_a_su_workspace_y_las_demas_se_quedan_donde_estan() {
        let mut c = Controller::new(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()));
        c.handle(expect("spotify", 4, Duration::from_secs(60)));
        with_exe(c.platform_mut(), 1, "Spotify.exe", "WMTEST musica");
        with_exe(c.platform_mut(), 2, "otro.exe", "WMTEST otro");
        c.handle(Message::Opened(1));
        c.handle(Message::Opened(2));
        assert_eq!(c.desktop().workspace_of(1), Some(4));
        assert!(c.platform().windows[&1].hidden, "el workspace 5 no se ve");
        assert_eq!(c.desktop().workspace_of(2), Some(0));
    }

    #[test]
    fn todas_las_ventanas_de_la_app_mientras_dura_la_espera_y_ninguna_despues() {
        let mut c = Controller::new(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()));
        c.handle(expect("code", 2, Duration::from_secs(60)));
        c.handle(expect("caducada", 6, Duration::ZERO));
        for id in [1, 2] {
            with_exe(c.platform_mut(), id, "code.exe", "WMTEST code");
            c.handle(Message::Opened(id));
        }
        with_exe(c.platform_mut(), 3, "caducada.exe", "WMTEST tarde");
        c.handle(Message::Opened(3));
        assert_eq!((c.desktop().workspace_of(1), c.desktop().workspace_of(2)), (Some(2), Some(2)));
        assert_eq!(c.desktop().workspace_of(3), Some(0), "la espera ya había caducado");
    }

    #[test]
    fn una_regla_de_ventana_o_una_exclusion_mandan_sobre_la_de_arranque() {
        use crate::domain::rules::WindowRule;
        let rules = vec![WindowRule { exe: Some("code".into()), title_contains: None, workspace: Some(3), float: false }];
        let mut excl = ExclusionList::default();
        excl.add("spotify", None);
        let mut c = Controller::with_all(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()), Settings::default(), Keymap::default(), excl, rules);
        c.handle(Message::ExpectStartup(vec![
            StartupExpect { exe: "code".into(), workspace: 7, ttl: Duration::from_secs(60), move_existing: false },
            StartupExpect { exe: "spotify".into(), workspace: 5, ttl: Duration::from_secs(60), move_existing: false },
        ]));
        with_exe(c.platform_mut(), 1, "code.exe", "WMTEST code");
        with_exe(c.platform_mut(), 2, "spotify.exe", "WMTEST musica");
        c.handle(Message::Opened(1));
        c.handle(Message::Opened(2));
        assert_eq!(c.desktop().workspace_of(1), Some(2), "gana la regla de ventana (workspace 3)");
        assert!(!c.platform().windows[&2].touched, "una app excluida no se mueve aunque sea de arranque");
    }

    #[test]
    fn una_ventana_que_nace_sin_titulo_se_recoge_en_el_siguiente_tick() {
        let mut c = Controller::new(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()));
        c.handle(expect("discord", 4, Duration::from_secs(60)));
        with_exe(c.platform_mut(), 1, "Discord.exe", "");
        c.handle(Message::Opened(1));
        assert_eq!(c.desktop().workspace_of(1), None, "sin título aún no se gestiona");
        c.platform_mut().windows.get_mut(&1).unwrap().facts.title = "WMTEST Discord".into();
        c.handle(Message::Tick);
        assert_eq!(c.desktop().workspace_of(1), Some(4));
        assert!(c.platform().windows[&1].hidden);
    }

    #[test]
    fn sin_esperas_el_tick_no_adopta_ventanas_nuevas() {
        let mut c = Controller::new(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()));
        with_exe(c.platform_mut(), 1, "otro.exe", "");
        c.handle(Message::Opened(1));
        c.platform_mut().windows.get_mut(&1).unwrap().facts.title = "WMTEST otro".into();
        c.handle(Message::Tick);
        assert_eq!(c.desktop().workspace_of(1), None);
    }

    #[test]
    fn una_app_ya_abierta_se_lleva_a_su_workspace_sin_cambiar_el_activo() {
        let mut c = Controller::new(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()));
        with_exe(c.platform_mut(), 1, "Spotify.exe", "WMTEST musica");
        with_exe(c.platform_mut(), 2, "otro.exe", "WMTEST otro");
        c.handle(Message::Opened(1));
        c.handle(Message::Opened(2));
        assert_eq!(c.desktop().workspace_of(1), Some(0));
        c.handle(Message::ExpectStartup(vec![StartupExpect { exe: "spotify".into(), workspace: 4, ttl: Duration::from_secs(60), move_existing: true }]));
        assert_eq!(c.desktop().workspace_of(1), Some(4));
        assert_eq!(c.desktop().workspace_of(2), Some(0));
        assert!(c.platform().windows[&1].hidden);
        assert_eq!(c.desktop().active_workspace(), 0, "no se cambia de workspace");
    }

    // ---------- Fase 5: estado para la barra y el panel ----------

    #[test]
    fn el_estado_trae_las_ventanas_por_workspace_para_la_barra_de_tareas() {
        let mut c = Controller::new(FakePlatform::new(), Scope::TitlePrefix("WMTEST".into()));
        with_exe(c.platform_mut(), 1, "Code.exe", "WMTEST uno");
        with_exe(c.platform_mut(), 2, "Spotify.exe", "WMTEST dos");
        c.handle(expect("spotify", 2, Duration::from_secs(60)));
        c.handle(Message::Opened(1));
        c.handle(Message::Opened(2));
        c.handle(Message::Minimized(1));
        let s = c.platform().states.last().unwrap().clone();
        let mut w: Vec<_> = s.windows.iter().map(|w| (w.id, w.workspace, w.exe.as_str(), w.minimized)).collect();
        w.sort();
        assert_eq!(w, [(1, 1, "code", true), (2, 3, "spotify", false)]);
        assert!(s.windows.iter().all(|w| w.title.starts_with("WMTEST")));
    }

    #[test]
    fn publica_el_estado_inicial_y_solo_cuando_cambia() {
        let mut c = ctl_with(2);
        assert_eq!(c.platform().states.len(), 1, "estado inicial");
        let s = &c.platform().states[0];
        assert_eq!(s.workspaces.len(), 9);
        assert_eq!(s.workspaces.iter().find(|w| w.number == 1).map(|w| (w.active, w.window_count)), Some((true, 2)));
        assert_eq!(s.monitors[0].visible_workspace, Some(1));
        assert!(s.monitors[0].bar_visible);
        c.handle(Message::Tick);
        c.handle(Message::Tick);
        assert_eq!(c.platform().states.len(), 1, "sin cambios no se republica");
        act(&mut c, Action::FocusWorkspace(3));
        assert_eq!(c.platform().states.len(), 2);
        let s = c.platform().states.last().unwrap();
        assert_eq!(s.workspaces.iter().find(|w| w.number == 3).map(|w| w.active), Some(true));
        assert_eq!(s.workspaces.iter().find(|w| w.number == 1).map(|w| w.active), Some(false));
    }

    #[test]
    fn el_estado_incluye_la_ventana_enfocada_pausa_y_modo() {
        let cfg = config_with(&[("resize_mode", &["alt+r"], None), ("resize_width_-2", &["h"], Some("resize"))]);
        let mut c = ctl_cfg(2, &cfg);
        let focused = c.platform().states.last().unwrap().focused.clone().expect("hay ventana enfocada");
        assert!(focused.title.starts_with("WMTEST"));
        press(&mut c, "alt+r");
        assert_eq!(c.platform().states.last().unwrap().mode.as_deref(), Some("resize"));
        c.handle(Message::Emergency);
        let s = c.platform().states.last().unwrap();
        assert!(s.paused && s.mode.is_none());
    }

    #[test]
    fn la_pantalla_completa_se_refleja_en_el_estado_de_la_barra() {
        let mut p = FakePlatform::new();
        with_exe(&mut p, 1, "a.exe", "WMTEST a");
        let mut c = ctl_excl(p, ExclusionList::default());
        c.platform.windows.get_mut(&1).unwrap().facts.fullscreen = true;
        c.handle(Message::Tick);
        assert!(!c.platform().states.last().unwrap().monitors[0].bar_visible);
    }

    #[test]
    fn el_atajo_del_panel_pide_abrirlo_en_el_monitor_activo() {
        let cfg = config_with(&[("toggle_panel", &["alt+b"], None)]);
        let mut c = ctl_cfg(1, &cfg);
        assert!(press(&mut c, "alt+b"));
        assert_eq!(c.platform().panel_log, vec![0]);
    }

    #[test]
    fn el_atajo_del_buscador_lo_abre_y_sigue_activo_con_el_gestor_en_pausa() {
        let cfg = config_with(&[("toggle_search", &["ctrl+shift+space"], None), ("toggle_pause", &["alt+shift+p"], None), ("toggle_panel", &["alt+b"], None)]);
        let mut c = ctl_cfg(1, &cfg);
        assert!(press(&mut c, "ctrl+shift+space"));
        assert_eq!(c.platform().search_toggles, 1);
        act(&mut c, Action::TogglePause);
        assert!(press(&mut c, "ctrl+shift+space"), "el buscador no toca ventanas: sirve en pausa");
        assert_eq!(c.platform().search_toggles, 2);
        assert!(!press(&mut c, "alt+b"), "el resto de atajos sí se desactiva");
    }

    #[test]
    fn si_el_atajo_del_buscador_esta_ocupado_el_aviso_explica_como_cambiarlo() {
        let cfg = config_with(&[("toggle_search", &["ctrl+shift+space"], None), ("toggle_panel", &["alt+b"], None)]);
        let valid = cfg.validate().unwrap();
        let mut p = FakePlatform::new();
        p.reject = vec!["ctrl+shift+space".into(), "alt+b".into()];
        let c = Controller::with_config(p, Scope::TitlePrefix("WMTEST".into()), valid.settings, valid.keymap);
        let notices = &c.platform().notices;
        assert!(notices.iter().any(|n| n.contains("buscador") && n.contains("toggle_search") && n.contains("ctrl+shift+space")), "{notices:?}");
        assert!(notices.iter().any(|n| n.contains("alt+b") && !n.contains("ctrl+shift+space")), "los demás se avisan aparte: {notices:?}");
    }

    #[test]
    fn el_mensaje_de_enfocar_ventana_llega_a_la_plataforma() {
        let mut c = ctl_with(2);
        act(&mut c, Action::FocusWorkspace(4));
        c.handle(Message::FocusWindow(1));
        assert_eq!(c.desktop().active_workspace(), 0);
        assert_eq!(c.platform().focus_log.last(), Some(&1));
        assert!(!c.platform().windows[&1].hidden);
    }
}
