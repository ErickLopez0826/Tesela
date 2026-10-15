//! Infraestructura: adaptadores a Windows y al sistema de archivos.

pub mod config_file;
pub mod exclusion_store;

#[cfg(windows)]
pub mod windows;
