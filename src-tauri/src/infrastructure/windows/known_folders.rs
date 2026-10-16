//! Carpetas conocidas de Windows (la de Documentos puede estar redirigida, p. ej. a OneDrive).

use std::path::PathBuf;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

pub fn documents() -> Result<PathBuf, String> {
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Documents, KF_FLAG_DEFAULT, None).map_err(|e| e.to_string())?;
        let text = p.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(p.0 as *const _));
        Ok(PathBuf::from(text?))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn la_carpeta_documentos_existe() {
        let d = super::documents().unwrap();
        println!("documentos: {}", d.display());
        assert!(d.is_dir());
    }
}
