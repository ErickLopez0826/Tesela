//! Atajos de teclado (§8): parseo de combinaciones y mapa de atajos con validación.

use std::collections::BTreeMap;
use std::fmt;

use crate::domain::actions::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Letter(char),
    Digit(u8),
    F(u8),
    Left,
    Right,
    Up,
    Down,
    Space,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
}

impl Key {
    /// Código de tecla virtual de Windows.
    pub fn virtual_key(&self) -> u32 {
        match *self {
            Key::Letter(c) => c.to_ascii_uppercase() as u32,
            Key::Digit(d) => 0x30 + d as u32,
            Key::F(n) => 0x6F + n as u32,
            Key::Left => 0x25,
            Key::Up => 0x26,
            Key::Right => 0x27,
            Key::Down => 0x28,
            Key::Space => 0x20,
            Key::Enter => 0x0D,
            Key::Escape => 0x1B,
            Key::Tab => 0x09,
            Key::Backspace => 0x08,
            Key::Delete => 0x2E,
            Key::Insert => 0x2D,
            Key::Home => 0x24,
            Key::End => 0x23,
            Key::PageUp => 0x21,
            Key::PageDown => 0x22,
        }
    }

    fn name(&self) -> String {
        match self {
            Key::Letter(c) => c.to_string(),
            Key::Digit(d) => d.to_string(),
            Key::F(n) => format!("f{n}"),
            Key::Left => "left".into(),
            Key::Right => "right".into(),
            Key::Up => "up".into(),
            Key::Down => "down".into(),
            Key::Space => "space".into(),
            Key::Enter => "enter".into(),
            Key::Escape => "escape".into(),
            Key::Tab => "tab".into(),
            Key::Backspace => "backspace".into(),
            Key::Delete => "delete".into(),
            Key::Insert => "insert".into(),
            Key::Home => "home".into(),
            Key::End => "end".into(),
            Key::PageUp => "pageup".into(),
            Key::PageDown => "pagedown".into(),
        }
    }

    fn parse(token: &str) -> Option<Key> {
        let mut chars = token.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if c.is_ascii_lowercase() {
                return Some(Key::Letter(c));
            }
            if let Some(d) = c.to_digit(10) {
                return Some(Key::Digit(d as u8));
            }
        }
        if let Some(n) = token.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
            if (1..=24).contains(&n) {
                return Some(Key::F(n));
            }
        }
        Some(match token {
            "left" | "izquierda" => Key::Left,
            "right" | "derecha" => Key::Right,
            "up" | "arriba" => Key::Up,
            "down" | "abajo" => Key::Down,
            "space" | "espacio" => Key::Space,
            "enter" | "return" => Key::Enter,
            "escape" | "esc" => Key::Escape,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "insert" | "ins" => Key::Insert,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub mods: Mods,
    pub key: Key,
}

impl Chord {
    pub fn parse(text: &str) -> Result<Chord, String> {
        let mut mods = Mods::default();
        let mut key: Option<Key> = None;
        let lower = text.trim().to_lowercase();
        if lower.is_empty() {
            return Err("combinación vacía".into());
        }
        for token in lower.split('+').map(str::trim) {
            let slot = match token {
                "ctrl" | "control" => Some(&mut mods.ctrl),
                "alt" => Some(&mut mods.alt),
                "shift" => Some(&mut mods.shift),
                "win" | "super" | "meta" => Some(&mut mods.win),
                _ => None,
            };
            if let Some(flag) = slot {
                if *flag {
                    return Err(format!("modificador repetido en '{text}'"));
                }
                *flag = true;
            } else {
                let k = Key::parse(token).ok_or_else(|| format!("tecla desconocida '{token}' en '{text}'"))?;
                if key.replace(k).is_some() {
                    return Err(format!("'{text}' tiene más de una tecla"));
                }
            }
        }
        key.map(|key| Chord { mods, key }).ok_or_else(|| format!("'{text}' no tiene tecla"))
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = vec![];
        if self.mods.ctrl {
            parts.push("ctrl".into());
        }
        if self.mods.alt {
            parts.push("alt".into());
        }
        if self.mods.shift {
            parts.push("shift".into());
        }
        if self.mods.win {
            parts.push("win".into());
        }
        parts.push(self.key.name());
        write!(f, "{}", parts.join("+"))
    }
}

/// Una entrada del archivo de configuración, antes de validar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingSpec {
    pub action: String,
    pub keys: Vec<String>,
    /// `None` = atajos globales; `Some("resize")` = modo de teclado.
    pub mode: Option<String>,
}

/// Atajos validados, por modo. La clave vacía es el modo global.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Keymap {
    modes: BTreeMap<String, Vec<(Chord, Action)>>,
}

