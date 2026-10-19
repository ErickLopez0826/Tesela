//! Redirecciones y búsquedas web del buscador (§18.6), en `search.json` junto a la configuración.

use std::path::{Path, PathBuf};

use crate::domain::search::{validate_redirects, Redirect};
use crate::infrastructure::config_file::config_dir;

pub fn default_path() -> PathBuf {
    config_dir().join("search.json")
}

/// Lee la lista; sin archivo o dañado, está vacía (de fábrica no hay ninguna).
pub fn load(path: &Path) -> Vec<Redirect> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<Redirect>>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|list| validate_redirects(list).ok())
        .unwrap_or_default()
}

/// Valida y guarda la lista completa; devuelve la lista ya limpia.
pub fn save(path: &Path, list: Vec<Redirect>) -> Result<Vec<Redirect>, String> {
    let list = validate_redirects(list)?;
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
    use crate::domain::search::RedirectKind;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tesela-search-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("search.json")
    }

    fn redirect(word: &str, url: &str, kind: RedirectKind) -> Redirect {
        Redirect { word: word.into(), url: url.into(), kind }
    }

    #[test]
    fn sin_archivo_o_danado_no_hay_redirecciones() {
        let p = temp("vacio");
        assert!(load(&p).is_empty());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{ esto no").unwrap();
        assert!(load(&p).is_empty());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn un_archivo_con_una_direccion_peligrosa_no_se_carga() {
        let p = temp("peligrosa");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, r#"[{"word":"x","url":"javascript:alert(1)","kind":"redirect"}]"#).unwrap();
        assert!(load(&p).is_empty());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn guardar_y_leer_conserva_el_orden_y_el_tipo() {
        let p = temp("ida-y-vuelta");
        let saved = save(
            &p,
            vec![redirect("g", "https://www.google.com/search?q={q}", RedirectKind::Search), redirect("claude", "https://claude.com", RedirectKind::Redirect)],
        )
        .unwrap();
        assert_eq!(load(&p), saved);
        assert_eq!(saved[0].word, "g");
        assert_eq!(saved[1].kind, RedirectKind::Redirect);
        assert!(!p.with_extension("json.tmp").exists(), "no queda el archivo temporal");
        // Vaciarla también se guarda.
        assert!(save(&p, vec![]).unwrap().is_empty());
        assert!(load(&p).is_empty());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn una_lista_invalida_no_toca_lo_guardado() {
        let p = temp("invalida");
        save(&p, vec![redirect("claude", "https://claude.com", RedirectKind::Redirect)]).unwrap();
        assert!(save(&p, vec![redirect("mal", "file:///C:/x", RedirectKind::Redirect)]).is_err());
        assert_eq!(load(&p)[0].word, "claude");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}
