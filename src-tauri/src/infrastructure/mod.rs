//! Infraestructura: adaptadores a Windows y al sistema de archivos.

pub mod config_file;

#[cfg(windows)]
pub mod windows;