impl Keymap {
    /// Valida y construye. Si hay cualquier error no se carga nada (§13): devuelve todos los errores.
    pub fn build(specs: &[BindingSpec]) -> Result<Keymap, Vec<String>> {
        let mut errors = Vec::new();
        let mut modes: BTreeMap<String, Vec<(Chord, Action)>> = BTreeMap::new();
        for spec in specs {
            let mode = spec.mode.clone().unwrap_or_default();
            let Some(action) = Action::from_name(&spec.action) else {
                errors.push(format!("acción desconocida '{}'", spec.action));
                continue;
            };
            if spec.keys.is_empty() {
                errors.push(format!("la acción '{}' no tiene combinaciones", spec.action));
            }
            for k in &spec.keys {
                match Chord::parse(k) {
                    Ok(chord) => {
                        let list = modes.entry(mode.clone()).or_default();
                        if let Some((_, other)) = list.iter().find(|(c, _)| *c == chord) {
                            let where_ = if mode.is_empty() { String::new() } else { format!(" (modo '{mode}')") };
                            errors.push(format!(
                                "la combinación '{chord}' está asignada a '{}' y a '{}'{where_}",
                                other.name(),
                                action.name()
                            ));
                        } else {
                            list.push((chord, action));
                        }
                    }
                    Err(e) => errors.push(e),
                }
            }
        }
        if errors.is_empty() {
            Ok(Keymap { modes })
        } else {
            Err(errors)
        }
    }

    pub fn global(&self) -> &[(Chord, Action)] {
        self.modes.get("").map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn mode(&self, name: &str) -> Option<&[(Chord, Action)]> {
        if name.is_empty() {
            return None;
        }
        self.modes.get(name).map(Vec::as_slice)
    }

    /// Qué acción dispara una combinación en un modo ("" = global).
    pub fn lookup(&self, mode: &str, chord: &Chord) -> Option<Action> {
        self.modes.get(mode)?.iter().find(|(c, _)| c == chord).map(|(_, a)| *a)
    }

    /// Los atajos que siguen activos mientras el gestor está en pausa: el de pausa y el del buscador (no toca ventanas).
    pub fn pause_bindings(&self) -> Vec<(Chord, Action)> {
        self.global().iter().filter(|(_, a)| matches!(a, Action::TogglePause | Action::ToggleSearch)).copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(action: &str, keys: &[&str], mode: Option<&str>) -> BindingSpec {
        BindingSpec { action: action.into(), keys: keys.iter().map(|s| s.to_string()).collect(), mode: mode.map(String::from) }
    }

    #[test]
    fn parsea_combinaciones() {
        let c = Chord::parse("Alt+Shift+H").unwrap();
        assert!(c.mods.alt && c.mods.shift && !c.mods.ctrl);
        assert_eq!(c.key, Key::Letter('h'));
        assert_eq!(c.to_string(), "alt+shift+h");
    }

    #[test]
    fn el_orden_de_modificadores_no_importa() {
        assert_eq!(Chord::parse("shift+alt+h").unwrap(), Chord::parse("alt+shift+h").unwrap());
    }

    #[test]
    fn teclas_especiales_y_alias() {
        assert_eq!(Chord::parse("alt+izquierda").unwrap(), Chord::parse("alt+left").unwrap());
        assert_eq!(Chord::parse("alt+space").unwrap().key.virtual_key(), 0x20);
        assert_eq!(Chord::parse("alt+enter").unwrap().key.virtual_key(), 0x0D);
        assert_eq!(Chord::parse("escape").unwrap().key, Key::Escape);
        assert_eq!(Chord::parse("ctrl+alt+f12").unwrap().key.virtual_key(), 0x7B);
        assert_eq!(Chord::parse("alt+1").unwrap().key.virtual_key(), 0x31);
        assert_eq!(Chord::parse("alt+h").unwrap().key.virtual_key(), 0x48);
    }

    #[test]
    fn rechaza_combinaciones_invalidas() {
        for bad in ["", "alt", "alt+alt+h", "alt+h+j", "alt+ñ", "alt+f25", "alt+nada"] {
            assert!(Chord::parse(bad).is_err(), "'{bad}' debería fallar");
        }
    }

    #[test]
    fn una_accion_puede_tener_varias_combinaciones() {
        let km = Keymap::build(&[spec("focus_left", &["alt+h", "alt+left"], None)]).unwrap();
        assert_eq!(km.global().len(), 2);
        assert_eq!(km.lookup("", &Chord::parse("alt+left").unwrap()), Action::from_name("focus_left"));
    }

    #[test]
    fn misma_combinacion_en_dos_acciones_no_carga_ninguna() {
        let err = Keymap::build(&[spec("focus_left", &["alt+h"], None), spec("move_left", &["alt+h"], None)]).unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("alt+h") && err[0].contains("focus_left") && err[0].contains("move_left"), "{err:?}");
    }

    #[test]
    fn la_misma_combinacion_en_modos_distintos_es_valida() {
        let km = Keymap::build(&[spec("focus_left", &["h"], None), spec("resize_width_-2", &["h"], Some("resize"))]).unwrap();
        assert_eq!(km.mode("resize").unwrap().len(), 1);
    }

    #[test]
    fn acciones_y_teclas_desconocidas_se_reportan_todas() {
        let err = Keymap::build(&[spec("volar", &["alt+h"], None), spec("focus_left", &["alt+ñ"], None)]).unwrap_err();
        assert_eq!(err.len(), 2);
    }

    #[test]
    fn solo_los_atajos_de_pausa_y_buscador_siguen_activos_en_pausa() {
        let km = Keymap::build(&[spec("toggle_pause", &["alt+shift+p"], None), spec("focus_left", &["alt+h"], None)]).unwrap();
        assert_eq!(km.pause_bindings().len(), 1);
        let km = Keymap::build(&[spec("toggle_pause", &["alt+shift+p"], None), spec("toggle_search", &["ctrl+shift+space"], None), spec("focus_left", &["alt+h"], None)]).unwrap();
        assert_eq!(km.pause_bindings().len(), 2);
    }
}
