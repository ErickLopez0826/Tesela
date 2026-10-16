//! Notas rápidas de la app: se guardan en `notes.json` dentro de la carpeta de configuración y se pueden exportar a
//! `.md` o `.txt` en Documentos. No se guarda nada del portapapeles de forma automática.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::infrastructure::config_file::config_dir;

pub const MAX_NOTES: usize = 2000;
pub const MAX_NOTE_CHARS: usize = 20_000;
const MAX_EXPORT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub text: String,
    /// Milisegundos desde 1970.
    pub created: u64,
    pub updated: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    notes: Vec<Note>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub struct NotesStore {
    path: PathBuf,
    notes: Mutex<Vec<Note>>,
}

fn clean(text: &str) -> Result<String, String> {
    let t = text.replace("\r\n", "\n");
    let t = t.trim_matches(|c: char| c == '\n' || c == ' ' || c == '\t').to_string();
    if t.is_empty() {
        return Err("la nota está vacía".into());
    }
    if t.chars().count() > MAX_NOTE_CHARS {
        return Err(format!("la nota es demasiado larga (máximo {MAX_NOTE_CHARS} caracteres)"));
    }
    Ok(t)
}

impl NotesStore {
    pub fn default_path() -> PathBuf {
        config_dir().join("notes.json")
    }

    /// Carga el archivo; si no existe o está dañado empieza vacío (el dañado se conserva como `.bak`).
    pub fn load(path: PathBuf) -> NotesStore {
        let notes = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<File>(text.trim_start_matches('\u{feff}')) {
                Ok(f) => f.notes,
                Err(_) => {
                    let _ = std::fs::rename(&path, path.with_extension("json.bak"));
                    vec![]
                }
            },
            Err(_) => vec![],
        };
        NotesStore { path, notes: Mutex::new(notes) }
    }

    /// Las más recientes primero.
    pub fn list(&self) -> Vec<Note> {
        let mut v = self.notes.lock().unwrap().clone();
        v.sort_by(|a, b| b.updated.cmp(&a.updated).then(b.id.cmp(&a.id)));
        v
    }

    fn save(&self, notes: &[Note]) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&File { notes: notes.to_vec() }).map_err(|e| e.to_string())?;
        // Escritura atómica: primero a un archivo temporal y luego se renombra.
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    pub fn add(&self, text: &str) -> Result<Note, String> {
        let text = clean(text)?;
        let mut notes = self.notes.lock().unwrap();
        if notes.len() >= MAX_NOTES {
            return Err(format!("ya hay {MAX_NOTES} notas: borra alguna para añadir más"));
        }
        let now = now_ms();
        // Dos notas en el mismo milisegundo no deben compartir identificador.
        let mut n = now;
        while notes.iter().any(|x| x.id == format!("n{n}")) {
            n += 1;
        }
        let note = Note { id: format!("n{n}"), text, created: now, updated: now };
        notes.push(note.clone());
        self.save(&notes)?;
        Ok(note)
    }

    pub fn update(&self, id: &str, text: &str) -> Result<Note, String> {
        let text = clean(text)?;
        let mut notes = self.notes.lock().unwrap();
        let note = notes.iter_mut().find(|n| n.id == id).ok_or("esa nota ya no existe")?;
        note.text = text;
        note.updated = now_ms();
        let out = note.clone();
        self.save(&notes)?;
        Ok(out)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut notes = self.notes.lock().unwrap();
        let before = notes.len();
        notes.retain(|n| n.id != id);
        if notes.len() == before {
            return Err("esa nota ya no existe".into());
        }
        self.save(&notes)
    }
}

/// Nombre de archivo de exportación aceptable: letras, números, guiones, puntos y espacios, con extensión `.md` o `.txt`.
fn valid_export_name(name: &str) -> bool {
    let ext_ok = name.ends_with(".md") || name.ends_with(".txt");
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or("");
    ext_ok
        && !stem.is_empty()
        && stem.len() <= 80
        && stem.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.' | '(' | ')'))
        && !stem.starts_with('.')
}

/// Guarda `content` en `dir/name`. Si ya hay un archivo con ese nombre, añade « (2)», « (3)»… Nunca sobrescribe.
pub fn export_to(dir: &Path, name: &str, content: &str) -> Result<PathBuf, String> {
    if !valid_export_name(name) {
        return Err(format!("nombre de archivo no válido: {name}"));
    }
    if content.len() > MAX_EXPORT_BYTES {
        return Err("el texto a exportar es demasiado grande".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear la carpeta {}: {e}", dir.display()))?;
    let (stem, ext) = name.rsplit_once('.').unwrap();
    let mut path = dir.join(name);
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> (NotesStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("wm-notes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("notes.json");
        (NotesStore::load(path.clone()), path)
    }

    #[test]
    fn agrega_edita_y_borra_y_sobrevive_a_reiniciar() {
        let (s, path) = store("crud");
        let a = s.add("  primera \r\n").unwrap();
        assert_eq!(a.text, "primera");
        let b = s.add("segunda").unwrap();
        assert_ne!(a.id, b.id);
        let edited = s.update(&a.id, "primera, editada").unwrap();
        assert!(edited.updated >= a.updated);
        assert_eq!(s.list().first().unwrap().id, a.id, "la editada es la más reciente");
        s.delete(&b.id).unwrap();
        assert!(s.delete(&b.id).is_err());
        let again = NotesStore::load(path);
        assert_eq!(again.list().iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["primera, editada"]);
    }

    #[test]
    fn rechaza_vacias_y_demasiado_largas() {
        let (s, _) = store("limites");
        assert!(s.add("   \n ").is_err());
        assert!(s.add(&"x".repeat(MAX_NOTE_CHARS + 1)).is_err());
        assert!(s.add(&"x".repeat(MAX_NOTE_CHARS)).is_ok());
        assert!(s.update("nada", "texto").is_err());
    }

    #[test]
    fn un_archivo_danado_no_tumba_nada_y_se_conserva() {
        let (_, path) = store("dañado");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ esto no es json").unwrap();
        let s = NotesStore::load(path.clone());
        assert!(s.list().is_empty());
        assert!(path.with_extension("json.bak").is_file());
        s.add("sigue funcionando").unwrap();
    }

    #[test]
    fn exporta_sin_sobrescribir_y_valida_el_nombre() {
        let dir = std::env::temp_dir().join(format!("wm-notes-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p1 = export_to(&dir, "notas-2026-10-16.md", "uno").unwrap();
        let p2 = export_to(&dir, "notas-2026-10-16.md", "dos").unwrap();
        assert_ne!(p1, p2);
        assert_eq!(std::fs::read_to_string(&p1).unwrap(), "uno");
        assert!(p2.file_name().unwrap().to_string_lossy().contains("(2)"));
        for bad in ["../x.md", "a/b.md", "a\\b.txt", "notas.exe", ".md", "con:dos.md", ""] {
            assert!(export_to(&dir, bad, "x").is_err(), "{bad} debería rechazarse");
        }
    }
}
