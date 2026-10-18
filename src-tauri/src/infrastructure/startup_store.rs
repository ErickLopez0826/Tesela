//! Lista de aplicaciones de arranque, en `startup-apps.json` junto a la configuración. Se edita desde Ajustes.

use std::path::{Path, PathBuf};

use crate::domain::startup::{validate, StartupApp};
use crate::infrastructure::config_file::config_dir;

pub fn default_path() -> PathBuf {
    config_dir().join("startup-apps.json")
}

/// Lee la lista; sin archivo o dañado, está vacía (no se abre nada por sorpresa).
pub fn load(path: &Path) -> Vec<StartupApp> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<StartupApp>>(&t).ok())
        .and_then(|list| validate(list).ok())
        .unwrap_or_default()
}

/// Valida y guarda la lista; devuelve la lista ya limpia.
pub fn save(path: &Path, list: Vec<StartupApp>) -> Result<Vec<StartupApp>, String> {
    let list = validate(list)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear la carpeta: {e}"))?;
    }
    let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("no se pudo guardar: {e}"))?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tesela-startup-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("startup-apps.json")
    }

    fn app(path: &str, ws: u8) -> StartupApp {
        StartupApp { name: String::new(), path: path.into(), exe: String::new(), workspace: ws }
    }

    #[test]
    fn sin_archivo_o_dañado_la_lista_esta_vacia() {
        let p = temp("vacio");
        assert!(load(&p).is_empty());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "esto no es json").unwrap();
        assert!(load(&p).is_empty());
    }

    #[test]
    fn guardar_y_volver_a_leer_conserva_la_lista_limpia() {
        let p = temp("ida-y-vuelta");
        let saved = save(&p, vec![app(r"C:\a\Spotify.exe", 3), app(r"C:\b\Code.exe", 1)]).unwrap();
        assert_eq!(load(&p), saved);
        assert_eq!(saved[0].exe, "spotify");
        assert_eq!(saved[0].workspace, 3);
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn una_lista_invalida_no_se_guarda() {
        let p = temp("invalida");
        assert!(save(&p, vec![app(r"C:\a.exe", 12)]).is_err());
        assert!(!p.exists());
    }
}
