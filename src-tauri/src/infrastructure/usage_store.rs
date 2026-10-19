//! Historial de uso del buscador (§18.7): qué se abre y cuánto. Vive en la carpeta de estado de este equipo
//! (`search-usage.json`), no se envía a ningún sitio y se puede borrar desde Ajustes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::domain::search::{record_usage, UsageEntry};

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    entries: HashMap<String, UsageEntry>,
}

/// Segundos desde 1970.
pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct UsageStore {
    path: PathBuf,
    entries: Mutex<HashMap<String, UsageEntry>>,
}

impl UsageStore {
    #[cfg(windows)]
    pub fn default_path() -> PathBuf {
        crate::infrastructure::windows::platform::state_dir().join("search-usage.json")
    }

    /// Carga el archivo; si no existe empieza vacío, y si está dañado se conserva como `.bak` y empieza vacío.
    pub fn load(path: PathBuf) -> UsageStore {
        let entries = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<File>(text.trim_start_matches('\u{feff}')) {
                Ok(f) => f.entries,
                Err(_) => {
                    let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                    HashMap::new()
                }
            },
            Err(_) => HashMap::new(),
        };
        UsageStore { path, entries: Mutex::new(entries) }
    }

    pub fn snapshot(&self) -> HashMap<String, UsageEntry> {
        self.entries.lock().unwrap().clone()
    }

    fn save(&self, entries: &HashMap<String, UsageEntry>) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string(&File { entries: entries.clone() }).map_err(|e| e.to_string())?;
        // Escritura atómica: primero a un archivo temporal y luego se renombra.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    /// Anota que se abrió algo. Un fallo al guardar no impide abrirlo: el uso se queda solo en memoria hasta el próximo guardado.
    pub fn record(&self, key: &str) {
        self.record_at(key, now_secs());
    }

    pub fn record_at(&self, key: &str, now: u64) {
        let mut entries = self.entries.lock().unwrap();
        record_usage(&mut entries, key, now);
        let _ = self.save(&entries);
    }

    /// Borra todo el historial.
    pub fn clear(&self) -> Result<(), String> {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("no se pudo borrar el historial: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tesela-usage-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("search-usage.json")
    }

    #[test]
    fn sin_archivo_empieza_vacio() {
        let p = temp("vacio");
        assert!(UsageStore::load(p).snapshot().is_empty());
    }

    #[test]
    fn anotar_guarda_y_se_recupera_al_volver_a_abrir() {
        let p = temp("ida-y-vuelta");
        let store = UsageStore::load(p.clone());
        store.record_at("app:spotify", 100);
        store.record_at("app:spotify", 100);
        store.record_at("web:claude", 200);
        let again = UsageStore::load(p.clone()).snapshot();
        assert_eq!(again.len(), 2);
        assert!((again["app:spotify"].weight - 2.0).abs() < 1e-9);
        assert_eq!(again["web:claude"].last_used, 200);
        assert!(!p.with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn un_archivo_danado_se_guarda_como_bak_y_no_rompe_nada() {
        let p = temp("danado");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{ esto no").unwrap();
        let store = UsageStore::load(p.clone());
        assert!(store.snapshot().is_empty());
        assert!(p.with_extension("json.bak").exists());
        store.record_at("app:a", 1);
        assert_eq!(UsageStore::load(p.clone()).snapshot().len(), 1);
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn borrar_el_historial_lo_vacia_en_memoria_y_en_disco() {
        let p = temp("borrar");
        let store = UsageStore::load(p.clone());
        store.record_at("app:a", 1);
        assert!(p.exists());
        store.clear().unwrap();
        assert!(store.snapshot().is_empty());
        assert!(!p.exists());
        store.clear().unwrap();
        assert!(UsageStore::load(p.clone()).snapshot().is_empty());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
