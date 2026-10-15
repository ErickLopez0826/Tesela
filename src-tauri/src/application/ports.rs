use crate::domain::desktop::Monitor;
use crate::domain::effects::{Effect, WindowId};
use crate::domain::manageable::WindowFacts;
use crate::domain::window::WindowCandidate;

/// Puerto: fuente de ventanas del sistema.
pub trait WindowEnumerator {
    fn enumerate(&self) -> Result<Vec<WindowCandidate>, String>;
}

/// Puerto: todo lo que el gestor necesita de Windows.
pub trait WindowPlatform {
    /// Hechos actuales de una ventana; `None` si ya no existe.
    fn facts(&self, id: WindowId) -> Option<WindowFacts>;
    /// Ventanas de nivel superior existentes (para adoptarlas al arrancar).
    fn top_level_windows(&self) -> Vec<WindowId>;
    /// Monitores conectados, ordenados de izquierda a derecha y de arriba abajo.
    fn monitors(&self) -> Vec<Monitor>;
    fn apply(&mut self, effect: &Effect) -> Result<(), String>;
    /// Olvida lo recordado de una ventana que ya no se gestiona (sin tocarla).
    fn forget(&mut self, id: WindowId);
    /// Devuelve cada ventana tocada a su posición original y la muestra.
    fn restore_all(&mut self);
}
