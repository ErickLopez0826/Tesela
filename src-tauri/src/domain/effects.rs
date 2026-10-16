use crate::domain::geometry::Rect;

pub type WindowId = isize;

/// Orden que el dominio pide a la infraestructura. El dominio nunca toca Windows.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Place { id: WindowId, rect: Rect },
    Hide(WindowId),
    Show(WindowId),
    Focus(WindowId),
    Close(WindowId),
    Minimize(WindowId),
    Restore(WindowId),
    /// Muestra u oculta la barra de un monitor (pantalla completa, §6.4).
    SetBar { monitor: u32, visible: bool },
}

/// Cambio de workspace en un monitor, para que la plataforma lo anime: las ventanas que salen suben y las que entran
/// bajan. No cambia nada por sí misma: los `Effect`s siguen siendo los que dejan el escritorio en su estado final.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// Área útil del monitor donde ocurre.
    pub area: Rect,
    /// Ventanas del workspace que se oculta (hoy en pantalla).
    pub out: Vec<WindowId>,
    /// Ventanas del workspace que se muestra.
    pub inn: Vec<WindowId>,
}
