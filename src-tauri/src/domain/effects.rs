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
