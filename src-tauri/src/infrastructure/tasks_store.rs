//! Lista de tareas, en `tasks.json` junto a la configuración. No se exporta con las notas.

use std::path::{Path, PathBuf};

use crate::domain::tasks::{validate, Task};
use crate::infrastructure::config_file::config_dir;

pub fn default_path() -> PathBuf {
    config_dir().join("tasks.json")
}

/// Lee la lista; sin archivo o dañado, está vacía.
pub fn load(path: &Path) -> Vec<Task> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<Task>>(&t).ok())
        .and_then(|list| validate(list).ok())
        .unwrap_or_default()
}

/// Valida y guarda la lista completa; devuelve la lista ya limpia.
pub fn save(path: &Path, list: Vec<Task>) -> Result<Vec<Task>, String> {
    let list = validate(list)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear la carpeta: {e}"))?;
    }
    let text = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    // Se escribe en un archivo aparte y se renombra: un corte a mitad de escritura no deja la lista rota.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("no se pudo guardar: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("no se pudo guardar: {e}"))?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tasks::Priority;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tesela-tasks-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("tasks.json")
    }

    fn task(id: &str, text: &str, priority: Priority) -> Task {
        Task { id: id.into(), text: text.into(), priority }
    }

    #[test]
    fn sin_archivo_o_dañado_no_hay_tareas() {
        let p = temp("vacio");
        assert!(load(&p).is_empty());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{ esto no").unwrap();
        assert!(load(&p).is_empty());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn guardar_y_leer_conserva_el_orden_y_la_prioridad() {
        let p = temp("ida-y-vuelta");
        let saved = save(&p, vec![task("2", "segunda", Priority::Low), task("1", "primera", Priority::High)]).unwrap();
        assert_eq!(load(&p), saved);
        assert_eq!(saved[0].id, "2");
        assert_eq!(saved[1].priority, Priority::High);
        assert!(!p.with_extension("json.tmp").exists(), "no queda el archivo temporal");
        // Vaciarla (todas hechas) también se guarda.
        assert!(save(&p, vec![]).unwrap().is_empty());
        assert!(load(&p).is_empty());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn una_lista_invalida_no_toca_lo_guardado() {
        let p = temp("invalida");
        save(&p, vec![task("1", "buena", Priority::Medium)]).unwrap();
        assert!(save(&p, vec![task("1", "  ", Priority::Medium)]).is_err());
        assert_eq!(load(&p)[0].text, "buena");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
