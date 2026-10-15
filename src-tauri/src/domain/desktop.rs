//! Fase 1: escritorio mínimo con 9 workspaces y reparto en columnas iguales.
//! El motor de layout completo llega en la Fase 2 y reemplaza `columns_for`.

use crate::domain::effects::{Effect, WindowId};
use crate::domain::geometry::{split_by_weights, Rect};

pub const WORKSPACE_COUNT: usize = 9;

#[derive(Debug, Clone)]
pub struct Desktop {
    area: Rect,
    workspaces: Vec<Vec<WindowId>>,
    active: usize,
    focused: Option<WindowId>,
}

impl Desktop {
    pub fn new(area: Rect) -> Self {
        Desktop { area, workspaces: vec![Vec::new(); WORKSPACE_COUNT], active: 0, focused: None }
    }

    pub fn active_workspace(&self) -> usize {
        self.active
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    pub fn windows_in(&self, ws: usize) -> &[WindowId] {
        &self.workspaces[ws]
    }

    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        self.workspaces.iter().position(|w| w.contains(&id))
    }

    fn columns_for(&self, ids: &[WindowId]) -> Vec<Effect> {
        let widths = split_by_weights(self.area.w, &vec![1.0; ids.len()]);
        let mut x = self.area.x;
        ids.iter()
            .zip(widths)
            .map(|(&id, w)| {
                let rect = Rect::new(x, self.area.y, w, self.area.h);
                x += w;
                Effect::Place { id, rect }
            })
            .collect()
    }

    fn reflow_active(&self) -> Vec<Effect> {
        self.columns_for(&self.workspaces[self.active])
    }

    /// Una ventana nueva entra en el workspace activo.
    pub fn add_window(&mut self, id: WindowId) -> Vec<Effect> {
        if self.workspace_of(id).is_some() {
            return vec![];
        }
        self.workspaces[self.active].push(id);
        self.focused = Some(id);
        self.reflow_active()
    }

    pub fn remove_window(&mut self, id: WindowId) -> Vec<Effect> {
        let Some(ws) = self.workspace_of(id) else { return vec![] };
        self.workspaces[ws].retain(|&w| w != id);
        if self.focused == Some(id) {
            self.focused = self.workspaces[self.active].last().copied();
        }
        let mut fx = if ws == self.active { self.reflow_active() } else { vec![] };
        if let (true, Some(f)) = (ws == self.active, self.focused) {
            fx.push(Effect::Focus(f));
        }
        fx
    }

    /// El sistema informa que una ventana tomó el foco.
    pub fn note_focus(&mut self, id: WindowId) {
        if self.workspace_of(id) == Some(self.active) {
            self.focused = Some(id);
        }
    }

    pub fn switch_workspace(&mut self, ws: usize) -> Vec<Effect> {
        if ws >= WORKSPACE_COUNT || ws == self.active {
            return vec![];
        }
        let mut fx: Vec<Effect> = self.workspaces[self.active].iter().map(|&id| Effect::Hide(id)).collect();
        self.active = ws;
        for &id in &self.workspaces[ws] {
            fx.push(Effect::Show(id));
        }
        fx.extend(self.reflow_active());
        self.focused = self.workspaces[ws].last().copied();
        if let Some(f) = self.focused {
            fx.push(Effect::Focus(f));
        }
        fx
    }

    pub fn move_focused_to(&mut self, ws: usize, follow: bool) -> Vec<Effect> {
        let Some(id) = self.focused else { return vec![] };
        if ws >= WORKSPACE_COUNT || ws == self.active {
            return vec![];
        }
        self.workspaces[self.active].retain(|&w| w != id);
        self.workspaces[ws].push(id);
        let mut fx = vec![Effect::Hide(id)];
        if follow {
            // El destino pasa a ser el activo y se muestra completo.
            let previous: Vec<WindowId> = self.workspaces[self.active].clone();
            fx.extend(previous.into_iter().map(Effect::Hide));
            self.active = ws;
            fx.extend(self.workspaces[ws].iter().map(|&w| Effect::Show(w)));
            fx.extend(self.reflow_active());
            self.focused = Some(id);
            fx.push(Effect::Focus(id));
        } else {
            self.focused = self.workspaces[self.active].last().copied();
            fx.extend(self.reflow_active());
            if let Some(f) = self.focused {
                fx.push(Effect::Focus(f));
            }
        }
        fx
    }

