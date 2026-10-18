//! Ajustes de la interfaz de Windows que reemplaza Tesela, en `shell.json` junto a la configuración.

use std::path::{Path, PathBuf};

use crate::domain::shell::ShellSettings;
use crate::infrastructure::config_file::config_dir;

pub fn default_path() -> PathBuf {
    config_dir().join("shell.json")
}

/// Lee los ajustes; sin archivo o dañado, los de siempre.
pub fn load(path: &Path) -> ShellSettings {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(path: &Path, settings: ShellSettings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear la carpeta: {e}"))?;
    }
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("no se pudo guardar: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("no se pudo guardar: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tesela-shell-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("shell.json")
    }

    #[test]
    fn sin_archivo_o_danado_se_usan_los_de_siempre() {
        let p = temp("vacio");
        assert_eq!(load(&p), ShellSettings::default());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{ no").unwrap();
        assert_eq!(load(&p), ShellSettings::default());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn lo_guardado_se_lee_igual() {
        let p = temp("guardar");
        let s = ShellSettings { replace_taskbar: false, win_key_panel: false };
        save(&p, s).unwrap();
        assert_eq!(load(&p), s);
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
