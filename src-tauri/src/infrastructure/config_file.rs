//! Lectura del archivo de configuración TOML (D-9).

use std::path::PathBuf;

use crate::domain::config::{Config, ValidConfig};

pub const DEFAULT_CONFIG: &str = include_str!("../../default-config.toml");

/// Carpeta de configuración (`WM_CONFIG_DIR` en pruebas; `%APPDATA%\window-manager` normalmente).
pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("WM_CONFIG_DIR") {
        return d.into();
    }
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("window-manager")
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

/// Arranque: crea el archivo con los valores por defecto si no existe y lo carga.
/// Si el archivo es inválido usa los valores por defecto y devuelve el aviso.
pub fn load_or_create() -> (ValidConfig, Option<String>) {
    let path = config_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, DEFAULT_CONFIG);
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
        assert_eq!(v.settings.layout.gaps.top, 60);
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

        assert_eq!(v.keymap.global().len(), expected.len(), "ni más ni menos atajos globales que la sección 8");
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
