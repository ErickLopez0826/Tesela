//! Escritorio: monitores, 9 workspaces, estados de ventana y operaciones (§4, §5, §7).
//! Es puro: recibe hechos y devuelve `Effect`s; nunca toca Windows.

use std::collections::HashMap;

use crate::domain::actions::Action;
use crate::domain::effects::{Effect, WindowId};
use crate::domain::geometry::Rect;
use crate::domain::layout::{focus_neighbor, Axis, Dir, Layout, LayoutSettings, NoRoom, Orientation};

pub const WORKSPACE_COUNT: usize = 9;

pub type MonitorId = u32;

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub id: MonitorId,
    /// Área útil (descontando la barra, §4).
    pub work_area: Rect,
    pub primary: bool,
}

impl Monitor {
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
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout: LayoutSettings::default(),
            horizontal_axis: Axis::Columns,
            vertical_axis: Axis::Rows,
            bindings: [None; WORKSPACE_COUNT],
            float_size: (0.6, 0.7),
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
    /// Columna que ocupaba al dejar el layout, para volver a su sitio (§7).
    slot: Option<usize>,
    float_rect: Option<Rect>,
}

impl WindowEntry {
    fn new(kind: Kind, workspace: Option<usize>) -> Self {
        WindowEntry { kind, workspace, minimized: false, fullscreen: false, slot: None, float_rect: None }
    }
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
}

impl Desktop {
    pub fn new(settings: Settings, mut monitors: Vec<Monitor>) -> Self {
        if monitors.is_empty() {
            monitors.push(Monitor { id: 0, work_area: Rect::new(0, 0, 1920, 1080), primary: true });
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
        w.layout
            .windows()
            .into_iter()
            .chain(w.floating.iter().copied())
            .filter(|id| self.windows.get(id).is_some_and(|e| !e.minimized))
            .collect()
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
            if let (false, Some(rect)) = (e.minimized, e.float_rect) {
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
        let (cx, cy) = self.monitor_by_id(from).center();
        self.monitors
            .iter()
            .filter(|m| m.id != from)
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
        let minimized = self.windows[&id].minimized;
        let slot = self.windows[&id].slot;
        let kind = if minimized {
            // Minimizada: no entra en el layout; recuerda dónde volver.
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
        if self.windows.contains_key(&id) || ws >= WORKSPACE_COUNT {
            return vec![];
        }
        self.windows.insert(id, WindowEntry::new(Kind::Managed, None));
        self.attach(id, ws, false);
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
            if let Some(cur) = self.visible_workspace_on(mon) {
                fx.extend(self.hide_fx(cur));
                self.workspaces[cur].visible = false;
            }
            self.workspaces[ws].visible = true;
            fx.extend(self.show_fx(ws));
        }
        self.set_active(ws);
        let f = self.pick_focus(ws);
        self.workspaces[ws].focused = f;
        if let Some(f) = f {
            fx.push(Effect::Focus(f));
        }
        fx
    }

    pub fn move_focused_to_workspace(&mut self, ws: usize, follow: bool) -> Vec<Effect> {
        let Some(id) = self.focused() else { return vec![] };
        self.move_window_to_workspace(id, ws, follow)
    }

    fn move_window_to_workspace(&mut self, id: WindowId, ws: usize, follow: bool) -> Vec<Effect> {
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
        } else if src == self.active {
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
        if self.workspaces[ws].layout.move_window(id, dir, &area, &settings) {
            return self.place_fx(ws);
        }
        // Tope de la estructura: pasa al workspace visible del monitor vecino.
        let Some(mon) = self.adjacent_monitor(self.workspaces[ws].monitor, dir) else { return vec![] };
        let Some(target) = self.visible_workspace_on(mon) else { return vec![] };
        let mut fx = self.move_window_to_workspace(id, target, false);
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
            // Pantalla completa llega en la Fase 4; el resto es del controlador.
            Action::ToggleFullscreen
            | Action::EnterResizeMode
            | Action::ExitResizeMode
            | Action::OpenTerminal
            | Action::TogglePause
            | Action::ReloadConfig
            | Action::Exit => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: MonitorId, x: i32, y: i32, w: i32, h: i32, primary: bool) -> Monitor {
        Monitor { id, work_area: Rect::new(x, y, w, h), primary }
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
    fn acciones_de_ventana_apilar_y_alternar() {
        let mut d = one();
        d.add_window(1);
        d.add_window(2);
        d.apply_action(&Action::MoveDir(Dir::Left));
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
}
