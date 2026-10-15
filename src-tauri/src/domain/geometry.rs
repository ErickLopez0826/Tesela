//! Geometría básica en píxeles de pantalla.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// Reduce el rectángulo `n` píxeles por cada lado (sin salir de tamaño 0).
    pub fn inset(&self, n: i32) -> Rect {
        Rect::new(self.x + n, self.y + n, (self.w - 2 * n).max(0), (self.h - 2 * n).max(0))
    }

    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.x >= self.x && other.y >= self.y && other.right() <= self.right() && other.bottom() <= self.bottom()
    }

    /// Centra un rectángulo de `w`×`h` dentro de este (se recorta si no cabe).
    pub fn centered(&self, w: i32, h: i32) -> Rect {
        let w = w.min(self.w);
        let h = h.min(self.h);
        Rect::new(self.x + (self.w - w) / 2, self.y + (self.h - h) / 2, w, h)
    }
}

/// Reparte `total` píxeles según `weights` (proporciones positivas) con enteros
/// cuya suma es exactamente `total` (método del mayor resto).
pub fn split_by_weights(total: i32, weights: &[f64]) -> Vec<i32> {
    let n = weights.len();
    if n == 0 {
        return vec![];
    }
    let total = total.max(0);
    let sum: f64 = weights.iter().sum();
    let ideal: Vec<f64> = if sum > 0.0 {
        weights.iter().map(|w| w / sum * total as f64).collect()
    } else {
        vec![total as f64 / n as f64; n]
    };
    let mut out: Vec<i32> = ideal.iter().map(|v| v.floor() as i32).collect();
    let mut rest = total - out.iter().sum::<i32>();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let fa = ideal[a] - ideal[a].floor();
        let fb = ideal[b] - ideal[b].floor();
        fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
    });
    for i in order {
        if rest <= 0 {
            break;
        }
        out[i] += 1;
        rest -= 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reparto_suma_exacta() {
        for n in 1..=10 {
            let parts = split_by_weights(1000, &vec![1.0; n]);
            assert_eq!(parts.iter().sum::<i32>(), 1000, "n={n}");
        }
    }

    #[test]
    fn reparto_respeta_pesos() {
        assert_eq!(split_by_weights(100, &[1.0, 3.0]), vec![25, 75]);
    }

    #[test]
    fn centrado_no_se_sale() {
        let r = Rect::new(0, 0, 100, 100).centered(500, 40);
        assert_eq!(r, Rect::new(0, 30, 100, 40));
    }

    #[test]
    fn inset_no_negativo() {
        assert_eq!(Rect::new(0, 0, 10, 10).inset(8).w, 0);
    }
}
