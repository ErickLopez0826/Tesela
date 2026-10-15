//! Exclusiones (§6) y reglas de ventana (§16): qué ventanas no se gestionan y a dónde van las demás.

use serde::{Deserialize, Serialize};

use crate::domain::manageable::WindowFacts;

/// Normaliza un nombre de ejecutable para compararlo: minúsculas y sin `.exe`.
pub fn normalize_exe(exe: &str) -> String {
    let e = exe.trim().to_lowercase();
    e.strip_suffix(".exe").map(str::to_string).unwrap_or(e)
}

fn title_matches(needle: &Option<String>, title: &str) -> bool {
    match needle {
        None => true,
        Some(n) => title.to_lowercase().contains(&n.to_lowercase()),
    }
}

/// Una exclusión se guarda por ejecutable (§6.3). El título es un criterio adicional, nunca una regla
/// independiente (§6.7): si está presente, la ventana debe cumplir ambos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExclusionRule {
    pub exe: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
}

impl ExclusionRule {
    pub fn new(exe: &str, title_contains: Option<&str>) -> Option<ExclusionRule> {
        let exe = normalize_exe(exe);
        if exe.is_empty() {
            return None;
        }
        let title_contains = title_contains.map(str::trim).filter(|t| !t.is_empty()).map(String::from);
        Some(ExclusionRule { exe, title_contains })
    }

    pub fn matches(&self, f: &WindowFacts) -> bool {
        normalize_exe(&f.exe) == self.exe && title_matches(&self.title_contains, &f.title)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExclusionList {
    pub rules: Vec<ExclusionRule>,
}

impl ExclusionList {
    pub fn matches(&self, f: &WindowFacts) -> bool {
        self.rules.iter().any(|r| r.matches(f))
    }

    /// Agrega una exclusión; devuelve `false` si ya existía o es inválida.
    pub fn add(&mut self, exe: &str, title_contains: Option<&str>) -> bool {
        match ExclusionRule::new(exe, title_contains) {
            Some(rule) if !self.rules.contains(&rule) => {
                self.rules.push(rule);
                true
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, exe: &str, title_contains: Option<&str>) -> bool {
        let Some(rule) = ExclusionRule::new(exe, title_contains) else { return false };
        let before = self.rules.len();
        self.rules.retain(|r| *r != rule);
        self.rules.len() != before
    }
}

/// Regla de ventana (§16): mandar a un workspace y/o hacerla flotante centrada.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRule {
    pub exe: Option<String>,
    pub title_contains: Option<String>,
    /// Workspace 1 a 9.
    pub workspace: Option<u8>,
    #[serde(default)]
    pub float: bool,
}

impl WindowRule {
    pub fn matches(&self, f: &WindowFacts) -> bool {
        if self.exe.is_none() && self.title_contains.is_none() {
            return false;
        }
        let exe_ok = self.exe.as_ref().map(|e| normalize_exe(e) == normalize_exe(&f.exe)).unwrap_or(true);
        exe_ok && title_matches(&self.title_contains, &f.title)
    }
}

/// Resultado de aplicar todas las reglas a una ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuleOutcome {
    /// Índice 0..8 del workspace destino.
    pub workspace: Option<usize>,
    pub float: bool,
}

/// Se suman las reglas que coinciden (como en GlazeWM): el primer workspace indicado gana; basta una regla de flotante.
pub fn apply_rules(rules: &[WindowRule], f: &WindowFacts) -> RuleOutcome {
    let mut out = RuleOutcome::default();
    for r in rules.iter().filter(|r| r.matches(f)) {
        if out.workspace.is_none() {
            out.workspace = r.workspace.map(|w| (w as usize).saturating_sub(1));
        }
        out.float |= r.float;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(exe: &str, title: &str) -> WindowFacts {
        WindowFacts { handle: 1, exe: exe.into(), title: title.into(), ..Default::default() }
    }

    #[test]
    fn la_exclusion_es_por_ejecutable_sin_importar_mayusculas_ni_exe() {
        let mut l = ExclusionList::default();
        assert!(l.add("Overwatch.exe", None));
        assert!(l.matches(&facts("overwatch.EXE", "Cualquier título")));
        assert!(l.matches(&facts("Overwatch", "")));
        assert!(!l.matches(&facts("chrome.exe", "Overwatch")));
    }

    #[test]
    fn el_titulo_es_un_criterio_adicional_no_una_regla_independiente() {
        let mut l = ExclusionList::default();
        l.add("chrome.exe", Some("Picture in picture"));
        assert!(l.matches(&facts("chrome.exe", "Picture in picture")));
        assert!(l.matches(&facts("chrome.exe", "picture IN picture - video")));
        assert!(!l.matches(&facts("chrome.exe", "Google")), "mismo proceso, otra ventana: se gestiona");
        assert!(!l.matches(&facts("firefox.exe", "Picture in picture")), "el título solo no basta");
    }

    #[test]
    fn sin_duplicados_y_se_pueden_quitar() {
        let mut l = ExclusionList::default();
        assert!(l.add("a.exe", None));
        assert!(!l.add("A", None));
        assert!(!l.add("   ", None));
        assert!(l.add("a.exe", Some("x")));
        assert_eq!(l.rules.len(), 2);
        assert!(l.remove("a", None));
        assert!(!l.remove("a", None));
        assert_eq!(l.rules.len(), 1);
    }

    #[test]
    fn serializa_a_json_y_vuelve() {
        let mut l = ExclusionList::default();
        l.add("a.exe", None);
        l.add("b.exe", Some("t"));
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<ExclusionList>(&json).unwrap(), l);
    }

    #[test]
    fn reglas_de_ventana_mover_y_flotar() {
        let rules = vec![
            WindowRule { exe: Some("Code".into()), title_contains: None, workspace: Some(3), float: false },
            WindowRule { exe: None, title_contains: Some("dashboard".into()), workspace: Some(1), float: false },
            WindowRule { exe: Some("javaw".into()), title_contains: None, workspace: None, float: true },
        ];
        assert_eq!(apply_rules(&rules, &facts("Code.exe", "x")), RuleOutcome { workspace: Some(2), float: false });
        assert_eq!(apply_rules(&rules, &facts("chrome.exe", "Mi Dashboard")), RuleOutcome { workspace: Some(0), float: false });
        assert_eq!(apply_rules(&rules, &facts("javaw.exe", "Minecraft")), RuleOutcome { workspace: None, float: true });
        assert_eq!(apply_rules(&rules, &facts("otro.exe", "otro")), RuleOutcome::default());
    }

    #[test]
    fn una_regla_vacia_no_coincide_con_todo() {
        let r = WindowRule { exe: None, title_contains: None, workspace: Some(1), float: false };
        assert!(!r.matches(&facts("a.exe", "b")));
    }
}
