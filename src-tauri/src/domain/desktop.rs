//! Escritorio: monitores, 9 workspaces, estados de ventana y operaciones (§4, §5, §7).
//! Es puro: recibe hechos y devuelve `Effect`s; nunca toca Windows.

use std::collections::HashMap;

use crate::domain::actions::Action;
use crate::domain::effects::{Effect, Transition, WindowId};
use crate::domain::geometry::Rect;
use crate::domain::snapshot::{MonitorSnapshot, WorkspaceSnapshot};
use crate::domain::layout::{focus_neighbor, Axis, Dir, Layout, LayoutSettings, NoRoom, Orientation};

pub const WORKSPACE_COUNT: usize = 9;

pub type MonitorId = u32;

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub id: MonitorId,
    /// Área útil (descontando la barra, §4).
    pub work_area: Rect,
    /// Pantalla completa del monitor, para ventanas en pantalla completa.
    pub bounds: Rect,
    pub primary: bool,
    /// Escala de DPI del monitor (1.0 = 100 %), para dimensionar la barra.
    pub dpi_scale: f64,
}

impl Monitor {
    pub fn new(id: MonitorId, work_area: Rect, primary: bool) -> Self {
        Monitor { id, work_area, bounds: work_area, primary, dpi_scale: 1.0 }
    }

    pub fn is_vertical(&self) -> bool {
        self.work_area.h > self.work_area.w
    }

    fn center(&self) -> (i32, i32) {
        self.work_area.center()
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub layout: LayoutSettings,
    /// Eje principal en monitores horizontales.
    pub horizontal_axis: Axis,
    /// Eje principal en monitores verticales (por defecto filas, ver DECISIONS.md).
    pub vertical_axis: Axis,
    /// Monitor (por posición en la lista) al que está ligado cada workspace.
    pub bindings: [Option<usize>; WORKSPACE_COUNT],
    /// Tamaño de una ventana flotante centrada, como fracción del área útil.
    pub float_size: (f64, f64),
    /// Anotar una `Transition` al cambiar de workspace (la plataforma la anima si sabe).
    pub workspace_transition: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout: LayoutSettings::default(),
            horizontal_axis: Axis::Columns,
            vertical_axis: Axis::Rows,
            bindings: [None; WORKSPACE_COUNT],
            float_size: (0.6, 0.7),
            workspace_transition: true,
        }
    }
}

/// Cómo gestiona la app una ventana (§7). Pantalla completa y minimizada son banderas aparte (D-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Managed,
    Floating,
    Excluded,
}

#[derive(Debug, Clone)]
pub struct WindowEntry {
    pub kind: Kind,
    pub workspace: Option<usize>,
    pub minimized: bool,
    pub fullscreen: bool,
    /// Monitor donde está la ventana (solo se conoce con certeza para las excluidas).
    pub monitor: Option<MonitorId>,
    /// La pantalla completa la pidió el usuario con el atajo (la app la coloca) y no la propia ventana.
    pub user_fullscreen: bool,
    /// Columna que ocupaba al dejar el layout, para volver a su sitio (§7).
    slot: Option<usize>,
    float_rect: Option<Rect>,
}

impl WindowEntry {
    fn new(kind: Kind, workspace: Option<usize>) -> Self {
        WindowEntry {
            kind,
            workspace,
            minimized: false,
            fullscreen: false,
            monitor: None,
            user_fullscreen: false,
            slot: None,
            float_rect: None,
        }
    }
}

/// Opciones al empezar a seguir una ventana.
#[derive(Debug, Clone, Default)]
pub struct TrackOptions {
    /// La ventana coincide con una exclusión (§6).
    pub excluded: bool,
    pub fullscreen: bool,
    pub monitor: Option<MonitorId>,
    /// Workspace destino (0..8) por una regla; por defecto el activo.
    pub workspace: Option<usize>,
    /// Entrar flotante (regla de ventana).
    pub float: bool,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub monitor: MonitorId,
    pub layout: Layout,
    pub floating: Vec<WindowId>,
    pub focused: Option<WindowId>,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct Desktop {
    settings: Settings,
    monitors: Vec<Monitor>,
    workspaces: Vec<Workspace>,
    windows: HashMap<WindowId, WindowEntry>,
    active: usize,
    recent: Option<usize>,
    minimized_order: Vec<WindowId>,
    /// La ventana con el foco del sistema es una excluida: los atajos sobre la ventana no la tocan (§6.2).
    foreground_excluded: bool,
    bar_state: HashMap<MonitorId, bool>,
    transition: Option<Transition>,
}

impl Desktop {
    pub fn new(settings: Settings, mut monitors: Vec<Monitor>) -> Self {
        if monitors.is_empty() {
            monitors.push(Monitor::new(0, Rect::new(0, 0, 1920, 1080), true));
        }
        if !monitors.iter().any(|m| m.primary) {
            monitors[0].primary = true;
        }
        let mut d = Desktop {
            settings,
            monitors,
            workspaces: vec![],
            windows: HashMap::new(),
            active: 0,
            recent: None,
            minimized_order: vec![],
            foreground_excluded: false,
            bar_state: HashMap::new(),
            transition: None,
        };
        let primary = d.primary_id();
        for i in 0..WORKSPACE_COUNT {
            let mon = d.settings.bindings[i].and_then(|b| d.monitors.get(b)).map(|m| m.id).unwrap_or(primary);
            let axis = d.axis_for_id(mon);
            d.workspaces.push(Workspace {
                monitor: mon,
                layout: Layout::new(axis),
                floating: vec![],
                focused: None,
                visible: false,
            });
        }
        d.ensure_visibility();
        d.active = (0..WORKSPACE_COUNT).find(|&i| d.workspaces[i].visible && d.workspaces[i].monitor == primary).unwrap_or(0);
        d
    }

    // ---------- consultas ----------

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn monitors(&self) -> &[Monitor] {
        &self.monitors
    }

    pub fn workspace(&self, ws: usize) -> &Workspace {
        &self.workspaces[ws]
    }

    pub fn active_workspace(&self) -> usize {
        self.active
    }

    pub fn recent_workspace(&self) -> Option<usize> {
        self.recent
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.workspaces[self.active].focused
    }

    pub fn entry(&self, id: WindowId) -> Option<&WindowEntry> {
        self.windows.get(&id)
    }

    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        self.windows.get(&id).and_then(|e| e.workspace)
    }

