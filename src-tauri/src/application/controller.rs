//! Controlador: traduce eventos y comandos en cambios del `Desktop` y los aplica a la plataforma.

use crate::application::ports::WindowPlatform;
use crate::domain::actions::Action;
use crate::domain::desktop::{Desktop, Settings};
use crate::domain::effects::{Effect, WindowId};
use crate::domain::manageable::{is_manageable, Scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Opened(WindowId),
    Closed(WindowId),
    Focused(WindowId),
    Minimized(WindowId),
    Restored(WindowId),
    MoveEnded(WindowId),
    /// Acción disparada por un atajo o por la interfaz.
    Action(Action),
    /// Atajo de emergencia: pausa y restaura; una segunda vez reanuda.
    Emergency,
    /// Revisión periódica (cambios de monitores).
    Tick,
    /// Cierre de la app: restaura y deja de gestionar.
    Shutdown,
}

pub struct Controller<P: WindowPlatform> {
    desk: Desktop,
    platform: P,
    scope: Scope,
    settings: Settings,
    paused: bool,
}

impl<P: WindowPlatform> Controller<P> {
    pub fn new(platform: P, scope: Scope) -> Self {
        Self::with_settings(platform, scope, Settings::default())
    }

    pub fn with_settings(platform: P, scope: Scope, settings: Settings) -> Self {
        let desk = Desktop::new(settings.clone(), platform.monitors());
        let mut c = Controller { desk, platform, scope, settings, paused: false };
        c.adopt_existing();
        c
    }

    pub fn desktop(&self) -> &Desktop {
        &self.desk
    }

    pub fn platform(&self) -> &P {
        &self.platform
    }

    pub fn is_paused(&self) -> bool {
        self.paused
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
        let fx = self.desk.add_window(id);
        self.run(fx);
    }

    fn run(&mut self, effects: Vec<Effect>) {
        for e in &effects {
            if let Err(err) = self.platform.apply(e) {
                eprintln!("[wm] no se pudo aplicar {e:?}: {err}");
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

    /// Procesa un mensaje. Devuelve `false` cuando el controlador debe terminar.
    pub fn handle(&mut self, msg: Message) -> bool {
        match msg {
            Message::Emergency => {
                if self.paused {
                    self.paused = false;
                    self.desk = Desktop::new(self.settings.clone(), self.platform.monitors());
                    self.adopt_existing();
                } else {
                    self.paused = true;
                    self.platform.restore_all();
                }
            }
            Message::Shutdown => {
                self.platform.restore_all();
                return false;
            }
            _ if self.paused => {}
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
            Message::Action(a) => {
                let fx = self.desk.apply_action(&a);
                self.run(fx);
            }
            Message::Tick => self.check_monitors(),
        }
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
    }

    impl FakePlatform {
        pub fn new() -> Self {
            FakePlatform {
                monitors: vec![Monitor { id: 0, work_area: Rect::new(0, 0, 1000, 600), primary: true }],
                windows: HashMap::new(),
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
            match e {
                Effect::Place { id, rect } => {
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
                Effect::Focus(_) | Effect::Close(_) | Effect::Minimize(_) | Effect::Restore(_) => {}
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
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakePlatform;
    use super::*;
    use crate::domain::actions::Action;

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
        c.platform.monitors = vec![Monitor { id: 0, work_area: Rect::new(0, 0, 800, 500), primary: true }];
        c.handle(Message::Tick);
        for w in c.platform().windows.values() {
            assert!(c.platform().monitors[0].work_area.contains_rect(&w.rect));
        }
    }
}