    /// Cicla el foco entre las ventanas del workspace activo.
    pub fn focus_next(&mut self) -> Vec<Effect> {
        let list = &self.workspaces[self.active];
        if list.is_empty() {
            return vec![];
        }
        let pos = self.focused.and_then(|f| list.iter().position(|&w| w == f)).unwrap_or(list.len() - 1);
        let next = list[(pos + 1) % list.len()];
        self.focused = Some(next);
        vec![Effect::Focus(next)]
    }

    /// Todas las ventanas conocidas (para restaurarlas al pausar o salir).
    pub fn all_windows(&self) -> Vec<WindowId> {
        self.workspaces.iter().flatten().copied().collect()
    }

    pub fn set_area(&mut self, area: Rect) -> Vec<Effect> {
        self.area = area;
        self.reflow_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk() -> Desktop {
        Desktop::new(Rect::new(0, 0, 1000, 600))
    }

    fn placed(fx: &[Effect]) -> Vec<(WindowId, Rect)> {
        fx.iter()
            .filter_map(|e| if let Effect::Place { id, rect } = e { Some((*id, *rect)) } else { None })
            .collect()
    }

    #[test]
    fn dos_ventanas_se_reparten_el_ancho() {
        let mut d = desk();
        d.add_window(1);
        let fx = d.add_window(2);
        assert_eq!(placed(&fx), vec![(1, Rect::new(0, 0, 500, 600)), (2, Rect::new(500, 0, 500, 600))]);
    }

    #[test]
    fn cerrar_reparte_de_nuevo() {
        let mut d = desk();
        d.add_window(1);
        d.add_window(2);
        let fx = d.remove_window(2);
        assert_eq!(placed(&fx), vec![(1, Rect::new(0, 0, 1000, 600))]);
    }

    #[test]
    fn cambiar_de_workspace_oculta_y_muestra() {
        let mut d = desk();
        d.add_window(1);
        d.switch_workspace(1);
        d.add_window(2);
        let fx = d.switch_workspace(0);
        assert!(fx.contains(&Effect::Hide(2)));
        assert!(fx.contains(&Effect::Show(1)));
        assert_eq!(d.focused(), Some(1));
    }

    #[test]
    fn mover_ventana_y_seguirla() {
        let mut d = desk();
        d.add_window(1);
        d.add_window(2);
        d.move_focused_to(3, true);
        assert_eq!(d.active_workspace(), 3);
        assert_eq!(d.workspace_of(2), Some(3));
        assert_eq!(d.workspace_of(1), Some(0));
    }

    #[test]
    fn mover_sin_seguir_deja_el_foco_aqui() {
        let mut d = desk();
        d.add_window(1);
        d.add_window(2);
        let fx = d.move_focused_to(3, false);
        assert_eq!(d.active_workspace(), 0);
        assert!(fx.contains(&Effect::Hide(2)));
        assert_eq!(placed(&fx), vec![(1, Rect::new(0, 0, 1000, 600))]);
        assert_eq!(d.focused(), Some(1));
    }

    #[test]
    fn ventana_duplicada_se_ignora() {
        let mut d = desk();
        d.add_window(1);
        assert!(d.add_window(1).is_empty());
    }

    #[test]
    fn ciclar_foco_da_la_vuelta() {
        let mut d = desk();
        d.add_window(1);
        d.add_window(2);
        assert_eq!(d.focus_next(), vec![Effect::Focus(1)]);
        assert_eq!(d.focus_next(), vec![Effect::Focus(2)]);
    }
}
