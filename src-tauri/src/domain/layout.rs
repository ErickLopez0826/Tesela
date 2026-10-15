//! Motor de layout (§5): grupos (columnas) con ventanas apiladas, reparto proporcional y mínimos.
//!
//! Vocabulario: el *eje principal* reparte los grupos (columnas en `Axis::Columns`,
//! filas en `Axis::Rows`). Dentro de un grupo las ventanas se apilan en vertical
//! (`Stack::Vertical`, repartiéndose el alto) o en horizontal (`Stack::Horizontal`).
//! Todos los tamaños se guardan como proporciones que suman 1.

use crate::domain::effects::WindowId;
use crate::domain::geometry::{split_by_weights, Rect};

const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    Columns,
    Rows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stack {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// Eje de pantalla: X reparte anchos, Y reparte altos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    X,
    Y,
}

impl Dir {
    pub fn orientation(self) -> Orientation {
        match self {
            Dir::Left | Dir::Right => Orientation::X,
            Dir::Up | Dir::Down => Orientation::Y,
        }
    }

    pub fn is_forward(self) -> bool {
        matches!(self, Dir::Right | Dir::Down)
    }
}

impl Axis {
    pub fn orientation(self) -> Orientation {
        match self {
            Axis::Columns => Orientation::X,
            Axis::Rows => Orientation::Y,
        }
    }

    /// Apilado por defecto de un grupo nuevo (ver DECISIONS.md y BLOCKERS.md B-1).
    pub fn default_stack(self) -> Stack {
        match self {
            Axis::Columns => Stack::Vertical,
            Axis::Rows => Stack::Horizontal,
        }
    }
}

impl Stack {
    pub fn orientation(self) -> Orientation {
        match self {
            Stack::Vertical => Orientation::Y,
            Stack::Horizontal => Orientation::X,
        }
    }

