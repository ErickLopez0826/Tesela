//! Persistencia de la lista de exclusiones (§6.3). Se edita desde el panel de la interfaz, no a mano.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::domain::rules::{ExclusionList, ExclusionRule};
use crate::infrastructure::config_file::config_dir;

/// Exclusiones iniciales, migradas de las reglas `ignore` del config.yaml de GlazeWM (§16).
/// Solo las que se pueden expresar por ejecutable (con título como criterio adicional).
pub fn seed_exclusions() -> ExclusionList {
    let mut l = ExclusionList::default();
    for exe in [
        "zebar",
        "overwatch",
        "deadbydaylight-win64-shipping",
        "ds",
        "explorer",
        "rdr2",
        "launcher",
        "buckshotroulette",
        "marvelrivals",
        "cyberpunk2077",
        "bmgame",
        "valorant-win64-shipping",
        "ghostoftsushima",
        "robloxstudiobeta",
    ] {
        l.add(exe, None);
    }
    // Picture-in-Picture de navegadores: mismo proceso, otra ventana (§6.7).
    for browser in ["chrome", "msedge", "firefox"] {
        l.add(browser, Some("picture in picture"));
        l.add(browser, Some("picture-in-picture"));
    }
    l.add("powertoys", Some("- Peek"));
    l
}

pub struct ExclusionStore {
    path: PathBuf,
    list: Mutex<ExclusionList>,
}

impl ExclusionStore {
    pub fn default_path() -> PathBuf {
        config_dir().join("exclusions.json")
    }

    /// Carga el archivo; si no existe o está dañado, parte de la lista inicial y la guarda.
    pub fn load_or_seed(path: PathBuf) -> ExclusionStore {
        let list = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<ExclusionList>(&t).ok());
        let store = ExclusionStore { path, list: Mutex::new(list.clone().unwrap_or_else(seed_exclusions)) };
        if list.is_none() {
            let _ = store.save();
        }
        store
    }

    pub fn list(&self) -> ExclusionList {
        self.list.lock().unwrap().clone()
    }

    fn save(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&*self.list.lock().unwrap()).map_err(|e| e.to_string())?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&self.path, text).map_err(|e| format!("no se pudo guardar las exclusiones: {e}"))
    }

    pub fn add(&self, exe: &str, title_contains: Option<&str>) -> Result<ExclusionList, String> {
        if ExclusionRule::new(exe, title_contains).is_none() {
            return Err("el nombre del ejecutable está vacío".into());
        }
        self.list.lock().unwrap().add(exe, title_contains);
        self.save()?;
        Ok(self.list())
    }

    pub fn remove(&self, exe: &str, title_contains: Option<&str>) -> Result<ExclusionList, String> {
        self.list.lock().unwrap().remove(exe, title_contains);
        self.save()?;
        Ok(self.list())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::manageable::WindowFacts;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wm-excl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("exclusions.json")
    }

    fn facts(exe: &str, title: &str) -> WindowFacts {
        WindowFacts { exe: exe.into(), title: title.into(), ..Default::default() }
    }

    #[test]
    fn la_primera_vez_usa_la_lista_inicial_y_la_guarda() {
        let path = temp_path("seed");
        let store = ExclusionStore::load_or_seed(path.clone());
        assert!(store.list().matches(&facts("Overwatch.exe", "Overwatch")));
        assert!(store.list().matches(&facts("chrome.exe", "Picture in picture")));
        assert!(!store.list().matches(&facts("chrome.exe", "Gmail")));
        assert!(path.exists());
    }

    #[test]
    fn agregar_y_quitar_persisten() {
        let path = temp_path("crud");
        let store = ExclusionStore::load_or_seed(path.clone());
        store.add("MiJuego.exe", None).unwrap();
        assert!(ExclusionStore::load_or_seed(path.clone()).list().matches(&facts("mijuego", "x")));
        store.remove("mijuego", None).unwrap();
        assert!(!ExclusionStore::load_or_seed(path).list().matches(&facts("mijuego", "x")));
    }

    #[test]
    fn nombre_vacio_se_rechaza() {
        let store = ExclusionStore::load_or_seed(temp_path("vacio"));
        assert!(store.add("  ", None).is_err());
    }

    #[test]
    fn archivo_danado_vuelve_a_la_lista_inicial() {
        let path = temp_path("dañado");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ no es json").unwrap();
        let store = ExclusionStore::load_or_seed(path);
        assert!(store.list().matches(&facts("explorer.exe", "Descargas")));
    }
}
