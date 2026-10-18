//! Infraestructura: adaptadores a Windows y al sistema de archivos.

pub mod config_file;
pub mod exclusion_store;
pub mod module_store;
pub mod notes_store;
pub mod startup_store;
pub mod tasks_store;
pub mod stats;

#[cfg(windows)]
pub mod windows;
