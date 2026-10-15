//! Controlador: traduce eventos y comandos en cambios del `Desktop` y los aplica a la plataforma.

use crate::application::ports::WindowPlatform;
use crate::domain::desktop::Desktop;
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
    SwitchWorkspace(usize),
    MoveToWorkspace { ws: usize, follow: bool },
    FocusNext,
    /// Atajo de emergencia: pausa y restaura; una segunda vez reanuda.
    Emergency,
    /// Cierre de la app: restaura y deja de gestionar.
    Shutdown,
}

pub struct Controller<P: WindowPlatform> {
    desk: Desktop,
    platform: P,
    scope: Scope,
    paused: bool,
}

impl<P: WindowPlatform> Controller<P> {
    pub fn new(platform: P, scope: Scope) -> Self {
        let desk = Desktop::new(platform.work_area());
        let mut c = Controller { desk, platform, scope, paused: false };
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

    /// Procesa un mensaje. Devuelve `false` cuando el controlador debe terminar.
    pub fn handle(&mut self, msg: Message) -> bool {
        match msg {
            Message::Emergency => {
                if self.paused {
                    self.paused = false;
                    self.desk = Desktop::new(self.platform.work_area());
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
                let fx = self.desk.remove_window(id);
                self.platform.forget(id);
                self.run(fx);
            }
            Message::Restored(id) => self.try_add(id),
            Message::MoveEnded(id) => {
                if self.desk.workspace_of(id) == Some(self.desk.active_workspace()) {
                    let fx = self.desk.set_area(self.platform.work_area());
                    self.run(fx);
                }
            }
            Message::SwitchWorkspace(ws) => {
                let fx = self.desk.switch_workspace(ws);
                self.run(fx);
            }
            Message::MoveToWorkspace { ws, follow } => {
                let fx = self.desk.move_focused_to(ws, follow);
                self.run(fx);
            }
            Message::FocusNext => {
                let fx = self.desk.focus_next();
                self.run(fx);
            }
        }
        true
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
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
        pub area: Rect,
        pub windows: HashMap<WindowId, FakeWindow>,
    }

    impl FakePlatform {
        pub fn new() -> Self {
            FakePlatform { area: Rect::new(0, 0, 1000, 600), windows: HashMap::new() }
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
        fn work_area(&self) -> Rect {
            self.area
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
                Effect::Focus(_) | Effect::Close(_) => {}
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

    fn ctl_with(n: isize) -> Controller<FakePlatform> {
        let mut p = FakePlatform::new();
        for i in 1..=n {
            p.spawn(i, &format!("WMTEST {i}"));
        }
        Controller::new(p, Scope::TitlePrefix("WMTEST".into()))
    }

    /// Invariante de la Fase 1: ninguna ventana gestionada se pierde ni queda fuera de pantalla.
    fn assert_no_lost_windows(c: &Controller<FakePlatform>) {
        let area = c.platform().area;
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
        assert_eq!(c.desktop().windows_in(0), &[1]);
        assert!(!c.platform().windows[&2].touched, "no debe tocar ventanas fuera del alcance");
    }

    #[test]
    fn abrir_cerrar_y_cambiar_sin_perder_ventanas() {
        let mut c = ctl_with(3);
        assert_no_lost_windows(&c);
        c.platform.spawn(4, "WMTEST 4");
        c.handle(Message::Opened(4));
        assert_no_lost_windows(&c);
        c.handle(Message::SwitchWorkspace(1));
        assert_no_lost_windows(&c);
        c.platform.spawn(5, "WMTEST 5");
        c.handle(Message::Opened(5));
        c.handle(Message::SwitchWorkspace(0));
        assert_no_lost_windows(&c);
        c.platform.destroy(2);
        c.handle(Message::Closed(2));
        assert_no_lost_windows(&c);
        c.handle(Message::MoveToWorkspace { ws: 2, follow: true });
        assert_no_lost_windows(&c);
        c.handle(Message::FocusNext);
        assert_no_lost_windows(&c);
    }

    #[test]
    fn emergencia_restaura_posiciones_originales() {
        let mut c = ctl_with(3);
        c.handle(Message::SwitchWorkspace(1));
        c.handle(Message::SwitchWorkspace(0));
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
        c.handle(Message::SwitchWorkspace(3));
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
}