    pub fn toggled(self) -> Stack {
        match self {
            Stack::Vertical => Stack::Horizontal,
            Stack::Horizontal => Stack::Vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gaps {
    pub inner: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Gaps {
    pub fn uniform(inner: i32, outer: i32) -> Self {
        Gaps { inner, top: outer, right: outer, bottom: outer, left: outer }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutSettings {
    /// Ancho mínimo en píxeles de una columna o ventana (§5.1).
    pub min_width: i32,
    /// Alto mínimo en píxeles de una fila o ventana apilada (§5.2).
    pub min_height: i32,
    pub gaps: Gaps,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        LayoutSettings { min_width: 200, min_height: 120, gaps: Gaps::default() }
    }
}

/// No cabe otra columna respetando el mínimo (§5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRoom;

#[derive(Debug, Clone, PartialEq)]
struct Group {
    windows: Vec<WindowId>,
    /// Proporciones a lo largo del apilado; suman 1.
    shares: Vec<f64>,
    stack: Stack,
    /// Proporción sobre el eje principal; todas las de un layout suman 1.
    size: f64,
}

impl Group {
    fn single(id: WindowId, stack: Stack, size: f64) -> Group {
        Group { windows: vec![id], shares: vec![1.0], stack, size }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    axis: Axis,
    groups: Vec<Group>,
}

/// Escala `weights` para que sumen `total` y ninguno baje de `min` ("llenado de agua", D-6).
/// Si no caben todos con el mínimo, reparte a partes iguales.
pub fn fit_with_min(weights: &[f64], total: f64, min: f64) -> Vec<f64> {
    let n = weights.len();
    if n == 0 {
        return vec![];
    }
    if n as f64 * min >= total - EPS {
        return vec![total / n as f64; n];
    }
    let mut fixed = vec![false; n];
    let mut out = vec![0.0; n];
    loop {
        let fixed_count = fixed.iter().filter(|f| **f).count();
        let free_total = total - fixed_count as f64 * min;
        let free_weight: f64 = (0..n).filter(|&i| !fixed[i]).map(|i| weights[i].max(0.0)).sum();
        let mut violated = false;
        for i in 0..n {
            if fixed[i] {
                out[i] = min;
                continue;
            }
            out[i] = if free_weight > EPS {
                weights[i].max(0.0) / free_weight * free_total
            } else {
                free_total / (n - fixed_count) as f64
            };
        }
        for i in 0..n {
            if !fixed[i] && out[i] < min - EPS {
                fixed[i] = true;
                violated = true;
            }
        }
        if !violated {
            return out;
        }
    }
}

/// Redimensiona la proporción `idx` en `delta`, repartiendo la diferencia entre las demás en proporción.
/// Devuelve `false` si no hay nada que cambiar.
fn resize_share(shares: &mut [f64], idx: usize, delta: f64, min: f64) -> bool {
    let n = shares.len();
    if n < 2 || n as f64 * min >= 1.0 - EPS {
        return false;
    }
    let max = 1.0 - (n - 1) as f64 * min;
    let target = (shares[idx] + delta).clamp(min, max);
    if (target - shares[idx]).abs() < EPS {
        return false;
    }
    let others: Vec<f64> = (0..n).filter(|&i| i != idx).map(|i| shares[i]).collect();
    let fitted = fit_with_min(&others, 1.0 - target, min);
    let mut it = fitted.into_iter();
    for (i, s) in shares.iter_mut().enumerate() {
        *s = if i == idx { target } else { it.next().unwrap() };
    }
    true
}

fn len_along(r: &Rect, o: Orientation) -> i32 {
    match o {
        Orientation::X => r.w,
        Orientation::Y => r.h,
    }
}

impl Layout {
    pub fn new(axis: Axis) -> Self {
        Layout { axis, groups: vec![] }
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    pub fn set_axis(&mut self, axis: Axis) {
        self.axis = axis;
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Ventanas en orden de grupo y de apilado.
    pub fn windows(&self) -> Vec<WindowId> {
        self.groups.iter().flat_map(|g| g.windows.iter().copied()).collect()
    }

    pub fn contains(&self, id: WindowId) -> bool {
        self.locate(id).is_some()
    }

    /// Ventanas de cada grupo, para inspección y pruebas.
    pub fn group_windows(&self) -> Vec<Vec<WindowId>> {
        self.groups.iter().map(|g| g.windows.clone()).collect()
    }

    pub fn group_sizes(&self) -> Vec<f64> {
        self.groups.iter().map(|g| g.size).collect()
    }

    pub fn stack_shares(&self, group: usize) -> Vec<f64> {
        self.groups[group].shares.clone()
    }

    pub fn stack_of(&self, id: WindowId) -> Option<Stack> {
        self.locate(id).map(|(g, _)| self.groups[g].stack)
    }

    fn locate(&self, id: WindowId) -> Option<(usize, usize)> {
        self.groups.iter().enumerate().find_map(|(gi, g)| g.windows.iter().position(|&w| w == id).map(|wi| (gi, wi)))
    }

    // ---------- geometría ----------

    fn inner_area(area: &Rect, s: &LayoutSettings) -> Rect {
        let g = &s.gaps;
        Rect::new(
            area.x + g.left,
            area.y + g.top,
            (area.w - g.left - g.right).max(0),
            (area.h - g.top - g.bottom).max(0),
        )
    }

    fn main_min(&self, s: &LayoutSettings) -> i32 {
        match self.axis {
            Axis::Columns => s.min_width,
            Axis::Rows => s.min_height,
        }
    }

    fn stack_min(stack: Stack, s: &LayoutSettings) -> i32 {
        match stack {
            Stack::Vertical => s.min_height,
            Stack::Horizontal => s.min_width,
        }
    }

    /// Longitud disponible en el eje principal para `n` grupos (sin huecos entre ellos).
    fn main_available(&self, inner: &Rect, n: usize, s: &LayoutSettings) -> i32 {
        len_along(inner, self.axis.orientation()) - (n.saturating_sub(1)) as i32 * s.gaps.inner
    }

    fn main_min_ratio(&self, inner: &Rect, n: usize, s: &LayoutSettings) -> f64 {
        let avail = self.main_available(inner, n, s);
        if avail <= 0 {
            return 1.0;
        }
        self.main_min(s) as f64 / avail as f64
    }

    /// Rectángulo de cada grupo, aplicando los mínimos.
    fn group_rects(&self, area: &Rect, s: &LayoutSettings) -> Vec<Rect> {
        let n = self.groups.len();
        if n == 0 {
            return vec![];
        }
        let inner = Self::inner_area(area, s);
        let avail = self.main_available(&inner, n, s);
        let sizes: Vec<f64> = self.groups.iter().map(|g| g.size).collect();
        let fitted = fit_with_min(&sizes, 1.0, self.main_min_ratio(&inner, n, s));
        let px = split_by_weights(avail, &fitted);
        let mut pos = match self.axis {
            Axis::Columns => inner.x,
            Axis::Rows => inner.y,
        };
        px.into_iter()
            .map(|len| {
                let r = match self.axis {
                    Axis::Columns => Rect::new(pos, inner.y, len, inner.h),
                    Axis::Rows => Rect::new(inner.x, pos, inner.w, len),
                };
                pos += len + s.gaps.inner;
                r
            })
            .collect()
    }

    /// Posición final de cada ventana dentro de `area` (área útil del monitor).
    pub fn compute(&self, area: &Rect, s: &LayoutSettings) -> Vec<(WindowId, Rect)> {
        let rects = self.group_rects(area, s);
        let mut out = Vec::new();
        for (g, gr) in self.groups.iter().zip(rects) {
            let o = g.stack.orientation();
            let m = g.windows.len();
            let total = len_along(&gr, o);
            let avail = total - (m.saturating_sub(1)) as i32 * s.gaps.inner;
            let min_r = if avail > 0 { Self::stack_min(g.stack, s) as f64 / avail as f64 } else { 1.0 };
            let fitted = fit_with_min(&g.shares, 1.0, min_r);
            let px = split_by_weights(avail, &fitted);
            let mut pos = match o {
                Orientation::X => gr.x,
                Orientation::Y => gr.y,
            };
            for (&id, len) in g.windows.iter().zip(px) {
                let r = match o {
                    Orientation::X => Rect::new(pos, gr.y, len, gr.h),
                    Orientation::Y => Rect::new(gr.x, pos, gr.w, len),
                };
                pos += len + s.gaps.inner;
                out.push((id, r));
            }
        }
        out
    }

    /// Guarda en el modelo los tamaños efectivos (con mínimos aplicados) del área actual.
    fn normalize(&mut self, area: &Rect, s: &LayoutSettings) {
        let n = self.groups.len();
        if n == 0 {
            return;
        }
        let inner = Self::inner_area(area, s);
        let sizes: Vec<f64> = self.groups.iter().map(|g| g.size).collect();
        let fitted = fit_with_min(&sizes, 1.0, self.main_min_ratio(&inner, n, s));
        let rects = self.group_rects(area, s);
        for ((g, size), gr) in self.groups.iter_mut().zip(fitted).zip(rects) {
            g.size = size;
            let m = g.windows.len();
            let avail = len_along(&gr, g.stack.orientation()) - (m.saturating_sub(1)) as i32 * s.gaps.inner;
            let min_r = if avail > 0 { Self::stack_min(g.stack, s) as f64 / avail as f64 } else { 1.0 };
            g.shares = fit_with_min(&g.shares, 1.0, min_r);
        }
    }

    // ---------- inserción y cierre ----------

    /// ¿Cabe otra columna respetando el ancho mínimo? (§5.6)
    pub fn can_add_group(&self, area: &Rect, s: &LayoutSettings) -> bool {
        let inner = Self::inner_area(area, s);
        let n = self.groups.len() + 1;
        let needed = n as i32 * self.main_min(s) + (n as i32 - 1) * s.gaps.inner;
        needed <= len_along(&inner, self.axis.orientation())
    }

    fn can_add_to_group(&self, gi: usize, area: &Rect, s: &LayoutSettings) -> bool {
        let g = &self.groups[gi];
        let rects = self.group_rects(area, s);
        let m = g.windows.len() + 1;
        let needed = m as i32 * Self::stack_min(g.stack, s) + (m as i32 - 1) * s.gaps.inner;
        needed <= len_along(&rects[gi], g.stack.orientation())
    }

    /// Añade `id` como grupo nuevo en `pos` (0 = primero). Reescala los demás en proporción.
    fn insert_group_at(&mut self, pos: usize, id: WindowId) {
        let n = self.groups.len();
        let size = 1.0 / (n + 1) as f64;
        let scale = n as f64 / (n + 1) as f64;
        for g in &mut self.groups {
            g.size *= scale;
        }
        self.groups.insert(pos, Group::single(id, self.axis.default_stack(), size));
    }

    /// Ventana nueva: columna a la derecha de la última (§5.3). Falla si no cabe (§5.6).
    pub fn insert_window(&mut self, id: WindowId, area: &Rect, s: &LayoutSettings) -> Result<(), NoRoom> {
        if self.contains(id) {
            return Ok(());
        }
        if !self.can_add_group(area, s) {
            return Err(NoRoom);
        }
        let n = self.groups.len();
        self.insert_group_at(n, id);
        self.normalize(area, s);
        Ok(())
    }

    /// Índice de la columna que contiene la ventana.
    pub fn group_index_of(&self, id: WindowId) -> Option<usize> {
        self.locate(id).map(|(g, _)| g)
    }

    /// Como `insert_window`, pero la columna nueva queda en la posición `pos` (se recorta al final).
    pub fn insert_window_at(&mut self, id: WindowId, pos: usize, area: &Rect, s: &LayoutSettings) -> Result<(), NoRoom> {
        if self.contains(id) {
            return Ok(());
        }
        if !self.can_add_group(area, s) {
            return Err(NoRoom);
        }
        let pos = pos.min(self.groups.len());
        self.insert_group_at(pos, id);
        self.normalize(area, s);
        Ok(())
    }

    /// Quita una ventana; si su grupo queda vacío desaparece y los demás se reparten el espacio (§5.4).
    pub fn remove_window(&mut self, id: WindowId, area: &Rect, s: &LayoutSettings) -> bool {
        let Some((gi, wi)) = self.locate(id) else { return false };
        self.take_out(gi, wi);
        self.normalize(area, s);
        true
    }

    fn take_out(&mut self, gi: usize, wi: usize) -> bool {
        let g = &mut self.groups[gi];
        g.windows.remove(wi);
        g.shares.remove(wi);
        if g.windows.is_empty() {
            self.groups.remove(gi);
            if !self.groups.is_empty() {
                let weights: Vec<f64> = self.groups.iter().map(|g| g.size).collect();
                let sum: f64 = weights.iter().sum();
                for (g, w) in self.groups.iter_mut().zip(&weights) {
                    g.size = if sum > EPS { w / sum } else { 1.0 / weights.len() as f64 };
                }
            }
            true
        } else {
            let sum: f64 = g.shares.iter().sum();
            for s in &mut g.shares {
                *s /= sum;
            }
            false
        }
    }

    // ---------- redimensionar ----------

    /// Cambia el tamaño de la ventana en el eje indicado (§5.1, §5.2). `delta` en proporción (0.02 = 2%).
    pub fn resize(&mut self, id: WindowId, o: Orientation, delta: f64, area: &Rect, s: &LayoutSettings) -> bool {
        let Some((gi, wi)) = self.locate(id) else { return false };
        self.normalize(area, s);
        let inner = Self::inner_area(area, s);
        let in_stack = self.groups[gi].stack.orientation() == o && self.groups[gi].windows.len() > 1;
        if in_stack {
            let rect = self.group_rects(area, s)[gi];
            let m = self.groups[gi].windows.len();
            let avail = len_along(&rect, o) - (m.saturating_sub(1)) as i32 * s.gaps.inner;
            if avail <= 0 {
                return false;
            }
            let min_r = Self::stack_min(self.groups[gi].stack, s) as f64 / avail as f64;
            resize_share(&mut self.groups[gi].shares, wi, delta, min_r)
        } else if o == self.axis.orientation() {
            let min_r = self.main_min_ratio(&inner, self.groups.len(), s);
            let mut sizes: Vec<f64> = self.groups.iter().map(|g| g.size).collect();
            let changed = resize_share(&mut sizes, gi, delta, min_r);
            for (g, v) in self.groups.iter_mut().zip(sizes) {
                g.size = v;
            }
            changed
        } else {
            false
        }
    }

    // ---------- movimiento ----------

    /// Alterna el apilado del grupo de la ventana (alt+v) y reparte sus ventanas por igual.
    pub fn toggle_stack(&mut self, id: WindowId) -> bool {
        let Some((gi, _)) = self.locate(id) else { return false };
        let g = &mut self.groups[gi];
        g.stack = g.stack.toggled();
        let n = g.windows.len();
        g.shares = vec![1.0 / n as f64; n];
        true
    }

    /// Mueve la ventana en una dirección. Devuelve `false` si no hay cambio posible.
    pub fn move_window(&mut self, id: WindowId, dir: Dir, area: &Rect, s: &LayoutSettings) -> bool {
        let Some((gi, wi)) = self.locate(id) else { return false };
        let o = dir.orientation();
        let step: isize = if dir.is_forward() { 1 } else { -1 };
        let stack_o = self.groups[gi].stack.orientation();
        if o == stack_o {
            let target = wi as isize + step;
            if target >= 0 && (target as usize) < self.groups[gi].windows.len() {
                self.groups[gi].windows.swap(wi, target as usize);
                return true;
            }
        }
        if o != self.axis.orientation() {
            return false;
        }
        // Salto entre grupos a lo largo del eje principal.
        let tgi = gi as isize + step;
        if tgi >= 0 && (tgi as usize) < self.groups.len() {
            let tgi = tgi as usize;
            if !self.can_add_to_group(tgi, area, s) {
                return false;
            }
            let removed_group = self.take_out(gi, wi);
            let tgi = if removed_group && tgi > gi { tgi - 1 } else { tgi };
            let t = &mut self.groups[tgi];
            let m = t.windows.len();
            let pos = if dir.is_forward() { 0 } else { m };
            t.windows.insert(pos, id);
            let share = 1.0 / (m + 1) as f64;
            for sh in &mut t.shares {
                *sh *= m as f64 / (m + 1) as f64;
            }
            t.shares.insert(pos, share);
            self.normalize(area, s);
            return true;
        }
        // Borde: una ventana apilada sale a una columna nueva si cabe.
        if self.groups[gi].windows.len() > 1 && self.can_add_group(area, s) {
            self.take_out(gi, wi);
            let pos = if dir.is_forward() { self.groups.len() } else { 0 };
            self.insert_group_at(pos, id);
            self.normalize(area, s);
            return true;
        }
        false
    }

    /// Sustituye una ventana por otra en el mismo hueco (p. ej. al cambiar de identidad).
    pub fn replace(&mut self, old: WindowId, new: WindowId) -> bool {
        match self.locate(old) {
            Some((gi, wi)) => {
                self.groups[gi].windows[wi] = new;
                true
            }
            None => false,
        }
    }
}

/// Vecino espacial de `id` en una dirección, a partir de las posiciones calculadas.
pub fn focus_neighbor(id: WindowId, dir: Dir, rects: &[(WindowId, Rect)]) -> Option<WindowId> {
    let cur = rects.iter().find(|(w, _)| *w == id)?.1;
    let (cx, cy) = cur.center();
    let overlap = |a0: i32, a1: i32, b0: i32, b1: i32| a0 < b1 && b0 < a1;
    rects
        .iter()
        .filter(|(w, _)| *w != id)
        .filter(|(_, r)| {
            let (x, y) = r.center();
            match dir {
                Dir::Right => x > cx,
                Dir::Left => x < cx,
                Dir::Down => y > cy,
                Dir::Up => y < cy,
            }
        })
        .map(|(w, r)| {
            let (x, y) = r.center();
            let (along, across, overlaps) = match dir.orientation() {
                Orientation::X => ((x - cx).abs(), (y - cy).abs(), overlap(cur.y, cur.bottom(), r.y, r.bottom())),
                Orientation::Y => ((y - cy).abs(), (x - cx).abs(), overlap(cur.x, cur.right(), r.x, r.right())),
            };
            ((!overlaps) as i32, along, across, *w)
        })
        .min()
        .map(|(_, _, _, w)| w)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 0, 3000, 1000);

    fn s() -> LayoutSettings {
        LayoutSettings { min_width: 200, min_height: 100, gaps: Gaps::default() }
    }

    fn layout_with(n: isize) -> Layout {
        let mut l = Layout::new(Axis::Columns);
        for i in 1..=n {
            l.insert_window(i, &AREA, &s()).unwrap();
        }
        l
    }

    fn rect_of(l: &Layout, id: WindowId) -> Rect {
        l.compute(&AREA, &s()).into_iter().find(|(w, _)| *w == id).unwrap().1
    }

    fn sum(v: &[f64]) -> f64 {
        v.iter().sum()
    }

    // ----- Agregar 1, 2, 5 y 10 columnas -----

    #[test]
    fn agregar_columnas_reparte_el_ancho() {
        for n in [1, 2, 5, 10] {
            let l = layout_with(n);
            let rects = l.compute(&AREA, &s());
            assert_eq!(rects.len(), n as usize);
            assert_eq!(rects.iter().map(|(_, r)| r.w).sum::<i32>(), 3000, "n={n}");
            let ideal = 3000.0 / n as f64;
            for (_, r) in &rects {
                assert!((r.w as f64 - ideal).abs() <= 1.0, "n={n} ancho={}", r.w);
                assert_eq!(r.h, 1000);
            }
            assert!((sum(&l.group_sizes()) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn ventana_nueva_va_a_la_derecha() {
        let l = layout_with(3);
        let xs: Vec<i32> = (1..=3).map(|i| rect_of(&l, i).x).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
    }

    // ----- Redimensionar -----

    #[test]
    fn redimensionar_reajusta_en_proporcion_y_la_suma_no_cambia() {
        let mut l = layout_with(3);
        // Dejamos tamaños distintos: 0.5 / 0.3 / 0.2.
        assert!(l.resize(1, Orientation::X, 0.5 - 1.0 / 3.0, &AREA, &s()));
        let before = l.group_sizes();
        let ratio_before = before[1] / before[2];
        assert!(l.resize(1, Orientation::X, 0.1, &AREA, &s()));
        let after = l.group_sizes();
        assert!((after[0] - (before[0] + 0.1)).abs() < 1e-9);
        assert!((after[1] / after[2] - ratio_before).abs() < 1e-9, "las demás conservan su proporción");
        assert!((sum(&after) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn redimensionar_no_baja_del_minimo() {
        let mut l = layout_with(3);
        for _ in 0..100 {
            l.resize(1, Orientation::X, -0.02, &AREA, &s());
        }
        let r = rect_of(&l, 1);
        assert!(r.w >= 200, "ancho {}", r.w);
        assert!(r.w <= 201, "debería quedar en el mínimo, ancho {}", r.w);
        assert!((sum(&l.group_sizes()) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn agrandar_hasta_el_limite_deja_a_las_demas_en_el_minimo() {
        let mut l = layout_with(3);
        for _ in 0..200 {
            l.resize(1, Orientation::X, 0.02, &AREA, &s());
        }
        for id in [2, 3] {
            assert!(rect_of(&l, id).w >= 200);
        }
        assert_eq!(l.compute(&AREA, &s()).iter().map(|(_, r)| r.w).sum::<i32>(), 3000);
    }

    #[test]
    fn redimensionar_una_unica_columna_no_hace_nada() {
        let mut l = layout_with(1);
        assert!(!l.resize(1, Orientation::X, 0.1, &AREA, &s()));
    }

    // ----- Cierre -----

    #[test]
    fn cerrar_una_columna_reparte_de_nuevo() {
        let mut l = layout_with(4);
        l.remove_window(2, &AREA, &s());
        let rects = l.compute(&AREA, &s());
        assert_eq!(rects.len(), 3);
        for (_, r) in rects {
            assert!((r.w - 1000).abs() <= 1);
        }
    }

    #[test]
    fn cerrar_conserva_proporciones_relativas() {
        let mut l = layout_with(3);
        l.resize(1, Orientation::X, 0.2, &AREA, &s());
        let before = l.group_sizes();
        l.remove_window(3, &AREA, &s());
        let after = l.group_sizes();
        assert!((after[0] / after[1] - before[0] / before[1]).abs() < 1e-9);
        assert!((sum(&after) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cerrar_la_ultima_columna_deja_el_workspace_vacio() {
        let mut l = layout_with(1);
        assert!(l.remove_window(1, &AREA, &s()));
        assert!(l.is_empty());
        assert!(l.compute(&AREA, &s()).is_empty());
        assert!(!l.remove_window(1, &AREA, &s()), "cerrar de nuevo no falla");
    }

    // ----- Apiladas -----

    #[test]
    fn mover_a_otra_columna_reajusta_ambas() {
        let mut l = layout_with(3);
        assert!(l.move_window(1, Dir::Right, &AREA, &s()));
        // La columna 1 desapareció: quedan 2 columnas, la primera con 1 y 2 apiladas.
        assert_eq!(l.group_windows(), vec![vec![1, 2], vec![3]]);
        for (_, r) in l.compute(&AREA, &s()) {
            assert!((r.w - 1500).abs() <= 1);
        }
    }

    #[test]
    fn apiladas_reparten_el_alto() {
        let mut l = layout_with(2);
        l.move_window(2, Dir::Left, &AREA, &s());
        assert_eq!(l.group_windows(), vec![vec![1, 2]]);
        let rects = l.compute(&AREA, &s());
        assert_eq!(rects.iter().map(|(_, r)| r.h).sum::<i32>(), 1000);
        assert!(rects.iter().all(|(_, r)| r.w == 3000 && (r.h - 500).abs() <= 1));
    }

    #[test]
    fn redimensionar_alto_reajusta_las_otras_en_proporcion() {
        let mut l = layout_with(1);
        for id in 2..=4 {
            // Apilamos a mano: mover a la columna de la izquierda.
            l.insert_window(id, &AREA, &s()).unwrap();
            l.move_window(id, Dir::Left, &AREA, &s());
        }
        assert_eq!(l.group_windows().len(), 1);
        let top = l.group_windows()[0][0];
        assert!(l.resize(top, Orientation::Y, 0.1, &AREA, &s()));
        let shares = l.stack_shares(0);
        assert!((sum(&shares) - 1.0).abs() < 1e-9);
        assert!((shares[0] - 0.35).abs() < 1e-9);
        assert!((shares[1] - shares[2]).abs() < 1e-9 && (shares[2] - shares[3]).abs() < 1e-9);
    }

    #[test]
    fn alto_minimo_respetado() {
        let mut l = layout_with(1);
        for id in 2..=3 {
            l.insert_window(id, &AREA, &s()).unwrap();
            l.move_window(id, Dir::Left, &AREA, &s());
        }
        let first = l.group_windows()[0][0];
        for _ in 0..100 {
            l.resize(first, Orientation::Y, -0.02, &AREA, &s());
        }
        let r = rect_of(&l, first);
        assert!(r.h >= 100 && r.h <= 101, "alto {}", r.h);
    }

    #[test]
    fn salir_de_una_pila_crea_columna_nueva() {
        let mut l = layout_with(2);
        l.move_window(2, Dir::Left, &AREA, &s());
        assert!(l.move_window(1, Dir::Right, &AREA, &s()));
        assert_eq!(l.group_windows(), vec![vec![2], vec![1]]);
    }

    #[test]
    fn reordenar_dentro_de_la_pila() {
        let mut l = layout_with(2);
        l.move_window(2, Dir::Left, &AREA, &s());
        assert_eq!(l.group_windows(), vec![vec![1, 2]]);
        assert!(l.move_window(2, Dir::Up, &AREA, &s()));
        assert_eq!(l.group_windows(), vec![vec![2, 1]]);
        assert!(!l.move_window(2, Dir::Up, &AREA, &s()), "ya está al principio");
    }

    #[test]
    fn alternar_apilado_cambia_de_vertical_a_horizontal() {
        let mut l = layout_with(2);
        l.move_window(2, Dir::Left, &AREA, &s());
        assert_eq!(l.stack_of(1), Some(Stack::Vertical));
        assert!(l.toggle_stack(1));
        assert_eq!(l.stack_of(1), Some(Stack::Horizontal));
        let rects = l.compute(&AREA, &s());
        assert!(rects.iter().all(|(_, r)| (r.w - 1500).abs() <= 1 && r.h == 1000));
        // En horizontal, "alt+u/p" (ancho) afecta al reparto dentro del grupo.
        assert!(l.resize(2, Orientation::X, 0.1, &AREA, &s()));
    }

    // ----- Mínimos y caso límite (§5.6) -----

    #[test]
    fn no_cabe_otra_columna() {
        // 3000 px / 200 mínimo = 15 columnas como máximo.
        let mut l = layout_with(15);
        assert!(!l.can_add_group(&AREA, &s()));
        assert_eq!(l.insert_window(99, &AREA, &s()), Err(NoRoom));
        assert!(!l.contains(99));
        // Las demás no se movieron ni se rompió el mínimo.
        let rects = l.compute(&AREA, &s());
        assert_eq!(rects.len(), 15);
        assert!(rects.iter().all(|(_, r)| r.w == 200));
    }

    #[test]
    fn insertar_en_una_posicion() {
        let mut l = layout_with(3);
        l.insert_window_at(9, 1, &AREA, &s()).unwrap();
        assert_eq!(l.group_windows(), vec![vec![1], vec![9], vec![2], vec![3]]);
        assert_eq!(l.group_index_of(9), Some(1));
    }

    #[test]
    fn ultima_columna_cabe_exacta() {
        let l = layout_with(14);
        assert!(l.can_add_group(&AREA, &s()));
    }

    #[test]
    fn achicar_el_area_aplica_los_minimos() {
        let l = layout_with(5);
        let small = Rect::new(0, 0, 1100, 1000);
        let rects = l.compute(&small, &s());
        assert_eq!(rects.iter().map(|(_, r)| r.w).sum::<i32>(), 1100);
        assert!(rects.iter().all(|(_, r)| r.w >= 200));
    }

    // ----- Gaps -----

    #[test]
    fn gaps_exteriores_e_interiores() {
        let settings = LayoutSettings { gaps: Gaps { inner: 20, top: 60, right: 20, bottom: 20, left: 20 }, ..s() };
        let mut l = Layout::new(Axis::Columns);
        l.insert_window(1, &AREA, &settings).unwrap();
        l.insert_window(2, &AREA, &settings).unwrap();
        let r = l.compute(&AREA, &settings);
        assert_eq!(r[0].1.x, 20);
        assert_eq!(r[0].1.y, 60);
        assert_eq!(r[0].1.h, 1000 - 60 - 20);
        assert_eq!(r[1].1.x, r[0].1.right() + 20);
        assert_eq!(r[1].1.right(), 3000 - 20);
    }

    // ----- Ejes -----

    #[test]
    fn eje_filas_reparte_el_alto() {
        let mut l = Layout::new(Axis::Rows);
        let area = Rect::new(0, 0, 1000, 3000);
        for i in 1..=3 {
            l.insert_window(i, &area, &s()).unwrap();
        }
        let rects = l.compute(&area, &s());
        assert!(rects.iter().all(|(_, r)| r.w == 1000 && (r.h - 1000).abs() <= 1));
        assert!(rects[0].1.y < rects[1].1.y && rects[1].1.y < rects[2].1.y);
        assert_eq!(l.stack_of(1), Some(Stack::Horizontal));
    }

    #[test]
    fn eje_filas_minimo_usa_alto_minimo() {
        let mut l = Layout::new(Axis::Rows);
        let area = Rect::new(0, 0, 1000, 1000); // 1000/100 = 10 filas máximo
        for i in 1..=10 {
            l.insert_window(i, &area, &s()).unwrap();
        }
        assert_eq!(l.insert_window(11, &area, &s()), Err(NoRoom));
    }

    // ----- Foco espacial -----

    #[test]
    fn foco_hacia_los_lados_y_arriba_abajo() {
        let mut l = layout_with(2);
        l.insert_window(3, &AREA, &s()).unwrap();
        l.move_window(3, Dir::Left, &AREA, &s()); // 3 apilada con 2
        let rects = l.compute(&AREA, &s());
        assert_eq!(focus_neighbor(1, Dir::Right, &rects), Some(2));
        assert_eq!(focus_neighbor(2, Dir::Left, &rects), Some(1));
        let (top, bottom) = (l.group_windows()[1][0], l.group_windows()[1][1]);
        assert_eq!(focus_neighbor(top, Dir::Down, &rects), Some(bottom));
        assert_eq!(focus_neighbor(bottom, Dir::Up, &rects), Some(top));
        assert_eq!(focus_neighbor(1, Dir::Left, &rects), None);
    }

    // ----- Utilidades -----

    #[test]
    fn fit_con_minimo_conserva_suma_y_respeta_minimo() {
        let out = fit_with_min(&[0.9, 0.05, 0.05], 1.0, 0.2);
        assert!((sum(&out) - 1.0).abs() < 1e-9);
        assert!(out.iter().all(|v| *v >= 0.2 - 1e-9));
        assert!((out[1] - 0.2).abs() < 1e-9 && (out[2] - 0.2).abs() < 1e-9);
    }

    #[test]
    fn fit_sin_espacio_reparte_igual() {
        let out = fit_with_min(&[0.7, 0.3], 1.0, 0.6);
        assert_eq!(out, vec![0.5, 0.5]);
    }
}
