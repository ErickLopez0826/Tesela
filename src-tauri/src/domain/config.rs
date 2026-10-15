//! Configuración editable (D-9): estructura, valores por defecto y validación.
//! La lectura del archivo TOML es de infraestructura; aquí no hay E/S.

use serde::Deserialize;

use crate::domain::desktop::{Settings, WORKSPACE_COUNT};
use crate::domain::layout::{Axis, Gaps, LayoutSettings};
use crate::domain::rules::WindowRule;
use crate::domain::shortcuts::{BindingSpec, Keymap};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LayoutConfig {
    pub min_column_width: i32,
    pub min_window_height: i32,
    pub inner_gap: i32,
    pub outer_top: i32,
    pub outer_right: i32,
    pub outer_bottom: i32,
    pub outer_left: i32,
    /// `columns` o `rows`.
    pub horizontal_axis: String,
    /// `columns` o `rows` (por defecto `rows`, ver DECISIONS.md).
    pub vertical_axis: String,
    pub float_width: f64,
    pub float_height: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        // Gaps del config.yaml de GlazeWM (§16): 20px entre ventanas y de margen. La barra reserva su
        // propio espacio como AppBar, así que arriba basta el margen de 20px (antes eran 60px para Zebar).
        LayoutConfig {
            min_column_width: 200,
            min_window_height: 120,
            inner_gap: 20,
            outer_top: 20,
            outer_right: 20,
            outer_bottom: 20,
            outer_left: 20,
            horizontal_axis: "columns".into(),
            vertical_axis: "rows".into(),
            float_width: 0.6,
            float_height: 0.7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    /// "1" a "9".
    pub name: String,
    /// Monitor al que está ligado (0 = el más a la izquierda).
    pub monitor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeybindingConfig {
    pub action: String,
    pub keys: Vec<String>,
    pub mode: Option<String>,
}

/// Ajustes de los módulos de sistema y de acción.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct SystemConfig {
    /// Si es `false` (por defecto), apagar/suspender/reiniciar/hibernar y cambiar Wi-Fi/Bluetooth/compartir
    /// red se simulan. Cambiarlo requiere reiniciar la app.
    pub allow_changes: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub layout: LayoutConfig,
    pub system: SystemConfig,
    #[serde(rename = "workspace")]
    pub workspaces: Vec<WorkspaceConfig>,
    #[serde(rename = "keybinding")]
    pub keybindings: Vec<KeybindingConfig>,
    #[serde(rename = "window_rule")]
    pub window_rules: Vec<WindowRule>,
}

fn parse_axis(name: &str, field: &str, errors: &mut Vec<String>) -> Axis {
    match name {
        "columns" => Axis::Columns,
        "rows" => Axis::Rows,
        other => {
            errors.push(format!("layout.{field}: '{other}' no es válido (usa 'columns' o 'rows')"));
            Axis::Columns
        }
    }
}

/// Configuración ya validada y convertida a los tipos del dominio.
#[derive(Debug, Clone)]
pub struct ValidConfig {
    pub settings: Settings,
    pub keymap: Keymap,
    pub rules: Vec<WindowRule>,
    pub system: SystemConfig,
}

impl Config {
    /// Valida todo y devuelve todos los errores a la vez; si hay alguno no se aplica nada.
    pub fn validate(&self) -> Result<ValidConfig, Vec<String>> {
        let mut errors = Vec::new();
        let l = &self.layout;
        if l.min_column_width < 1 || l.min_window_height < 1 {
            errors.push("layout: los mínimos deben ser positivos".into());
        }
        if [l.inner_gap, l.outer_top, l.outer_right, l.outer_bottom, l.outer_left].iter().any(|g| *g < 0) {
            errors.push("layout: los gaps no pueden ser negativos".into());
        }
        if !(0.1..=1.0).contains(&l.float_width) || !(0.1..=1.0).contains(&l.float_height) {
            errors.push("layout: float_width y float_height deben estar entre 0.1 y 1.0".into());
        }
        let horizontal_axis = parse_axis(&l.horizontal_axis, "horizontal_axis", &mut errors);
        let vertical_axis = parse_axis(&l.vertical_axis, "vertical_axis", &mut errors);

        let mut bindings = [None; WORKSPACE_COUNT];
        let mut seen = [false; WORKSPACE_COUNT];
        for w in &self.workspaces {
            match w.name.parse::<usize>() {
                Ok(n) if (1..=WORKSPACE_COUNT).contains(&n) => {
                    if seen[n - 1] {
                        errors.push(format!("workspace '{}' repetido", w.name));
                    }
                    seen[n - 1] = true;
                    bindings[n - 1] = w.monitor;
                }
                _ => errors.push(format!("workspace '{}': el nombre debe ser un número del 1 al 9", w.name)),
            }
        }

        for (i, r) in self.window_rules.iter().enumerate() {
            if r.exe.is_none() && r.title_contains.is_none() {
                errors.push(format!("window_rule #{}: necesita 'exe' o 'title_contains'", i + 1));
            }
            if let Some(w) = r.workspace {
                if !(1..=9).contains(&w) {
                    errors.push(format!("window_rule #{}: workspace {w} fuera de 1..9", i + 1));
                }
            }
            if r.workspace.is_none() && !r.float {
                errors.push(format!("window_rule #{}: no hace nada (falta 'workspace' o 'float')", i + 1));
            }
        }

        let specs: Vec<BindingSpec> = self
            .keybindings
            .iter()
            .map(|k| BindingSpec { action: k.action.clone(), keys: k.keys.clone(), mode: k.mode.clone() })
            .collect();
        let keymap = match Keymap::build(&specs) {
            Ok(k) => Some(k),
            Err(mut e) => {
                errors.append(&mut e);
                None
            }
        };

        match (errors.is_empty(), keymap) {
            (true, Some(keymap)) => Ok(ValidConfig {
                settings: Settings {
                    layout: LayoutSettings {
                        min_width: l.min_column_width,
                        min_height: l.min_window_height,
                        gaps: Gaps { inner: l.inner_gap, top: l.outer_top, right: l.outer_right, bottom: l.outer_bottom, left: l.outer_left },
                    },
                    horizontal_axis,
                    vertical_axis,
                    bindings,
                    float_size: (l.float_width, l.float_height),
                },
                keymap,
                rules: self.window_rules.clone(),
                system: self.system.clone(),
            }),
            _ => Err(errors),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(action: &str, keys: &[&str]) -> KeybindingConfig {
        KeybindingConfig { action: action.into(), keys: keys.iter().map(|s| s.to_string()).collect(), mode: None }
    }

    #[test]
    fn config_vacia_es_valida_con_valores_por_defecto() {
        let v = Config::default().validate().unwrap();
        assert_eq!(v.settings.layout.min_width, 200);
        assert_eq!(v.settings.vertical_axis, Axis::Rows);
        assert_eq!(v.settings.horizontal_axis, Axis::Columns);
        assert!(v.keymap.global().is_empty());
    }

    #[test]
    fn workspaces_ligados_a_monitores() {
        let c = Config {
            workspaces: vec![
                WorkspaceConfig { name: "1".into(), monitor: Some(0) },
                WorkspaceConfig { name: "5".into(), monitor: Some(1) },
                WorkspaceConfig { name: "9".into(), monitor: None },
            ],
            ..Default::default()
        };
        let b = c.validate().unwrap().settings.bindings;
        assert_eq!((b[0], b[4], b[8], b[1]), (Some(0), Some(1), None, None));
    }

    #[test]
    fn errores_se_acumulan() {
        let c = Config {
            layout: LayoutConfig { vertical_axis: "diagonal".into(), min_column_width: 0, ..Default::default() },
            workspaces: vec![WorkspaceConfig { name: "12".into(), monitor: None }],
            keybindings: vec![kb("focus_left", &["alt+h"]), kb("move_left", &["alt+h"])],
            ..Default::default()
        };
        let errs = c.validate().unwrap_err();
        assert!(errs.len() >= 4, "{errs:?}");
    }
}
