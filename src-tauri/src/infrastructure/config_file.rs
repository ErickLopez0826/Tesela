//! Lectura del archivo de configuración TOML (D-9).

use std::path::PathBuf;

use crate::domain::config::{Config, ValidConfig};

pub const DEFAULT_CONFIG: &str = include_str!("../../default-config.toml");

/// Carpeta de configuración (`WM_CONFIG_DIR` en pruebas; `%APPDATA%\Tesela` normalmente).
pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("WM_CONFIG_DIR") {
        return d.into();
    }
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    let base = PathBuf::from(base);
    migrate_legacy_dir(base.join("window-manager"), base.join("Tesela"))
}

/// La app se llamaba «window-manager»: si queda la carpeta antigua y aún no existe la nueva, se mueve entera para no
/// perder módulos, notas ni ajustes. Si no se puede mover (algo la tiene abierta), se sigue usando la antigua.
pub fn migrate_legacy_dir(old: PathBuf, new: PathBuf) -> PathBuf {
    if !new.exists() && old.exists() && std::fs::rename(&old, &new).is_err() {
        return old;
    }
    new
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn parse_config(text: &str) -> Result<ValidConfig, Vec<String>> {
    let config: Config = toml::from_str(text).map_err(|e| vec![format!("TOML inválido: {e}")])?;
    config.validate()
}

/// Lee y valida el archivo de configuración.
pub fn load_file() -> Result<ValidConfig, Vec<String>> {
    let path = config_path();
    let text = std::fs::read_to_string(&path).map_err(|e| vec![format!("no se pudo leer {}: {e}", path.display())])?;
    parse_config(&text)
}

/// Bloque que se añade al `config.toml` de quien ya lo tenía antes de que existiera el buscador (§18): el archivo por
/// defecto solo se escribe si no existe, así que sin esto el atajo nuevo nunca le llegaría.
const SEARCH_BINDING_BLOCK: &str = r#"
# Buscador de aplicaciones (§18): abre y cierra la ventana de búsqueda. Sigue activo con el gestor en pausa.
# Si otro programa ya usa esta combinación, cámbiala aquí y recarga con alt+shift+r.
# Para desactivarlo, deja la lista vacía: keys = []
[[keybinding]]
action = "toggle_search"
keys = ["ctrl+shift+space"]
"#;

/// Si el archivo es válido y no tiene aún ninguna acción `toggle_search`, devuelve su texto con el atajo del buscador
/// añadido al final (sin tocar nada más). Si la combinación ya la usa otra acción, o el archivo no se entiende, no hace nada.
pub fn with_search_binding(text: &str) -> Option<String> {
    if text.contains("toggle_search") {
        return None;
    }
    let valid = parse_config(text).ok()?;
    let chord = crate::domain::shortcuts::Chord::parse("ctrl+shift+space").expect("combinación válida");
    if valid.keymap.lookup("", &chord).is_some() {
        return None;
    }
    let mut out = text.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(SEARCH_BINDING_BLOCK);
    Some(out)
}

/// Pone al día un `config.toml` existente con las novedades que necesitan una entrada nueva. Devuelve `true` si lo cambió.
fn migrate_file(path: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else { return false };
    match with_search_binding(&text) {
        Some(new) => {
            // Se escribe a un temporal y se renombra: un corte a mitad no deja el archivo del usuario a medias.
            let tmp = path.with_extension("toml.tmp");
            std::fs::write(&tmp, new).is_ok() && std::fs::rename(&tmp, path).is_ok()
        }
        None => false,
    }
}

/// Arranque: crea el archivo con los valores por defecto si no existe y lo carga.
/// Si el archivo es inválido usa los valores por defecto y devuelve el aviso.
pub fn load_or_create() -> (ValidConfig, Option<String>) {
    let path = config_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, DEFAULT_CONFIG);
    } else if migrate_file(&path) {
        eprintln!("[wm] se añadió al config.toml el atajo del buscador (ctrl+shift+space)");
    }
    match load_file() {
        Ok(v) => (v, None),
        Err(errors) => {
            let default = parse_config(DEFAULT_CONFIG).expect("la configuración por defecto es válida");
            (default, Some(format!("Configuración inválida, se usan los valores por defecto: {}", errors.join(" | "))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::actions::Action;
    use crate::domain::layout::{Axis, Dir, Orientation};
    use crate::domain::shortcuts::Chord;

    #[test]
    fn la_carpeta_antigua_se_mueve_a_la_nueva_con_su_contenido_y_solo_una_vez() {
        let base = std::env::temp_dir().join(format!("tesela-migracion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (old, new) = (base.join("window-manager"), base.join("Tesela"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("config.toml"), "dato").unwrap();

        assert_eq!(migrate_legacy_dir(old.clone(), new.clone()), new);
        assert_eq!(std::fs::read_to_string(new.join("config.toml")).unwrap(), "dato");
        assert!(!old.exists());

        // Si la nueva ya existe no se toca (ni se mezcla con otra antigua que reaparezca).
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("config.toml"), "otro").unwrap();
        assert_eq!(migrate_legacy_dir(old.clone(), new.clone()), new);
        assert_eq!(std::fs::read_to_string(new.join("config.toml")).unwrap(), "dato");

        // Sin carpeta antigua tampoco hace nada.
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(migrate_legacy_dir(old, new.clone()), new);
        assert!(!new.exists());
    }

    fn default() -> ValidConfig {
        parse_config(DEFAULT_CONFIG).unwrap()
    }

    fn has(v: &ValidConfig, mode: &str, chord: &str, action: Action) {
        let c = Chord::parse(chord).unwrap();
        assert_eq!(v.keymap.lookup(mode, &c), Some(action), "{chord} en modo '{mode}'");
    }

    #[test]
    fn la_config_por_defecto_es_valida() {
        let v = default();
        assert_eq!(v.settings.vertical_axis, Axis::Rows);
        assert_eq!(v.settings.layout.gaps.top, 20);
        assert_eq!(v.settings.bindings[0], Some(0));
        assert_eq!(v.settings.bindings[4], Some(1));
        assert_eq!(v.settings.bindings[8], None);
    }

    /// La sección 8 del plan, escrita aparte a mano. Si el archivo por defecto se desvía, falla.
    #[test]
    fn los_atajos_por_defecto_son_exactamente_los_de_la_seccion_8() {
        use Action::*;
        let v = default();
        let mut expected: Vec<(String, Action)> = vec![];
        let mut add = |c: &str, a: Action| expected.push((c.to_string(), a));
        // 8.1 y 8.2
        for (key, arrow, dir) in [("h", "left", Dir::Left), ("l", "right", Dir::Right), ("k", "up", Dir::Up), ("j", "down", Dir::Down)] {
            add(&format!("alt+{key}"), FocusDir(dir));
            add(&format!("alt+{arrow}"), FocusDir(dir));
            add(&format!("alt+shift+{key}"), MoveDir(dir));
            add(&format!("alt+shift+{arrow}"), MoveDir(dir));
        }
        // 8.3
        add("alt+u", Resize { orientation: Orientation::X, percent: -2 });
        add("alt+p", Resize { orientation: Orientation::X, percent: 2 });
        add("alt+o", Resize { orientation: Orientation::Y, percent: 2 });
        add("alt+i", Resize { orientation: Orientation::Y, percent: -2 });
        // 8.4
        add("alt+r", EnterResizeMode);
        // 8.5
        for n in 1..=9u8 {
            add(&format!("alt+{n}"), FocusWorkspace(n));
            add(&format!("alt+shift+{n}"), MoveToWorkspace(n));
        }
        add("alt+s", NextWorkspace);
        add("alt+a", PrevWorkspace);
        add("alt+d", RecentWorkspace);
        add("alt+shift+a", MoveWorkspaceDir(Dir::Left));
        add("alt+shift+f", MoveWorkspaceDir(Dir::Right));
        add("alt+shift+d", MoveWorkspaceDir(Dir::Up));
        add("alt+shift+s", MoveWorkspaceDir(Dir::Down));
        // 8.6
        add("alt+v", ToggleStack);
        add("alt+t", ToggleFloating);
        add("alt+f", ToggleFullscreen);
        add("alt+m", ToggleMinimize);
        add("alt+shift+space", ToggleFloatingCentered);
        add("alt+shift+q", CloseWindow);
        add("alt+space", CycleFocus);
        // 8.7
        add("alt+enter", OpenTerminal);
        add("alt+shift+p", TogglePause);
        add("alt+shift+r", ReloadConfig);
        add("alt+shift+w", Redraw);
        add("alt+shift+e", Exit);
        // Fuera de §8: el plan pide abrir el panel "con un clic o un atajo" sin definir la tecla (D-33).
        add("alt+b", TogglePanel);
        // Fuera de §8: el buscador de aplicaciones (§18.2).
        add("ctrl+shift+space", ToggleSearch);

        assert_eq!(v.keymap.global().len(), expected.len(), "ni más ni menos atajos globales que la sección 8 (+ panel y buscador)");
        for (chord, action) in &expected {
            has(&v, "", chord, *action);
        }

        // Modo redimensionar (8.4).
        let mode = v.keymap.mode("resize").expect("modo resize");
        assert_eq!(mode.len(), 10);
        for k in ["h", "left"] {
            has(&v, "resize", k, Resize { orientation: Orientation::X, percent: -2 });
        }
        for k in ["l", "right"] {
            has(&v, "resize", k, Resize { orientation: Orientation::X, percent: 2 });
        }
        for k in ["k", "up"] {
            has(&v, "resize", k, Resize { orientation: Orientation::Y, percent: 2 });
        }
        for k in ["j", "down"] {
            has(&v, "resize", k, Resize { orientation: Orientation::Y, percent: -2 });
        }
        for k in ["escape", "enter"] {
            has(&v, "resize", k, ExitResizeMode);
        }
    }

    #[test]
    fn las_reglas_de_ventana_migradas_funcionan() {
        use crate::domain::manageable::WindowFacts;
        use crate::domain::rules::{apply_rules, RuleOutcome};
        let v = default();
        let f = |exe: &str, title: &str| WindowFacts { exe: exe.into(), title: title.into(), ..Default::default() };
        let out = |exe: &str, title: &str| apply_rules(&v.rules, &f(exe, title));
        assert_eq!(out("Code.exe", "x"), RuleOutcome { workspace: Some(2), float: false });
        assert_eq!(out("Discord.exe", "x"), RuleOutcome { workspace: Some(4), float: false });
        assert_eq!(out("chrome.exe", "Mi Dashboard"), RuleOutcome { workspace: Some(0), float: false });
        assert_eq!(out("steam.exe", "Steam"), RuleOutcome { workspace: Some(1), float: true });
        assert_eq!(out("javaw.exe", "Minecraft"), RuleOutcome { workspace: None, float: true });
        assert_eq!(out("notepad.exe", "x"), RuleOutcome::default());
    }

    /// El `config.toml` de quien lo creó antes del buscador: sin la acción `toggle_search`.
    fn old_config() -> String {
        let mut text = String::new();
        let mut skip = 0;
        for line in DEFAULT_CONFIG.lines() {
            if line.contains("Buscador de aplicaciones") {
                skip = 6;
            }
            if skip > 0 {
                skip -= 1;
                continue;
            }
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    #[test]
    fn la_config_antigua_de_prueba_no_tiene_el_buscador() {
        let old = old_config();
        assert!(!old.contains("toggle_search"));
        assert!(parse_config(&old).is_ok());
    }

    #[test]
    fn a_una_config_sin_buscador_se_le_anade_el_atajo_y_nada_mas() {
        let old = old_config();
        let new = with_search_binding(&old).expect("se añade");
        assert!(new.starts_with(&old), "lo anterior queda intacto");
        has(&parse_config(&new).unwrap(), "", "ctrl+shift+space", Action::ToggleSearch);
        assert_eq!(parse_config(&new).unwrap().keymap.global().len(), parse_config(&old).unwrap().keymap.global().len() + 1);
    }

    #[test]
    fn la_migracion_se_hace_una_sola_vez_y_respeta_lo_que_el_usuario_decidio() {
        let new = with_search_binding(&old_config()).unwrap();
        assert_eq!(with_search_binding(&new), None, "ya la tiene");
        // Si lo desactivó con la lista vacía, sigue desactivado.
        let disabled = old_config() + "[[keybinding]]\naction = \"toggle_search\"\nkeys = []\n";
        assert_eq!(with_search_binding(&disabled), None);
        // Si la combinación la usa otra acción suya, no se le quita.
        let taken = old_config().replace("alt+b", "ctrl+shift+space");
        assert_eq!(with_search_binding(&taken), None);
        // Un archivo que no se entiende no se toca.
        assert_eq!(with_search_binding("esto no es toml [["), None);
    }

    #[test]
    fn sin_salto_de_linea_final_el_bloque_no_se_pega_a_la_ultima_linea() {
        let old = old_config();
        let new = with_search_binding(old.trim_end()).unwrap();
        assert!(parse_config(&new).is_ok(), "{new}");
    }

    #[test]
    fn migrar_el_archivo_lo_reescribe_una_vez() {
        let dir = std::env::temp_dir().join(format!("tesela-cfg-migra-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, old_config()).unwrap();
        assert!(migrate_file(&path));
        assert!(std::fs::read_to_string(&path).unwrap().contains("toggle_search"));
        assert!(!dir.join("config.toml.tmp").exists());
        assert!(!migrate_file(&path), "la segunda vez no cambia nada");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn toml_mal_formado_da_error_en_vez_de_panico() {
        assert!(parse_config("esto no es toml [[").is_err());
    }

    #[test]
    fn campos_desconocidos_se_rechazan() {
        let err = parse_config("[layout]\nmin_column_widht = 100\n").unwrap_err();
        assert!(err[0].contains("min_column_widht"), "{err:?}");
    }

    #[test]
    fn atajos_duplicados_en_el_archivo_no_cargan_ninguno() {
        let text = r#"
[[keybinding]]
action = "focus_left"
keys = ["alt+h"]
[[keybinding]]
action = "move_left"
keys = ["alt+h"]
"#;
        let err = parse_config(text).unwrap_err();
        assert!(err.iter().any(|e| e.contains("alt+h")));
    }
}
