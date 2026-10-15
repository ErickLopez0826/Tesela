use crate::domain::window::WindowCandidate;

/// Puerto: fuente de ventanas del sistema.
pub trait WindowEnumerator {
    fn enumerate(&self) -> Result<Vec<WindowCandidate>, String>;
}