    /// Ventanas visibles del workspace (en layout y flotantes, sin minimizadas).
    pub fn windows_in(&self, ws: usize) -> Vec<WindowId> {
        let w = &self.workspaces[ws];
        let mut out: Vec<WindowId> = w
            .layout
            .windows()
            .into_iter()
            .chain(w.floating.iter().copied())
            .filter(|id| self.windows.get(id).is_some_and(|e| !e.minimized))
            .collect();
        // Gestionadas en pantalla completa: visibles pero fuera del layout.
        let mut rest: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(id, e)| e.workspace == Some(ws) && e.kind == Kind::Managed && !e.minimized && !out.contains(id))
            .map(|(id, _)| *id)
            .collect();
        rest.sort();
        out.extend(rest);
        out
    }

    /// Todas las ventanas gestionadas de cada workspace (también las minimizadas), para la barra de tareas:
    /// `(ventana, workspace 0..8, minimizada)`, por workspace y en el orden en que están en pantalla.
    pub fn taskbar_windows(&self) -> Vec<(WindowId, usize, bool)> {
        let mut out = Vec::new();
        for ws in 0..WORKSPACE_COUNT {
            out.extend(self.windows_in(ws).into_iter().map(|id| (id, ws, false)));
            let mut minimized: Vec<WindowId> =
                self.windows.iter().filter(|(_, e)| e.workspace == Some(ws) && e.minimized).map(|(id, _)| *id).collect();
            minimized.sort();
            out.extend(minimized.into_iter().map(|id| (id, ws, true)));
        }
        out
    }

    pub fn all_windows(&self) -> Vec<WindowId> {
        let mut v: Vec<_> = self.windows.keys().copied().collect();
        v.sort();
        v
    }

    pub fn monitor_of(&self, ws: usize) -> &Monitor {
        self.monitor_by_id(self.workspaces[ws].monitor)
    }

    pub fn visible_workspace_on(&self, monitor: MonitorId) -> Option<usize> {
        (0..WORKSPACE_COUNT).find(|&i| self.workspaces[i].visible && self.workspaces[i].monitor == monitor)
    }

    /// Monitores y workspaces tal como los ve la interfaz.
    pub fn snapshot(&self) -> (Vec<MonitorSnapshot>, Vec<WorkspaceSnapshot>) {
        let active_monitor = self.workspaces[self.active].monitor;
        let monitors = self
            .monitors
            .iter()
            .map(|m| MonitorSnapshot {
                id: m.id,
                primary: m.primary,
                vertical: m.is_vertical(),
                visible_workspace: self.visible_workspace_on(m.id).map(|w| w + 1),
                bar_visible: self.bar_state.get(&m.id).copied().unwrap_or(true),
            })
            .collect();
        let workspaces = (0..WORKSPACE_COUNT)
            .map(|i| WorkspaceSnapshot {
                number: i + 1,
                monitor: self.workspaces[i].monitor,
                visible: self.workspaces[i].visible,
                active: self.workspaces[i].visible && self.workspaces[i].monitor == active_monitor && i == self.active,
                window_count: self.windows_in(i).len(),
            })
            .collect();
        (monitors, workspaces)
    }

    /// Monitor del workspace activo (donde está el foco).
    pub fn active_monitor(&self) -> MonitorId {
        self.workspaces[self.active].monitor
    }

    /// Título y ejecutable los conoce la plataforma; aquí solo el id de la ventana enfocada.
    pub fn focused_window(&self) -> Option<WindowId> {
        self.focused()
    }

    // ---------- ayudas internas ----------

    fn primary_id(&self) -> MonitorId {
        self.monitors.iter().find(|m| m.primary).unwrap_or(&self.monitors[0]).id
    }

    fn monitor_by_id(&self, id: MonitorId) -> &Monitor {
        self.monitors.iter().find(|m| m.id == id).unwrap_or_else(|| self.monitors.iter().find(|m| m.primary).unwrap_or(&self.monitors[0]))
    }

    fn axis_for_id(&self, id: MonitorId) -> Axis {
        if self.monitor_by_id(id).is_vertical() {
            self.settings.vertical_axis
        } else {
            self.settings.horizontal_axis
        }
    }

    fn area(&self, ws: usize) -> Rect {
        self.monitor_of(ws).work_area
    }

    fn float_rect_for(&self, ws: usize) -> Rect {
        let a = self.area(ws);
        a.centered((a.w as f64 * self.settings.float_size.0) as i32, (a.h as f64 * self.settings.float_size.1) as i32)
    }

    fn place_fx(&self, ws: usize) -> Vec<Effect> {
        let w = &self.workspaces[ws];
        let mut fx: Vec<Effect> = w
            .layout
            .compute(&self.area(ws), &self.settings.layout)
            .into_iter()
            .map(|(id, rect)| Effect::Place { id, rect })
            .collect();
        for &id in &w.floating {
            let e = &self.windows[&id];
            if let (false, false, Some(rect)) = (e.minimized, e.fullscreen, e.float_rect) {
                fx.push(Effect::Place { id, rect });
            }
        }
        fx
    }

    fn hide_fx(&self, ws: usize) -> Vec<Effect> {
        self.windows_in(ws).into_iter().map(Effect::Hide).collect()
    }

    fn show_fx(&self, ws: usize) -> Vec<Effect> {
        let mut fx: Vec<Effect> = self.windows_in(ws).into_iter().map(Effect::Show).collect();
        fx.extend(self.place_fx(ws));
        fx
    }

    fn relayout_visible(&self) -> Vec<Effect> {
        (0..WORKSPACE_COUNT).filter(|&i| self.workspaces[i].visible).flat_map(|i| self.place_fx(i)).collect()
    }

    fn pick_focus(&self, ws: usize) -> Option<WindowId> {
        let w = &self.workspaces[ws];
        if let Some(f) = w.focused {
            if self.windows_in(ws).contains(&f) {
                return Some(f);
            }
        }
        self.windows_in(ws).last().copied()
    }

    fn set_active(&mut self, ws: usize) {
        if ws != self.active {
            self.recent = Some(self.active);
            self.active = ws;
        }
    }

    /// Cada monitor tiene exactamente un workspace visible (si tiene alguno asignado).
    fn ensure_visibility(&mut self) {
        let ids: Vec<MonitorId> = self.monitors.iter().map(|m| m.id).collect();
        for mid in ids {
            let visible: Vec<usize> =
                (0..WORKSPACE_COUNT).filter(|&i| self.workspaces[i].monitor == mid && self.workspaces[i].visible).collect();
            for &extra in visible.iter().skip(1) {
                self.workspaces[extra].visible = false;
            }
            if visible.is_empty() {
                let pick = (0..WORKSPACE_COUNT)
                    .find(|&i| self.workspaces[i].monitor == mid)
                    .or_else(|| {
                        // Monitor sin workspaces: toma uno libre y sin ligar de otro monitor.
                        (0..WORKSPACE_COUNT).find(|&i| {
                            !self.workspaces[i].visible && self.settings.bindings[i].is_none() && self.windows_in(i).is_empty()
                        })
                    });
                if let Some(i) = pick {
                    self.workspaces[i].monitor = mid;
                    self.workspaces[i].visible = true;
                }
            }
        }
        for i in 0..WORKSPACE_COUNT {
            let axis = self.axis_for_id(self.workspaces[i].monitor);
            self.workspaces[i].layout.set_axis(axis);
        }
    }

    fn adjacent_monitor(&self, from: MonitorId, dir: Dir) -> Option<MonitorId> {
        let src = self.monitor_by_id(from);
        let (cx, cy) = src.center();
        let sb = src.bounds;
        self.monitors
            .iter()
            .filter(|m| m.id != from)
            // Un monitor solo es vecino si comparte tramo con el origen en el otro eje: dos pantallas una junto
            // a otra, con una algo más alta, no son «arriba» y «abajo» entre sí.
            .filter(|m| {
                let b = m.bounds;
                match dir.orientation() {
                    Orientation::X => b.y < sb.bottom() && sb.y < b.bottom(),
                    Orientation::Y => b.x < sb.right() && sb.x < b.right(),
                }
            })
            .filter(|m| {
                let (x, y) = m.center();
                match dir {
                    Dir::Right => x > cx,
                    Dir::Left => x < cx,
                    Dir::Down => y > cy,
                    Dir::Up => y < cy,
                }
            })
            .min_by_key(|m| {
                let (x, y) = m.center();
                match dir.orientation() {
                    Orientation::X => ((x - cx).abs(), (y - cy).abs()),
                    Orientation::Y => ((y - cy).abs(), (x - cx).abs()),
                }
            })
            .map(|m| m.id)
    }

    /// Saca la ventana de la estructura de su workspace, recordando su columna.
    fn detach(&mut self, id: WindowId) -> Option<usize> {
        let ws = self.windows.get(&id)?.workspace?;
        let area = self.area(ws);
        let settings = self.settings.layout;
        let slot = self.workspaces[ws].layout.group_index_of(id);
        let w = &mut self.workspaces[ws];
        w.layout.remove_window(id, &area, &settings);
        w.floating.retain(|&f| f != id);
        if w.focused == Some(id) {
            w.focused = None;
        }
        let e = self.windows.get_mut(&id).unwrap();
        e.workspace = None;
        if slot.is_some() {
            e.slot = slot;
        }
        let f = self.pick_focus(ws);
        self.workspaces[ws].focused = f;
        Some(ws)
    }

    /// Mete la ventana en un workspace: en el layout si cabe, si no flotante centrada (decisión 1).
    fn attach(&mut self, id: WindowId, ws: usize, want_float: bool) -> Kind {
        let area = self.area(ws);
        let settings = self.settings.layout;
        let float_rect = self.float_rect_for(ws);
        let minimized = self.windows[&id].minimized || self.windows[&id].fullscreen;
        let slot = self.windows[&id].slot;
        let kind = if minimized {
            // Minimizada o en pantalla completa: no entra en el layout; recuerda dónde volver.
            if want_float { Kind::Floating } else { Kind::Managed }
        } else if want_float {
            Kind::Floating
        } else {
            let r = match slot {
                Some(pos) => self.workspaces[ws].layout.insert_window_at(id, pos, &area, &settings),
                None => self.workspaces[ws].layout.insert_window(id, &area, &settings),
            };
            match r {
                Ok(()) => Kind::Managed,
                Err(NoRoom) => Kind::Floating,
            }
        };
        let e = self.windows.get_mut(&id).unwrap();
        e.workspace = Some(ws);
        e.kind = kind;
        if kind == Kind::Managed {
            e.slot = None;
        }
        if kind == Kind::Floating {
            e.float_rect = Some(float_rect);
            if !self.workspaces[ws].floating.contains(&id) {
                self.workspaces[ws].floating.push(id);
            }
        }
        self.workspaces[ws].focused = Some(id);
        kind
    }

    // ---------- eventos del sistema ----------

    /// Ventana nueva: entra en el workspace activo.
    pub fn add_window(&mut self, id: WindowId) -> Vec<Effect> {
        let ws = self.active;
        self.add_window_to(id, ws)
    }

    pub fn add_window_to(&mut self, id: WindowId, ws: usize) -> Vec<Effect> {
        self.track_window(id, TrackOptions { workspace: Some(ws), ..Default::default() })
    }

    /// Empieza a seguir una ventana: gestionada, o excluida (§6) que solo se vigila.
    pub fn track_window(&mut self, id: WindowId, opts: TrackOptions) -> Vec<Effect> {
        if self.windows.contains_key(&id) {
            return vec![];
        }
        if opts.excluded {
            let mut e = WindowEntry::new(Kind::Excluded, None);
            e.fullscreen = opts.fullscreen;
            e.monitor = opts.monitor;
            self.windows.insert(id, e);
            return vec![];
        }
        let ws = opts.workspace.filter(|w| *w < WORKSPACE_COUNT).unwrap_or(self.active);
        let mut e = WindowEntry::new(Kind::Managed, None);
        e.fullscreen = opts.fullscreen;
        e.monitor = opts.monitor;
        self.windows.insert(id, e);
        self.attach(id, ws, opts.float);
        if self.workspaces[ws].visible {
            self.place_fx(ws)
        } else {
            vec![Effect::Hide(id)]
        }
    }

    pub fn remove_window(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![] };
        self.minimized_order.retain(|&m| m != id);
        let ws = entry.workspace;
        if ws.is_some() {
            self.detach(id);
        }
        self.windows.remove(&id);
        let Some(ws) = ws else { return vec![] };
        let mut fx = Vec::new();
        if self.workspaces[ws].visible {
            fx = self.place_fx(ws);
            if ws == self.active {
                if let Some(f) = self.workspaces[ws].focused {
                    fx.push(Effect::Focus(f));
                }
            }
        }
        fx
    }

    /// El sistema informa que una ventana tomó el foco.
    pub fn note_focus(&mut self, id: WindowId) {
        self.foreground_excluded = self.windows.get(&id).is_some_and(|e| e.kind == Kind::Excluded);
        let Some(ws) = self.workspace_of(id) else { return };
        if self.windows_in(ws).contains(&id) {
            self.workspaces[ws].focused = Some(id);
            if self.workspaces[ws].visible {
                self.set_active(ws);
            }
        }
    }

    /// Minimizar o restaurar según informa el sistema (§7: la minimizada sale del layout).
    pub fn set_minimized(&mut self, id: WindowId, minimized: bool) -> Vec<Effect> {
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![] };
        let Some(ws) = entry.workspace else {
            self.windows.get_mut(&id).unwrap().minimized = minimized;
            return vec![];
        };
        if entry.minimized == minimized {
            return vec![];
        }
        if minimized {
            let slot = self.workspaces[ws].layout.group_index_of(id);
            let area = self.area(ws);
            let settings = self.settings.layout;
            self.workspaces[ws].layout.remove_window(id, &area, &settings);
            let e = self.windows.get_mut(&id).unwrap();
            e.minimized = true;
            e.slot = slot;
            self.minimized_order.push(id);
            if self.workspaces[ws].focused == Some(id) {
                self.workspaces[ws].focused = None;
                self.workspaces[ws].focused = self.pick_focus(ws);
            }
        } else {
            self.windows.get_mut(&id).unwrap().minimized = false;
            self.minimized_order.retain(|&m| m != id);
            if entry.kind == Kind::Managed {
                self.attach(id, ws, false);
            } else {
                self.workspaces[ws].focused = Some(id);
            }
        }
        if self.workspaces[ws].visible { self.place_fx(ws) } else { vec![] }
    }

    /// Aplica una configuración nueva (recarga en caliente): layout, ejes y tamaño flotante.
    /// Los workspaces conservan monitor y ventanas; las ligaduras solo valen para arranques nuevos.
    pub fn update_settings(&mut self, settings: Settings) -> Vec<Effect> {
        self.settings = settings;
        self.ensure_visibility();
        for i in 0..WORKSPACE_COUNT {
            let rect = self.float_rect_for(i);
            for id in self.workspaces[i].floating.clone() {
                self.windows.get_mut(&id).unwrap().float_rect = Some(rect);
            }
        }
        self.relayout_visible()
    }

    /// El usuario soltó la ventana tras arrastrarla: se recoloca (§12 Fase 1).
    pub fn relayout(&mut self) -> Vec<Effect> {
        self.relayout_visible()
    }

    // ---------- workspaces ----------

    pub fn switch_workspace(&mut self, ws: usize) -> Vec<Effect> {
        if ws >= WORKSPACE_COUNT {
            return vec![];
        }
        let mut fx = Vec::new();
        if !self.workspaces[ws].visible && self.settings.bindings[ws].is_none() {
            // Un workspace sin ligar se abre en el monitor donde estás.
            let mon = self.workspaces[self.active].monitor;
            self.workspaces[ws].monitor = mon;
            let axis = self.axis_for_id(mon);
            self.workspaces[ws].layout.set_axis(axis);
        }
        if !self.workspaces[ws].visible {
            let mon = self.workspaces[ws].monitor;
            let mut out = Vec::new();
            if let Some(cur) = self.visible_workspace_on(mon) {
                out = self.windows_in(cur);
                fx.extend(self.hide_fx(cur));
                self.workspaces[cur].visible = false;
            }
            self.workspaces[ws].visible = true;
            fx.extend(self.show_fx(ws));
            let inn = self.windows_in(ws);
            if self.settings.workspace_transition && !(out.is_empty() && inn.is_empty()) {
                self.transition = Some(Transition { area: self.area(ws), out, inn });
            }
        }
        self.set_active(ws);
        let f = self.pick_focus(ws);
        self.workspaces[ws].focused = f;
        if let Some(f) = f {
            fx.push(Effect::Focus(f));
        }
        fx
    }

    /// La transición anotada por el último cambio de workspace (una sola vez).
    pub fn take_transition(&mut self) -> Option<Transition> {
        self.transition.take()
    }

    pub fn move_focused_to_workspace(&mut self, ws: usize, follow: bool) -> Vec<Effect> {
        let Some(id) = self.focused() else { return vec![] };
        self.move_window_to_workspace(id, ws, follow)
    }

    /// Lleva una ventana a otro workspace sin cambiar de workspace ni enfocar nada (aplicaciones de arranque ya abiertas).
    pub fn move_window_quietly(&mut self, id: WindowId, ws: usize) -> Vec<Effect> {
        self.move_window_inner(id, ws, false, false)
    }

    fn move_window_to_workspace(&mut self, id: WindowId, ws: usize, follow: bool) -> Vec<Effect> {
        self.move_window_inner(id, ws, follow, true)
    }

    /// `refocus_source`: al sacar la ventana del workspace activo, enfocar la que queda allí. Quien se la lleva
    /// consigo (cruce de monitor) lo desactiva: un segundo `Focus` a otra ventana podría ganarle al de la movida.
    fn move_window_inner(&mut self, id: WindowId, ws: usize, follow: bool, refocus_source: bool) -> Vec<Effect> {
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![] };
        let Some(src) = entry.workspace else { return vec![] };
        if ws >= WORKSPACE_COUNT || ws == src || entry.kind == Kind::Excluded {
            return vec![];
        }
        self.detach(id);
        self.attach(id, ws, entry.kind == Kind::Floating);
        let mut fx = Vec::new();
        if self.workspaces[src].visible {
            fx.extend(self.place_fx(src));
        }
        if self.workspaces[ws].visible {
            fx.extend(self.place_fx(ws));
        } else {
            fx.push(Effect::Hide(id));
        }
        if follow {
            fx.extend(self.switch_workspace(ws));
        } else if src == self.active && refocus_source {
            if let Some(f) = self.workspaces[src].focused {
                fx.push(Effect::Focus(f));
            }
        }
        fx
    }

    fn cycle_workspace(&mut self, step: isize) -> Vec<Effect> {
        let mon = self.workspaces[self.active].monitor;
        let candidates: Vec<usize> = (0..WORKSPACE_COUNT)
            .filter(|&i| {
                self.workspaces[i].monitor == mon
                    && (i == self.active || self.workspaces[i].visible || !self.windows_in(i).is_empty())
            })
            .collect();
        let Some(pos) = candidates.iter().position(|&i| i == self.active) else { return vec![] };
        let next = candidates[(pos as isize + step).rem_euclid(candidates.len() as isize) as usize];
        self.switch_workspace(next)
    }

    /// Mueve el workspace activo al monitor de esa dirección, conservando su estructura (§13).
    pub fn move_workspace_dir(&mut self, dir: Dir) -> Vec<Effect> {
        let ws = self.active;
        let src = self.workspaces[ws].monitor;
        let Some(tgt) = self.adjacent_monitor(src, dir) else { return vec![] };
        let repl = (0..WORKSPACE_COUNT)
            .filter(|&i| i != ws && self.workspaces[i].monitor == src)
            .min()
            .or_else(|| {
                (0..WORKSPACE_COUNT).find(|&i| i != ws && !self.workspaces[i].visible && self.windows_in(i).is_empty())
            });
        let Some(repl) = repl else { return vec![] };
        let mut fx = Vec::new();
        if let Some(old) = self.visible_workspace_on(tgt) {
            fx.extend(self.hide_fx(old));
            self.workspaces[old].visible = false;
        }
        self.workspaces[ws].monitor = tgt;
        let tgt_axis = self.axis_for_id(tgt);
        self.workspaces[ws].layout.set_axis(tgt_axis);
        self.workspaces[repl].monitor = src;
        let src_axis = self.axis_for_id(src);
        self.workspaces[repl].layout.set_axis(src_axis);
        let repl_was_visible = self.workspaces[repl].visible;
        self.workspaces[repl].visible = true;
        if !repl_was_visible {
            fx.extend(self.show_fx(repl));
        } else {
            fx.extend(self.place_fx(repl));
        }
        fx.extend(self.place_fx(ws));
        if let Some(f) = self.workspaces[ws].focused {
            fx.push(Effect::Focus(f));
        }
        fx
    }

    /// Cambia la lista de monitores (conectar/desconectar). Los workspaces huérfanos pasan al principal
    /// y las ventanas de uno que estaba visible se reubican en el workspace visible del principal.
    pub fn set_monitors(&mut self, mut monitors: Vec<Monitor>) -> Vec<Effect> {
        if monitors.is_empty() {
            return vec![];
        }
        if !monitors.iter().any(|m| m.primary) {
            monitors[0].primary = true;
        }
        let before: Vec<bool> = self.workspaces.iter().map(|w| w.visible).collect();
        let hidden_before: Vec<Vec<WindowId>> = (0..WORKSPACE_COUNT).map(|i| self.windows_in(i)).collect();
        self.monitors = monitors;
        let primary = self.primary_id();

        let orphans: Vec<usize> =
            (0..WORKSPACE_COUNT).filter(|&i| !self.monitors.iter().any(|m| m.id == self.workspaces[i].monitor)).collect();
        for &i in &orphans {
            self.workspaces[i].monitor = primary;
        }
        // Reconectar un monitor devuelve a él los workspaces ligados que estén ocultos.
        for i in 0..WORKSPACE_COUNT {
            if let Some(m) = self.settings.bindings[i].and_then(|b| self.monitors.get(b)) {
                if self.workspaces[i].monitor != m.id && !self.workspaces[i].visible {
                    self.workspaces[i].monitor = m.id;
                }
            }
        }
        // Un workspace huérfano que estaba visible se funde en el visible del principal.
        let target = self.visible_workspace_on(primary).filter(|t| !orphans.contains(t));
        for &i in &orphans {
            if !self.workspaces[i].visible {
                continue;
            }
            self.workspaces[i].visible = false;
            let Some(t) = target.or_else(|| (0..WORKSPACE_COUNT).find(|&k| !orphans.contains(&k) && self.workspaces[k].monitor == primary))
            else {
                self.workspaces[i].visible = true;
                continue;
            };
            let ids: Vec<WindowId> = self.windows.iter().filter(|(_, e)| e.workspace == Some(i)).map(|(id, _)| *id).collect();
            for id in ids {
                if self.windows[&id].minimized {
                    self.windows.get_mut(&id).unwrap().workspace = Some(t);
                    continue;
                }
                let floating = self.windows[&id].kind == Kind::Floating;
                self.detach(id);
                self.attach(id, t, floating);
            }
            if !self.workspaces[t].visible {
                self.workspaces[t].visible = true;
            }
        }
        self.ensure_visibility();
        if !self.workspaces[self.active].visible && !self.workspaces.iter().any(|w| w.visible && w.monitor == self.workspaces[self.active].monitor) {
            self.active = self.visible_workspace_on(primary).unwrap_or(0);
        }
        // Las ventanas flotantes se recentran en su monitor.
        for i in 0..WORKSPACE_COUNT {
            let rect = self.float_rect_for(i);
            for id in self.workspaces[i].floating.clone() {
                self.windows.get_mut(&id).unwrap().float_rect = Some(rect);
            }
        }
        let mut fx = Vec::new();
        for i in 0..WORKSPACE_COUNT {
            let was = before[i];
            let now = self.workspaces[i].visible;
            if was && !now {
                fx.extend(hidden_before[i].iter().filter(|id| self.windows_in(i).contains(id)).map(|&id| Effect::Hide(id)));
                fx.extend(self.windows_in(i).into_iter().map(Effect::Hide));
            } else if !was && now {
                fx.extend(self.show_fx(i));
            } else if now {
                fx.extend(self.place_fx(i));
            }
        }
        fx
    }

    // ---------- pantalla completa y exclusiones (§6, §7) ----------

    /// La ventana entra o sale de pantalla completa (§7: sale del layout; al volver regresa a su columna).
    /// `user` indica que la pidió el atajo y por tanto la app la coloca cubriendo el monitor.
    pub fn set_fullscreen(&mut self, id: WindowId, fullscreen: bool, monitor: Option<MonitorId>, user: bool) -> Vec<Effect> {
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![] };
        {
            let e = self.windows.get_mut(&id).unwrap();
            if monitor.is_some() {
                e.monitor = monitor;
            }
            if e.fullscreen == fullscreen {
                return vec![];
            }
            e.fullscreen = fullscreen;
            e.user_fullscreen = fullscreen && user;
        }
        let Some(ws) = entry.workspace else { return vec![] };
        if entry.kind == Kind::Excluded {
            return vec![];
        }
        let covers = self.monitor_of(ws).bounds;
        let mut fx = Vec::new();
        match entry.kind {
            Kind::Managed if fullscreen => {
                if !entry.minimized {
                    let slot = self.workspaces[ws].layout.group_index_of(id);
                    let area = self.area(ws);
                    let settings = self.settings.layout;
                    self.workspaces[ws].layout.remove_window(id, &area, &settings);
                    self.windows.get_mut(&id).unwrap().slot = slot;
                }
                if self.workspaces[ws].visible {
                    fx.extend(self.place_fx(ws));
                    if user {
                        fx.push(Effect::Place { id, rect: covers });
                    }
                }
            }
            Kind::Managed => {
                if !entry.minimized {
                    self.attach(id, ws, false);
                }
                if self.workspaces[ws].visible {
                    fx.extend(self.place_fx(ws));
                }
            }
            Kind::Floating => {
                if self.workspaces[ws].visible {
                    if fullscreen && user {
                        fx.push(Effect::Place { id, rect: covers });
                    } else if !fullscreen {
                        fx.extend(self.place_fx(ws));
                    }
                }
            }
            Kind::Excluded => {}
        }
        fx
    }

    /// Alternar pantalla completa de la ventana enfocada (alt+f).
    fn toggle_fullscreen(&mut self) -> Vec<Effect> {
        let ws = self.active;
        let Some(id) = self.workspaces[ws].focused else { return vec![] };
        let Some(entry) = self.windows.get(&id) else { return vec![] };
        let (fs, mon) = (!entry.fullscreen, self.workspaces[ws].monitor);
        self.set_fullscreen(id, fs, Some(mon), true)
    }

    /// Marca o desmarca una ventana como excluida (§7: Gestionada → Excluida "se queda donde está").
    pub fn set_excluded(&mut self, id: WindowId, excluded: bool) -> Vec<Effect> {
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![] };
        if (entry.kind == Kind::Excluded) == excluded {
            return vec![];
        }
        if excluded {
            let ws = self.detach(id);
            let e = self.windows.get_mut(&id).unwrap();
            e.kind = Kind::Excluded;
            e.workspace = None;
            e.slot = None;
            e.float_rect = None;
            e.user_fullscreen = false;
            self.minimized_order.retain(|&m| m != id);
            match ws {
                Some(ws) if self.workspaces[ws].visible => self.place_fx(ws),
                _ => vec![],
            }
        } else {
            let ws = self.active;
            let e = self.windows.get_mut(&id).unwrap();
            e.kind = Kind::Managed;
            self.attach(id, ws, false);
            if self.workspaces[ws].visible {
                self.place_fx(ws)
            } else {
                vec![]
            }
        }
    }

    /// Ventanas que el sistema entregó como excluidas (para limpiar sus entradas al cerrarse).
    pub fn is_excluded(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|e| e.kind == Kind::Excluded)
    }

    /// Ventanas seguidas que están (o creen estar) en pantalla completa y se pueden revisar.
    /// Ventanas que deberían verse ahora mismo: las del mosaico (o flotantes) en un workspace visible y sin minimizar.
    /// Si una de ellas ya no es visible de verdad, ocupa un hueco vacío en pantalla.
    pub fn shown_windows(&self) -> Vec<WindowId> {
        let mut v: Vec<_> = self
            .windows
            .iter()
            .filter(|(_, e)| !e.minimized && e.workspace.is_some_and(|ws| self.workspaces[ws].visible))
            .map(|(id, _)| *id)
            .collect();
        v.sort();
        v
    }

    pub fn tracked_windows(&self) -> Vec<(WindowId, bool)> {
        let mut v: Vec<_> = self.windows.iter().map(|(id, e)| (*id, e.user_fullscreen)).collect();
        v.sort();
        v
    }

    /// Visibilidad de la barra por monitor (§6.4): se oculta si cualquier ventana, gestionada o excluida,
    /// está en pantalla completa en ese monitor. Devuelve solo los cambios desde la última llamada.
    pub fn sync_bar(&mut self) -> Vec<Effect> {
        let mut desired: HashMap<MonitorId, bool> = self.monitors.iter().map(|m| (m.id, true)).collect();
        for e in self.windows.values().filter(|e| e.fullscreen) {
            let mon = match e.kind {
                Kind::Excluded => e.monitor,
                _ => e.workspace.filter(|&ws| self.workspaces[ws].visible).map(|ws| self.workspaces[ws].monitor),
            };
            if let Some(m) = mon {
                desired.insert(m, false);
            }
        }
        let mut fx = Vec::new();
        let mut ids: Vec<MonitorId> = desired.keys().copied().collect();
        ids.sort();
        for m in ids {
            let now = desired[&m];
            let before = self.bar_state.get(&m).copied().unwrap_or(true);
            if now != before {
                fx.push(Effect::SetBar { monitor: m, visible: now });
            }
            self.bar_state.insert(m, now);
        }
        self.bar_state.retain(|m, _| desired.contains_key(m));
        fx
    }

    // ---------- acciones sobre la ventana enfocada ----------

    pub fn focus_dir(&mut self, dir: Dir) -> Vec<Effect> {
        let ws = self.active;
        let rects = self.workspaces[ws].layout.compute(&self.area(ws), &self.settings.layout);
        if let Some(id) = self.workspaces[ws].focused {
            if let Some(nb) = focus_neighbor(id, dir, &rects) {
                self.workspaces[ws].focused = Some(nb);
                return vec![Effect::Focus(nb)];
            }
        }
        // Sin vecino: salta al monitor de esa dirección.
        let Some(mon) = self.adjacent_monitor(self.workspaces[ws].monitor, dir) else { return vec![] };
        let Some(target) = self.visible_workspace_on(mon) else { return vec![] };
        self.set_active(target);
        match self.pick_focus(target) {
            Some(f) => {
                self.workspaces[target].focused = Some(f);
                vec![Effect::Focus(f)]
            }
            None => vec![],
        }
    }

    /// Enfoca una ventana concreta: cambia al workspace donde está (si no se ve), la restaura si estaba
    /// minimizada y le da el foco. Sirve a la lista de "apps abiertas" (§9.3).
    pub fn focus_window(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![Effect::Focus(id)] };
        let Some(ws) = entry.workspace else {
            // Excluida: vive donde Windows la puso; solo se le da el foco.
            return vec![Effect::Focus(id)];
        };
        let mut fx = Vec::new();
        if !self.workspaces[ws].visible || ws != self.active {
            fx.extend(self.switch_workspace(ws));
        }
        if entry.minimized {
            fx.push(Effect::Restore(id));
        }
        self.workspaces[ws].focused = Some(id);
        fx.push(Effect::Focus(id));
        fx
    }

    pub fn cycle_focus(&mut self) -> Vec<Effect> {
        let ws = self.active;
        let list = self.windows_in(ws);
        if list.is_empty() {
            return vec![];
        }
        let pos = self.workspaces[ws].focused.and_then(|f| list.iter().position(|&w| w == f)).unwrap_or(list.len() - 1);
        let next = list[(pos + 1) % list.len()];
        self.workspaces[ws].focused = Some(next);
        vec![Effect::Focus(next)]
    }

    pub fn move_dir(&mut self, dir: Dir) -> Vec<Effect> {
        let ws = self.active;
        let Some(id) = self.workspaces[ws].focused else { return vec![] };
        if self.windows.get(&id).map(|e| e.kind) != Some(Kind::Managed) {
            return vec![];
        }
        let area = self.area(ws);
        let settings = self.settings.layout;
        if self.workspaces[ws].layout.move_window(id, dir, &area, &settings, false) {
            return self.place_fx(ws);
        }
        // Tope de la estructura: pasa al workspace visible del monitor vecino.
        let target = self
            .adjacent_monitor(self.workspaces[ws].monitor, dir)
            .and_then(|mon| self.visible_workspace_on(mon));
        let Some(target) = target else {
            // Sin monitor hacia ese lado, la ventana se coloca junto a las demás de su monitor.
            if self.workspaces[ws].layout.move_window(id, dir, &area, &settings, true) {
                return self.place_fx(ws);
            }
            return vec![];
        };
        let mut fx = self.move_window_inner(id, target, false, false);
        self.set_active(target);
        self.workspaces[target].focused = Some(id);
        fx.push(Effect::Focus(id));
        fx
    }

    pub fn resize_focused(&mut self, o: Orientation, percent: i8) -> Vec<Effect> {
        let ws = self.active;
        let Some(id) = self.workspaces[ws].focused else { return vec![] };
        let area = self.area(ws);
        let settings = self.settings.layout;
        if self.workspaces[ws].layout.resize(id, o, percent as f64 / 100.0, &area, &settings) {
            self.place_fx(ws)
        } else {
            vec![]
        }
    }

    pub fn toggle_stack(&mut self) -> Vec<Effect> {
        let ws = self.active;
        let Some(id) = self.workspaces[ws].focused else { return vec![] };
        if self.workspaces[ws].layout.toggle_stack(id) {
            self.place_fx(ws)
        } else {
            vec![]
        }
    }

    /// Alternar tiling/flotante (alt+t, alt+shift+space). Siempre centrada: no se conoce el tamaño previo.
    pub fn toggle_floating(&mut self) -> Vec<Effect> {
        let ws = self.active;
        let Some(id) = self.workspaces[ws].focused else { return vec![] };
        let Some(entry) = self.windows.get(&id).cloned() else { return vec![] };
        match entry.kind {
            Kind::Excluded => vec![],
            Kind::Managed => {
                self.detach(id);
                self.attach(id, ws, true);
                self.workspaces[ws].focused = Some(id);
                self.place_fx(ws)
            }
            Kind::Floating => {
                self.workspaces[ws].floating.retain(|&f| f != id);
                let slot = entry.slot;
                self.windows.get_mut(&id).unwrap().slot = slot;
                self.attach(id, ws, false);
                self.workspaces[ws].focused = Some(id);
                self.place_fx(ws)
            }
        }
    }

    pub fn toggle_minimize(&mut self) -> Vec<Effect> {
        let ws = self.active;
        match self.workspaces[ws].focused {
            Some(id) => vec![Effect::Minimize(id)],
            None => match self.minimized_order.last() {
                Some(&id) => vec![Effect::Restore(id)],
                None => vec![],
            },
        }
    }

    pub fn close_focused(&mut self) -> Vec<Effect> {
        self.focused().map(|id| vec![Effect::Close(id)]).unwrap_or_default()
    }

    /// Ejecuta una acción de ventana/workspace. Las acciones del sistema las atiende el controlador.
    pub fn apply_action(&mut self, action: &Action) -> Vec<Effect> {
        if self.foreground_excluded
            && matches!(
                action,
                Action::MoveDir(_)
                    | Action::Resize { .. }
                    | Action::MoveToWorkspace(_)
                    | Action::ToggleStack
                    | Action::ToggleFloating
                    | Action::ToggleFloatingCentered
                    | Action::ToggleFullscreen
                    | Action::ToggleMinimize
                    | Action::CloseWindow
            )
        {
            // §6.2: una ventana excluida no recibe atajos de posición; tampoco se actúa sobre otra a sus espaldas.
            return vec![];
        }
        match *action {
            Action::FocusDir(d) => self.focus_dir(d),
            Action::MoveDir(d) => self.move_dir(d),
            Action::Resize { orientation, percent } => self.resize_focused(orientation, percent),
            Action::FocusWorkspace(n) => self.switch_workspace(n as usize - 1),
            Action::MoveToWorkspace(n) => self.move_focused_to_workspace(n as usize - 1, true),
            Action::NextWorkspace => self.cycle_workspace(1),
            Action::PrevWorkspace => self.cycle_workspace(-1),
            Action::RecentWorkspace => match self.recent {
                Some(r) => self.switch_workspace(r),
                None => vec![],
            },
            Action::MoveWorkspaceDir(d) => self.move_workspace_dir(d),
            Action::ToggleStack => self.toggle_stack(),
            Action::ToggleFloating | Action::ToggleFloatingCentered => self.toggle_floating(),
            Action::ToggleMinimize => self.toggle_minimize(),
            Action::CloseWindow => self.close_focused(),
            Action::CycleFocus => self.cycle_focus(),
            Action::Redraw => self.relayout(),
            Action::ToggleFullscreen => self.toggle_fullscreen(),
            // El resto es del controlador.
            Action::EnterResizeMode
            | Action::ExitResizeMode
            | Action::OpenTerminal
            | Action::TogglePause
            | Action::TogglePanel
            | Action::ReloadConfig
            | Action::Exit => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: MonitorId, x: i32, y: i32, w: i32, h: i32, primary: bool) -> Monitor {
        Monitor::new(id, Rect::new(x, y, w, h), primary)
    }

    fn one() -> Desktop {
        Desktop::new(Settings::default(), vec![mon(0, 0, 0, 3000, 1000, true)])
    }

    /// Principal horizontal (0) + vertical (1) a la derecha. Workspaces 1-4 -> 0, 5-7 -> 1.
    fn two() -> Desktop {
        let mut s = Settings::default();
        s.bindings = [Some(0), Some(0), Some(0), Some(0), Some(1), Some(1), Some(1), None, None];
        Desktop::new(s, vec![mon(0, 0, 0, 3000, 1000, true), mon(1, 3000, 0, 1000, 3000, false)])
    }

    fn placed(fx: &[Effect]) -> Vec<(WindowId, Rect)> {
        fx.iter().filter_map(|e| if let Effect::Place { id, rect } = e { Some((*id, *rect)) } else { None }).collect()
    }

    fn rect_in(d: &Desktop, ws: usize, id: WindowId) -> Rect {
        placed(&d.place_fx(ws)).into_iter().find(|(w, _)| *w == id).unwrap().1
    }

    #[test]
    fn cambiar_de_workspace_anota_salientes_y_entrantes_una_sola_vez() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        d.move_focused_to_workspace(1, false); // la enfocada (2) pasa al workspace 2, que queda oculto
        d.switch_workspace(1);
        let t = d.take_transition().expect("hay transición");
        assert_eq!(t.out, vec![1]);
        assert_eq!(t.inn, vec![2]);
        assert_eq!(t.area, Rect::new(0, 0, 3000, 1000));
        assert!(d.take_transition().is_none(), "se recoge una sola vez");
        // Cambiar al que ya está a la vista no anima.
        d.switch_workspace(1);
        assert!(d.take_transition().is_none());
    }

    #[test]
    fn sin_ventanas_en_ninguno_de_los_dos_no_hay_transicion() {
        let mut d = one();
        d.switch_workspace(3);
        assert!(d.take_transition().is_none());
    }

    #[test]
    fn la_transicion_se_puede_apagar() {
        let mut s = Settings::default();
        s.workspace_transition = false;
        let mut d = Desktop::new(s, vec![mon(0, 0, 0, 3000, 1000, true)]);
        d.add_window(1);
        d.switch_workspace(1);
        assert!(d.take_transition().is_none());
    }

    #[test]
    fn dos_ventanas_se_reparten_el_ancho() {
        let mut d = one();
        d.add_window(1);
        let fx = d.add_window(2);
        let p = placed(&fx);
        assert_eq!(p[0].1.w + p[1].1.w, 3000);
        assert!(p[0].1.x < p[1].1.x);
    }

    #[test]
    fn cerrar_reparte_de_nuevo() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        let fx = d.remove_window(2);
        assert_eq!(placed(&fx), vec![(1, Rect::new(0, 0, 3000, 1000))]);
        assert!(fx.contains(&Effect::Focus(1)));
    }

    #[test]
    fn cerrar_la_ultima_deja_el_workspace_vacio() {
        let mut d = one();
        d.add_window(1);
        let fx = d.remove_window(1);
        assert!(placed(&fx).is_empty());
        assert!(d.windows_in(0).is_empty());
        assert_eq!(d.focused(), None);
        assert!(d.remove_window(1).is_empty(), "cerrar dos veces no falla");
    }

    // ----- Workspaces -----

    #[test]
    fn cambiar_de_workspace_oculta_y_muestra() {
        let mut d = one();
        d.add_window(1);
        d.switch_workspace(1);
        d.add_window(2);
        let fx = d.switch_workspace(0);
        assert!(fx.contains(&Effect::Hide(2)));
        assert!(fx.contains(&Effect::Show(1)));
        assert_eq!(d.focused(), Some(1));
        assert_eq!(d.recent_workspace(), Some(1));
    }

    #[test]
    fn workspace_oculto_conserva_su_estructura() {
        let mut d = one();
        for i in 1..=3 {
            d.add_window(i);
        }
        d.resize_focused(Orientation::X, 10);
        let before = d.workspace(0).layout.clone();
        d.switch_workspace(1);
        d.switch_workspace(0);
        assert_eq!(d.workspace(0).layout, before);
    }

    #[test]
    fn mover_ventana_a_otro_workspace() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        let fx = d.move_focused_to_workspace(3, true);
        assert_eq!(d.active_workspace(), 3);
        assert_eq!(d.workspace_of(2), Some(3));
        assert_eq!(d.workspace_of(1), Some(0));
        assert!(fx.contains(&Effect::Focus(2)));
        assert!(!d.workspace(0).layout.contains(2), "sale de la estructura de origen");
        assert!(d.workspace(3).layout.contains(2), "entra en la del destino");
    }

    #[test]
    fn mover_sin_seguir_deja_el_foco_aqui() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        let fx = d.move_focused_to_workspace(3, false);
        assert_eq!(d.active_workspace(), 0);
        assert!(fx.contains(&Effect::Hide(2)));
        assert_eq!(placed(&fx), vec![(1, Rect::new(0, 0, 3000, 1000))]);
        assert_eq!(d.focused(), Some(1));
    }

    #[test]
    fn ventana_duplicada_se_ignora() {
        let mut d = one();
        d.add_window(1);
        assert!(d.add_window(1).is_empty());
    }

    #[test]
    fn ciclar_foco_da_la_vuelta() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        assert_eq!(d.cycle_focus(), vec![Effect::Focus(1)]);
        assert_eq!(d.cycle_focus(), vec![Effect::Focus(2)]);
    }

    #[test]
    fn workspace_reciente_y_siguiente_activo() {
        let mut d = one();
        d.add_window(1);
        d.switch_workspace(2);
        d.add_window(2);
        d.switch_workspace(0);
        d.apply_action(&Action::RecentWorkspace);
        assert_eq!(d.active_workspace(), 2);
        d.apply_action(&Action::NextWorkspace);
        assert_eq!(d.active_workspace(), 0, "da la vuelta entre los activos (1 y 3)");
        d.apply_action(&Action::PrevWorkspace);
        assert_eq!(d.active_workspace(), 2);
    }

    // ----- Caso límite 5.6 -----

    #[test]
    fn sin_espacio_la_ventana_nueva_es_flotante_centrada() {
        let mut d = one();
        for i in 1..=15 {
            d.add_window(i);
        }
        let before = d.workspace(0).layout.compute(&d.area(0), &d.settings.layout);
        let fx = d.add_window(99);
        assert_eq!(d.entry(99).unwrap().kind, Kind::Floating);
        assert!(!d.workspace(0).layout.contains(99));
        let r = placed(&fx).into_iter().find(|(w, _)| *w == 99).unwrap().1;
        let (cx, cy) = r.center();
        assert_eq!((cx, cy), (1500, 500), "centrada");
        // Las demás columnas no se movieron.
        let after = d.workspace(0).layout.compute(&d.area(0), &d.settings.layout);
        assert_eq!(before, after);
        // Y al cerrarla no deja rastro.
        d.remove_window(99);
        assert!(d.workspace(0).floating.is_empty());
    }

    // ----- Estados (§7) -----

    #[test]
    fn gestionada_a_flotante_y_de_vuelta_a_su_columna() {
        let mut d = one();
        for i in 1..=3 {
            d.add_window(i);
        }
        d.note_focus(2);
        let fx = d.toggle_floating();
        assert_eq!(d.entry(2).unwrap().kind, Kind::Floating);
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![3]]);
        assert!(placed(&fx).iter().any(|(id, _)| *id == 2));
        d.toggle_floating();
        assert_eq!(d.entry(2).unwrap().kind, Kind::Managed);
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![2], vec![3]], "vuelve a su posición");
    }

    #[test]
    fn flotante_vuelve_al_final_si_su_posicion_ya_no_existe() {
        let mut d = one();
        for i in 1..=3 {
            d.add_window(i);
        }
        d.note_focus(3);
        d.toggle_floating();
        d.remove_window(2);
        d.remove_window(1);
        d.toggle_floating();
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![3]]);
    }

    #[test]
    fn minimizada_sale_del_layout_y_vuelve_a_su_posicion() {
        let mut d = one();
        for i in 1..=3 {
            d.add_window(i);
        }
        let fx = d.set_minimized(2, true);
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![3]]);
        assert!(placed(&fx).iter().all(|(id, _)| *id != 2));
        d.set_minimized(2, false);
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn cerrar_una_minimizada_no_deja_estado() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        d.set_minimized(2, true);
        d.remove_window(2);
        assert!(d.entry(2).is_none());
        assert_eq!(d.minimized_order, Vec::<WindowId>::new());
    }

    // ----- Monitores (§13) -----

    #[test]
    fn dos_monitores_cada_uno_calcula_su_layout() {
        let mut d = two();
        d.add_window(1);
        d.add_window(2);
        d.switch_workspace(4); // workspace 5, en el monitor vertical
        for i in 3..=5 {
            d.add_window(i);
        }
        assert_eq!(d.active_workspace(), 4);
        // Monitor vertical con eje en filas: tres filas que se reparten el alto.
        let rects: Vec<Rect> = (3..=5).map(|i| rect_in(&d, 4, i)).collect();
        assert!(rects.iter().all(|r| r.w == 1000 && (r.h - 1000).abs() <= 1), "{rects:?}");
        assert!(rects[0].y < rects[1].y);
        // El monitor horizontal sigue repartiendo columnas por su cuenta.
        let left = [rect_in(&d, 0, 1), rect_in(&d, 0, 2)];
        assert!(left.iter().all(|r| r.h == 1000 && (r.w - 1500).abs() <= 1));
        // Ambos workspaces son visibles a la vez, en monitores distintos.
        assert!(d.workspace(0).visible && d.workspace(4).visible);
        assert_ne!(d.workspace(0).monitor, d.workspace(4).monitor);
    }

    #[test]
    fn el_eje_del_monitor_vertical_es_configurable() {
        let mut s = Settings::default();
        s.vertical_axis = Axis::Columns;
        let mut d = Desktop::new(s, vec![mon(0, 0, 0, 1000, 3000, true)]);
        d.add_window(1);
        d.add_window(2);
        let r = rect_in(&d, 0, 2);
        assert_eq!(r.x, 500, "con eje en columnas se reparte el ancho");
    }

    #[test]
    fn mover_workspace_a_otro_monitor_conserva_su_estructura() {
        let mut d = two();
        for i in 1..=3 {
            d.add_window(i);
        }
        d.apply_action(&Action::Resize { orientation: Orientation::X, percent: 10 });
        let sizes_before = d.workspace(0).layout.group_sizes();
        let groups_before = d.workspace(0).layout.group_windows();
        let fx = d.move_workspace_dir(Dir::Right);
        assert_eq!(d.workspace(0).monitor, 1);
        assert_eq!(d.workspace(0).layout.group_windows(), groups_before);
        assert_eq!(d.workspace(0).layout.group_sizes(), sizes_before);
        // En el monitor vertical el eje es filas: todas las ventanas caben en su área.
        let area = d.area(0);
        for (_, r) in placed(&fx) {
            if d.windows_in(0).iter().any(|_| true) {
                assert!(r.x >= 0);
            }
        }
        for id in 1..=3 {
            assert!(area.contains_rect(&rect_in(&d, 0, id)));
        }
        // El monitor de origen no se queda sin workspace visible.
        assert!(d.visible_workspace_on(0).is_some());
    }

    #[test]
    fn mover_workspace_sin_monitor_en_esa_direccion_no_hace_nada() {
        let mut d = two();
        d.add_window(1);
        assert!(d.move_workspace_dir(Dir::Left).is_empty());
        assert_eq!(d.workspace(0).monitor, 0);
    }

    #[test]
    fn desconectar_un_monitor_reasigna_sus_ventanas_al_principal() {
        let mut d = two();
        d.add_window(1);
        d.switch_workspace(4);
        d.add_window(2);
        d.add_window(3);
        d.switch_workspace(5); // otro workspace del monitor 1, oculto después
        d.add_window(4);
        d.switch_workspace(4);
        // Desconecta el monitor 1.
        let fx = d.set_monitors(vec![mon(0, 0, 0, 3000, 1000, true)]);
        let area = d.area(0);
        for id in [1, 2, 3] {
            let ws = d.workspace_of(id).unwrap();
            assert_eq!(d.workspace(ws).monitor, 0, "ventana {id} reasignada al principal");
            assert!(d.workspace(ws).visible, "las ventanas visibles siguen visibles");
            assert!(area.contains_rect(&rect_in(&d, ws, id)), "ventana {id} dentro del monitor principal");
        }
        // El workspace oculto también pasó al principal, sin perder la ventana.
        assert_eq!(d.workspace(5).monitor, 0);
        assert_eq!(d.workspace_of(4), Some(5));
        assert!(!d.workspace(5).visible);
        // Exactamente un workspace visible en el único monitor.
        assert_eq!((0..9).filter(|&i| d.workspace(i).visible).count(), 1);
        assert!(!fx.is_empty());
    }

    #[test]
    fn reconectar_el_monitor_recupera_los_workspaces_ligados() {
        let mut d = two();
        d.switch_workspace(5);
        d.add_window(1);
        d.switch_workspace(0);
        d.set_monitors(vec![mon(0, 0, 0, 3000, 1000, true)]);
        assert_eq!(d.workspace(5).monitor, 0);
        d.set_monitors(vec![mon(0, 0, 0, 3000, 1000, true), mon(1, 3000, 0, 1000, 3000, false)]);
        assert_eq!(d.workspace(5).monitor, 1);
        assert_eq!((0..9).filter(|&i| d.workspace(i).visible && d.workspace(i).monitor == 1).count(), 1);
    }

    #[test]
    fn foco_y_movimiento_entre_monitores() {
        let mut d = two();
        d.add_window(1);
        d.switch_workspace(4);
        d.add_window(2);
        d.switch_workspace(0);
        // Foco a la derecha: salta al monitor vertical.
        let fx = d.focus_dir(Dir::Right);
        assert_eq!(fx, vec![Effect::Focus(2)]);
        assert_eq!(d.active_workspace(), 4);
        // Mover la ventana a la izquierda desde el tope: pasa al monitor horizontal.
        d.move_dir(Dir::Left);
        assert_eq!(d.workspace_of(2), Some(0));
    }

    #[test]
    fn arriba_no_salta_a_un_monitor_que_solo_esta_al_lado() {
        // El secundario (derecha) está algo más abajo que el principal: no hay nada «arriba» ni «abajo» entre ellos.
        let mut s = Settings::default();
        s.bindings = [Some(0), Some(0), Some(0), Some(0), Some(1), Some(1), Some(1), None, None];
        let mut d = Desktop::new(s, vec![mon(0, 0, 0, 1920, 1000, true), mon(1, 1920, 200, 1920, 1000, false)]);
        d.add_window(1);
        d.switch_workspace(4);
        d.add_window(2);
        d.note_focus(2);
        for _ in 0..5 {
            d.move_dir(Dir::Up);
        }
        assert_eq!(d.workspace_of(2), Some(4), "sigue en su monitor");
        assert_eq!(d.workspace_of(1), Some(0));
    }

    #[test]
    fn intercambiar_de_sitio_en_un_solo_movimiento() {
        let mut d = two();
        d.switch_workspace(4);
        d.add_window(1); // «Spotify» arriba
        d.add_window(2); // «Discord» abajo
        d.note_focus(2);
        let y = |d: &Desktop, id| rect_in(d, 4, id).y;
        assert!(y(&d, 1) < y(&d, 2));
        d.move_dir(Dir::Up);
        assert!(y(&d, 2) < y(&d, 1), "Discord pasó arriba con una sola pulsación");
        assert_eq!(d.workspace_of(2), Some(4));
        // A la derecha no hay monitor: se coloca junto a la otra, en su misma fila.
        d.move_dir(Dir::Right);
        assert_eq!(d.workspace_of(2), Some(4));
        assert_eq!(y(&d, 1), y(&d, 2));
        assert!(rect_in(&d, 4, 2).x > rect_in(&d, 4, 1).x);
        // A la izquierda sí hay monitor: cruza.
        d.move_dir(Dir::Left);
        d.move_dir(Dir::Left);
        assert_eq!(d.workspace(d.workspace_of(2).unwrap()).monitor, 0);
    }

    #[test]
    fn cruzar_de_monitor_solo_enfoca_la_ventana_movida() {
        let mut d = two();
        d.add_window(1);
        d.add_window(2);
        d.note_focus(2);
        // La 2 está en el tope derecho del monitor horizontal: la siguiente pulsación la pasa al vertical.
        let fx = d.move_dir(Dir::Right);
        assert_eq!(d.workspace_of(2), Some(4), "pasó al workspace visible del monitor 2");
        assert_eq!(d.active_workspace(), 4);
        let focuses: Vec<_> = fx.iter().filter(|e| matches!(e, Effect::Focus(_))).collect();
        assert_eq!(focuses, vec![&Effect::Focus(2)], "no debe enfocar a la que se quedó en el monitor origen");
        // Otra pulsación en la misma dirección no arrastra a ninguna otra ventana.
        let again = d.move_dir(Dir::Right);
        assert!(again.is_empty());
        assert_eq!(d.workspace_of(1), Some(0));
        assert_eq!(d.focused(), Some(2));
    }

    #[test]
    fn mover_a_un_workspace_sigue_enfocando_lo_que_queda() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        let fx = d.move_focused_to_workspace(3, false);
        assert!(fx.contains(&Effect::Focus(1)), "alt+shift+N enfoca la ventana que queda en el origen");
    }

    #[test]
    fn acciones_de_ventana_apilar_y_alternar() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        d.apply_action(&Action::MoveDir(Dir::Down));
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1, 2]]);
        let before = rect_in(&d, 0, 2);
        d.apply_action(&Action::ToggleStack);
        let after = rect_in(&d, 0, 2);
        assert_ne!(before, after);
        assert_eq!(after.h, 1000);
    }

    #[test]
    fn cerrar_y_minimizar_producen_efectos() {
        let mut d = one();
        d.add_window(1);
        assert_eq!(d.apply_action(&Action::CloseWindow), vec![Effect::Close(1)]);
        assert_eq!(d.apply_action(&Action::ToggleMinimize), vec![Effect::Minimize(1)]);
    }

    #[test]
    fn recargar_ajustes_reacomoda_sin_perder_ventanas() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        let mut s = Settings::default();
        s.layout.gaps = crate::domain::layout::Gaps::uniform(10, 10);
        let fx = d.update_settings(s);
        let p = placed(&fx);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].1.x, 10);
        assert_eq!(d.windows_in(0), vec![1, 2]);
    }

    #[test]
    fn ventana_nueva_en_workspace_oculto_se_oculta() {
        let mut d = one();
        let fx = d.add_window_to(7, 5);
        assert_eq!(fx, vec![Effect::Hide(7)]);
        assert_eq!(d.workspace_of(7), Some(5));
    }

    // ----- Exclusiones y pantalla completa (§6, §7, §13) -----

    fn excluded(mon: Option<MonitorId>) -> TrackOptions {
        TrackOptions { excluded: true, monitor: mon, ..Default::default() }
    }

    #[test]
    fn una_ventana_excluida_no_cambia_de_posicion_ni_cuenta_para_el_reparto() {
        let mut d = one();
        d.add_window(1);
        assert!(d.track_window(99, excluded(Some(0))).is_empty(), "no se le coloca nada");
        let fx = d.add_window(2);
        let p = placed(&fx);
        assert_eq!(p.len(), 2, "solo las gestionadas entran en el reparto");
        assert!(p.iter().all(|(id, r)| *id != 99 && (r.w - 1500).abs() <= 1));
        assert!(!d.windows_in(0).contains(&99));
        assert_eq!(d.entry(99).unwrap().kind, Kind::Excluded);
        // Ningún efecto posterior toca a la excluida.
        for fx in [d.relayout(), d.switch_workspace(1), d.switch_workspace(0)] {
            assert!(fx.iter().all(|e| !matches!(e, Effect::Place { id: 99, .. } | Effect::Hide(99) | Effect::Show(99))));
        }
    }

    #[test]
    fn cerrar_una_excluida_no_deja_estado_residual() {
        let mut d = one();
        d.add_window(1);
        d.track_window(99, TrackOptions { excluded: true, fullscreen: true, monitor: Some(0), ..Default::default() });
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: false }]);
        d.remove_window(99);
        assert!(d.entry(99).is_none());
        assert_eq!(d.all_windows(), vec![1]);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: true }], "la barra reaparece");
        assert!(d.sync_bar().is_empty());
    }

    #[test]
    fn gestionada_a_excluida_sale_del_layout_y_se_queda_donde_esta() {
        let mut d = one();
        for i in 1..=3 {
            d.add_window(i);
        }
        let fx = d.set_excluded(2, true);
        let p = placed(&fx);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|(id, _)| *id != 2), "no se mueve la excluida");
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![3]]);
        assert_eq!(d.workspace_of(2), None);
        // Y de vuelta: entra al final del workspace activo.
        d.set_excluded(2, false);
        assert!(d.workspace(0).layout.contains(2));
        assert!(d.set_excluded(2, false).is_empty());
    }

    #[test]
    fn los_atajos_de_posicion_no_actuan_si_el_foco_es_una_excluida() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        d.track_window(99, excluded(Some(0)));
        d.note_focus(99);
        for a in [
            Action::MoveDir(Dir::Left),
            Action::Resize { orientation: Orientation::X, percent: 2 },
            Action::ToggleFloating,
            Action::ToggleStack,
            Action::ToggleFullscreen,
            Action::CloseWindow,
            Action::ToggleMinimize,
            Action::MoveToWorkspace(3),
        ] {
            assert!(d.apply_action(&a).is_empty(), "{a:?} no debe actuar sobre otra ventana");
        }
        // Al volver a una gestionada los atajos funcionan otra vez.
        d.note_focus(1);
        assert!(!d.apply_action(&Action::CloseWindow).is_empty());
    }

    #[test]
    fn pantalla_completa_gestionada_sale_del_layout_y_oculta_la_barra() {
        let mut d = one();
        for i in 1..=3 {
            d.add_window(i);
        }
        let fx = d.set_fullscreen(2, true, Some(0), false);
        assert_eq!(placed(&fx).len(), 2, "las otras dos se reparten el ancho");
        assert!(placed(&fx).iter().all(|(id, _)| *id != 2), "la ventana en pantalla completa no se toca");
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![3]]);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: false }]);
        assert!(d.windows_in(0).contains(&2), "sigue siendo una ventana visible del workspace");
        // Salir: vuelve a su columna y reaparece la barra.
        d.set_fullscreen(2, false, Some(0), false);
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![2], vec![3]]);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: true }]);
    }

    #[test]
    fn dos_pantallas_completas_en_monitores_distintos_ocultan_cada_barra() {
        let mut d = two();
        d.add_window(1);
        d.switch_workspace(4);
        d.add_window(2);
        d.set_fullscreen(1, true, Some(0), false);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: false }]);
        d.set_fullscreen(2, true, Some(1), false);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 1, visible: false }]);
        d.set_fullscreen(1, false, Some(0), false);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: true }], "solo reaparece la del monitor 0");
        d.set_fullscreen(2, false, Some(1), false);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 1, visible: true }]);
    }

    #[test]
    fn pantalla_completa_de_una_excluida_oculta_la_barra_de_su_monitor() {
        let mut d = two();
        d.track_window(50, TrackOptions { excluded: true, monitor: Some(1), ..Default::default() });
        assert!(d.sync_bar().is_empty());
        d.set_fullscreen(50, true, Some(1), false);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 1, visible: false }]);
    }

    #[test]
    fn pantalla_completa_en_un_workspace_oculto_no_oculta_la_barra() {
        let mut d = one();
        d.add_window(1);
        d.set_fullscreen(1, true, Some(0), false);
        d.sync_bar();
        d.switch_workspace(3);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: true }]);
        d.switch_workspace(0);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: false }]);
    }

    #[test]
    fn alt_f_coloca_la_ventana_cubriendo_el_monitor_y_alterna() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        let fx = d.apply_action(&Action::ToggleFullscreen);
        assert!(placed(&fx).contains(&(2, Rect::new(0, 0, 3000, 1000))));
        assert!(d.entry(2).unwrap().user_fullscreen);
        assert_eq!(d.sync_bar().len(), 1);
        let fx = d.apply_action(&Action::ToggleFullscreen);
        assert_eq!(placed(&fx).len(), 2);
        assert_eq!(d.workspace(0).layout.group_windows(), vec![vec![1], vec![2]]);
    }

    #[test]
    fn ventana_que_nace_en_pantalla_completa_no_entra_al_layout() {
        let mut d = one();
        d.add_window(1);
        let fx = d.track_window(7, TrackOptions { fullscreen: true, monitor: Some(0), ..Default::default() });
        assert_eq!(placed(&fx), vec![(1, Rect::new(0, 0, 3000, 1000))]);
        assert_eq!(d.sync_bar(), vec![Effect::SetBar { monitor: 0, visible: false }]);
    }

    #[test]
    fn reglas_de_ventana_workspace_y_flotante() {
        let mut d = one();
        let fx = d.track_window(5, TrackOptions { workspace: Some(2), ..Default::default() });
        assert_eq!(fx, vec![Effect::Hide(5)], "va a un workspace oculto");
        assert_eq!(d.workspace_of(5), Some(2));
        let fx = d.track_window(6, TrackOptions { float: true, ..Default::default() });
        assert_eq!(d.entry(6).unwrap().kind, Kind::Floating);
        assert_eq!(placed(&fx)[0].1.center(), (1500, 500));
    }

    // ----- Enfocar una ventana concreta (lista de apps abiertas, §9.3) -----

    #[test]
    fn enfocar_una_ventana_de_otro_workspace_cambia_de_workspace_y_la_muestra() {
        let mut d = one();
        d.add_window(1);
        d.switch_workspace(3);
        d.add_window(2);
        let fx = d.focus_window(1);
        assert_eq!(d.active_workspace(), 0);
        assert!(fx.contains(&Effect::Show(1)) && fx.contains(&Effect::Hide(2)));
        assert_eq!(fx.last(), Some(&Effect::Focus(1)));
        assert_eq!(d.focused(), Some(1));
    }

    #[test]
    fn enfocar_en_el_workspace_actual_solo_da_el_foco() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        assert_eq!(d.focus_window(1), vec![Effect::Focus(1)]);
        assert_eq!(d.focused(), Some(1));
    }

    #[test]
    fn enfocar_una_minimizada_la_restaura() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        d.set_minimized(1, true);
        let fx = d.focus_window(1);
        assert!(fx.contains(&Effect::Restore(1)) && fx.last() == Some(&Effect::Focus(1)));
    }

    #[test]
    fn enfocar_excluidas_y_desconocidas_solo_da_el_foco() {
        let mut d = one();
        d.track_window(9, excluded(Some(0)));
        assert_eq!(d.focus_window(9), vec![Effect::Focus(9)]);
        assert_eq!(d.focus_window(404), vec![Effect::Focus(404)]);
    }
}
