use crate::domain::actions::Action;
use crate::domain::desktop::Monitor;
use crate::domain::effects::{Effect, Transition, WindowId};
use crate::domain::manageable::WindowFacts;
use crate::domain::shortcuts::Chord;
use crate::domain::snapshot::{StateSnapshot, SystemStats};
use crate::domain::window::WindowCandidate;

/// Prefijo del error al aplicar un efecto cuando Windows no deja mover la ventana (programa con más privilegios).
pub const DENIED: &str = "acceso denegado";

/// Lo que la plataforma devuelve al empezar una animación de cambio de workspace; se entrega a `slide_end` para cerrarla.
pub struct SlideHandle(pub Box<dyn std::any::Any + Send>);

/// Puerto: fuente de ventanas del sistema.
pub trait WindowEnumerator {
    fn enumerate(&self) -> Result<Vec<WindowCandidate>, String>;
}

/// Puerto: métricas del sistema (CPU, RAM).
pub trait SystemStatsProvider {
    fn read(&mut self) -> SystemStats;
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
    /// ¿Está la ventana en pantalla completa? y en qué monitor. `None` si ya no existe.
    fn fullscreen_state(&self, id: WindowId) -> Option<(bool, Option<u32>)>;
    /// Reemplaza los atajos globales registrados. Devuelve las combinaciones que no se pudieron registrar.
    fn set_hotkeys(&mut self, bindings: &[(Chord, Action)]) -> Vec<String>;
    /// Abre una terminal (alt+enter).
    fn open_terminal(&mut self);
    /// Publica el estado visible (workspaces, ventana enfocada...) para la barra y el panel.
    fn publish_state(&mut self, state: &StateSnapshot);
    /// Abre o cierra el panel desplegable en el monitor indicado (§9.2).
    fn toggle_panel(&mut self, monitor: u32);
    /// Abre o cierra el buscador de aplicaciones en el monitor bajo el cursor (§18).
    fn toggle_search(&mut self) {}
    /// Avisa al usuario (registro y, si hay interfaz, un aviso en pantalla).
    fn notify(&mut self, text: &str);
    /// Anima el cambio de workspace (las ventanas que salen suben, las que entran bajan). Vuelve cuando la animación
    /// llega al final, con las ventanas viejas ya ocultas y las nuevas aún sin mostrar; el controlador aplica entonces
    /// el resto de efectos y llama a `slide_end`. `None`: sin animación (el cambio se hace como siempre).
    fn slide_begin(&mut self, _transition: &Transition) -> Option<SlideHandle> {
        None
    }
    /// Retira lo que dibujó `slide_begin`, ya con las ventanas reales en su sitio.
    fn slide_end(&mut self, _handle: SlideHandle) {}
}
