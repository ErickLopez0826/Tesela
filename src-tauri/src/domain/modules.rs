//! Módulos instalables (§10): manifiesto, permisos, versiones y reglas de validación. Datos puros.
//!
//! Un módulo es una carpeta con `module.json` y una página web (HTML + JS + CSS) que se muestra en un
//! iframe aislado (sandbox, sin acceso directo a Windows ni a Tauri). Solo habla con el núcleo mediante
//! la API de `application::modules`, y el núcleo decide qué le permite según los permisos concedidos.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Versión actual del núcleo, para comprobar `min_core_version` de cada módulo.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lo que un módulo puede pedir (§10.2). Un módulo de mensajes no necesita Wi-Fi ni energía (§10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Leer workspaces, ventana enfocada y recibir sus eventos.
    Windows,
    /// Pedir acciones de energía (siguen sujetas al modo seguro del núcleo).
    Power,
    /// Conectarse a los servidores que el manifiesto declare en `network_hosts`.
    Network,
    /// Leer y escribir archivos en su propia carpeta de datos (nada fuera de ella).
    Files,
    /// Mostrar avisos al usuario.
    Notifications,
    /// Leer estadísticas del sistema (CPU, RAM...).
    System,
    /// Abrir aplicaciones (programas y accesos directos) en tu nombre.
    Launch,
    /// Leer el texto del portapapeles y copiar texto a él.
    Clipboard,
    /// Medir la velocidad de tu conexión (consume datos).
    Speedtest,
}

impl Permission {
    pub const ALL: [Permission; 9] = [
        Permission::Windows,
        Permission::Power,
        Permission::Network,
        Permission::Files,
        Permission::Notifications,
        Permission::System,
        Permission::Launch,
        Permission::Clipboard,
        Permission::Speedtest,
    ];

    /// Texto que se le muestra al usuario antes de instalar (§10.3).
    pub fn describe(&self) -> &'static str {
        match self {
            Permission::Windows => "Ver tus workspaces y la ventana enfocada",
            Permission::Power => "Pedir apagar, suspender, reiniciar o hibernar el equipo",
            Permission::Network => "Conectarse a internet (solo a los servidores que declara)",
            Permission::Files => "Guardar y leer archivos en su propia carpeta",
            Permission::Notifications => "Mostrarte avisos",
            Permission::System => "Ver el uso de CPU y memoria",
            Permission::Launch => "Abrir aplicaciones en tu nombre",
            Permission::Clipboard => "Leer y escribir el texto de tu portapapeles",
            Permission::Speedtest => "Medir la velocidad de tu conexión (usa datos)",
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = serde_json::to_string(self).unwrap_or_default();
        write!(f, "{}", s.trim_matches('"'))
    }
}

/// Versión `x.y.z` comparable. Solo números; sin etiquetas de preliberación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let mut it = s.trim().split('.');
        let v = Version(it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
        it.next().is_none().then_some(v)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub id: String,
    #[serde(default)]
    pub min_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDecl {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortcutSuggestion {
    pub action: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PanelEntry {
    pub title: String,
    /// Alto en píxeles del recuadro dentro del panel.
    #[serde(default = "default_panel_height")]
    pub height: u32,
}

fn default_panel_height() -> u32 {
    160
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct BarEntry {
    /// Ancho en píxeles del hueco en la barra.
    #[serde(default = "default_bar_width")]
    pub width: u32,
}

fn default_bar_width() -> u32 {
    120
}

/// Componentes de interfaz que aporta el módulo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UiEntries {
    pub panel: Option<PanelEntry>,
    pub bar: Option<BarEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_core_version: String,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Servidores permitidos si pide `network` (p. ej. `https://abc.supabase.co`, `wss://abc.supabase.co`).
    #[serde(default)]
    pub network_hosts: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub ui: UiEntries,
    #[serde(default)]
    pub actions: Vec<ActionDecl>,
    #[serde(default)]
    pub shortcuts: Vec<ShortcutSuggestion>,
}

fn default_entry() -> String {
    "index.html".into()
}

/// Máximo de módulos con el mismo id de dependencia, etc. no aplica; estos son los límites reales:
pub const MAX_ID_LEN: usize = 40;

pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    id.len() <= MAX_ID_LEN
        && chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Una ruta relativa segura dentro de la carpeta del módulo: sin `..`, sin rutas absolutas ni unidades.
pub fn is_safe_relative_path(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with(['/', '\\'])
        && !p.contains(':')
        && !p.contains('\0')
        && p.split(['/', '\\']).all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

fn is_valid_host(h: &str) -> bool {
    let scheme_ok = ["https://", "wss://"].iter().any(|s| h.starts_with(s));
    let rest = h.split("://").nth(1).unwrap_or("");
    scheme_ok
        && !rest.is_empty()
        && !rest.contains(['/', ' ', ';', ',', '\'', '"', '*'])
        && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':')
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Manifest, String> {
        serde_json::from_str(json).map_err(|e| format!("module.json inválido: {e}"))
    }

    pub fn version(&self) -> Version {
        Version::parse(&self.version).unwrap_or(Version(0, 0, 0))
    }

    /// Valida todo y devuelve todos los problemas a la vez. `core_version` es la del núcleo que instala.
    pub fn validate(&self, core_version: Version) -> Vec<String> {
        let mut errors = Vec::new();
        if !is_valid_id(&self.id) {
            errors.push(format!("id '{}' no válido (minúsculas, números y guiones, hasta {MAX_ID_LEN})", self.id));
        }
        for (field, value) in [("name", &self.name), ("author", &self.author), ("description", &self.description)] {
            if value.trim().is_empty() {
                errors.push(format!("falta '{field}'"));
            }
        }
        if Version::parse(&self.version).is_none() {
            errors.push(format!("version '{}' no es x.y.z", self.version));
        }
        match Version::parse(&self.min_core_version) {
            None => errors.push(format!("min_core_version '{}' no es x.y.z", self.min_core_version)),
            Some(min) if min > core_version => {
                errors.push(format!("necesita el núcleo {min} o superior (esta app es la {core_version})"))
            }
            _ => {}
        }
        if !is_safe_relative_path(&self.entry) {
            errors.push(format!("entry '{}' no es una ruta relativa segura", self.entry));
        }
        let unique: BTreeSet<_> = self.permissions.iter().collect();
        if unique.len() != self.permissions.len() {
            errors.push("permisos repetidos".into());
        }
        if self.permissions.contains(&Permission::Network) {
            if self.network_hosts.is_empty() {
                errors.push("pide 'network' pero no declara network_hosts".into());
            }
        } else if !self.network_hosts.is_empty() {
            errors.push("declara network_hosts sin pedir el permiso 'network'".into());
        }
        for h in &self.network_hosts {
            if !is_valid_host(h) {
                errors.push(format!("network_hosts: '{h}' no es válido (usa https://host o wss://host, sin comodines)"));
            }
        }
        if self.ui.panel.is_none() && self.ui.bar.is_none() {
            errors.push("no aporta ningún componente de interfaz (ui.panel o ui.bar)".into());
        }
        for d in &self.dependencies {
            if !is_valid_id(&d.id) {
                errors.push(format!("dependencia '{}' no válida", d.id));
            }
            if d.id == self.id {
                errors.push("un módulo no puede depender de sí mismo".into());
            }
            if d.min_version.as_deref().is_some_and(|v| Version::parse(v).is_none()) {
                errors.push(format!("dependencia '{}': min_version no es x.y.z", d.id));
            }
        }
        let action_ids: BTreeSet<_> = self.actions.iter().map(|a| a.id.as_str()).collect();
        if action_ids.len() != self.actions.len() {
            errors.push("acciones repetidas".into());
        }
        for a in &self.actions {
            if !is_valid_id(&a.id) {
                errors.push(format!("acción '{}' no válida", a.id));
            }
        }
        for s in &self.shortcuts {
            if !action_ids.contains(s.action.as_str()) {
                errors.push(format!("el atajo sugerido apunta a la acción '{}', que el módulo no declara", s.action));
            }
        }
        errors
    }

    /// Política de seguridad de contenido con la que el núcleo sirve este módulo, según lo CONCEDIDO.
    /// Sin `network` concedido no puede conectarse a ningún sitio.
    pub fn csp(&self, granted: &BTreeSet<Permission>) -> String {
        let connect = if granted.contains(&Permission::Network) && !self.network_hosts.is_empty() {
            self.network_hosts.join(" ")
        } else {
            "'none'".to_string()
        };
        format!(
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
             font-src 'self'; connect-src {connect}; base-uri 'none'; form-action 'none'; frame-src 'none'"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> Manifest {
        Manifest {
            id: "hello".into(),
            name: "Hola".into(),
            version: "1.0.0".into(),
            author: "Erick".into(),
            description: "Módulo de prueba".into(),
            min_core_version: "0.1.0".into(),
            permissions: vec![Permission::Windows, Permission::Files],
            network_hosts: vec![],
            dependencies: vec![],
            entry: "index.html".into(),
            ui: UiEntries { panel: Some(PanelEntry { title: "Hola".into(), height: 120 }), bar: None },
            actions: vec![ActionDecl { id: "saludar".into(), title: "Saludar".into() }],
            shortcuts: vec![ShortcutSuggestion { action: "saludar".into(), keys: vec!["alt+g".into()] }],
        }
    }

    fn core() -> Version {
        Version(0, 1, 0)
    }

    #[test]
    fn un_manifiesto_correcto_no_tiene_errores() {
        assert_eq!(sample().validate(core()), Vec::<String>::new());
    }

    #[test]
    fn versiones() {
        assert_eq!(Version::parse("1.2.3"), Some(Version(1, 2, 3)));
        assert!(Version::parse("1.2").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
        assert!(Version::parse("1.2.x").is_none());
        assert!(Version(1, 10, 0) > Version(1, 9, 9));
        assert_eq!(CORE_VERSION.split('.').count(), 3);
    }

    #[test]
    fn ids_validos() {
        for ok in ["hello", "mi-modulo", "a1", "0abc"] {
            assert!(is_valid_id(ok), "{ok}");
        }
        for bad in ["", "-x", "Hola", "con espacio", "a/b", "..", "x".repeat(41).as_str(), "ñu"] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn rutas_relativas_seguras() {
        for ok in ["index.html", "sub/a.js", "a\\b.css"] {
            assert!(is_safe_relative_path(ok), "{ok}");
        }
        for bad in ["", "../x", "a/../b", "/etc/passwd", "\\x", "C:\\x", "a//b", "./a", "a\0b"] {
            assert!(!is_safe_relative_path(bad), "{bad}");
        }
    }

    #[test]
    fn el_nucleo_debe_ser_suficiente() {
        let mut m = sample();
        m.min_core_version = "9.0.0".into();
        let e = m.validate(core());
        assert!(e.iter().any(|x| x.contains("necesita el núcleo 9.0.0")), "{e:?}");
    }

    #[test]
    fn network_exige_hosts_y_viceversa() {
        let mut m = sample();
        m.permissions.push(Permission::Network);
        assert!(m.validate(core()).iter().any(|e| e.contains("no declara network_hosts")));
        m.network_hosts = vec!["https://abc.supabase.co".into(), "wss://abc.supabase.co".into()];
        assert!(m.validate(core()).is_empty());
        m.permissions.retain(|p| *p != Permission::Network);
        assert!(m.validate(core()).iter().any(|e| e.contains("sin pedir el permiso")));
    }

    #[test]
    fn hosts_peligrosos_se_rechazan() {
        let mut m = sample();
        m.permissions.push(Permission::Network);
        for bad in ["http://x.com", "https://*.x.com", "https://x.com/ruta", "https://x.com; script-src *", "https://", "ftp://x"] {
            m.network_hosts = vec![bad.into()];
            assert!(!m.validate(core()).is_empty(), "'{bad}' debería rechazarse");
        }
    }

    #[test]
    fn otros_errores_se_acumulan() {
        let mut m = sample();
        m.id = "MAL ID".into();
        m.entry = "../fuera.html".into();
        m.version = "uno".into();
        m.ui = UiEntries::default();
        m.permissions = vec![Permission::Files, Permission::Files];
        m.shortcuts = vec![ShortcutSuggestion { action: "no-existe".into(), keys: vec!["alt+x".into()] }];
        m.dependencies = vec![Dependency { id: "mal id".into(), min_version: Some("x".into()) }];
        assert!(m.validate(core()).len() >= 7);
    }

    #[test]
    fn parseo_de_json_rechaza_campos_desconocidos_y_permisos_inventados() {
        let ok = r#"{"id":"a","name":"A","version":"1.0.0","author":"x","description":"d","min_core_version":"0.1.0","permissions":["windows"],"ui":{"panel":{"title":"A"}}}"#;
        let m = Manifest::parse(ok).unwrap();
        assert_eq!(m.entry, "index.html");
        assert_eq!(m.ui.panel.unwrap().height, 160);
        assert!(Manifest::parse(&ok.replace("\"windows\"", "\"root\"")).is_err());
        assert!(Manifest::parse(&ok.replace("\"id\"", "\"extra\":1,\"id\"")).is_err());
        assert!(Manifest::parse("no es json").is_err());
    }

    #[test]
    fn la_csp_depende_de_lo_concedido() {
        let mut m = sample();
        m.permissions.push(Permission::Network);
        m.network_hosts = vec!["https://abc.supabase.co".into()];
        let sin: BTreeSet<Permission> = [Permission::Files].into();
        let con: BTreeSet<Permission> = [Permission::Files, Permission::Network].into();
        assert!(m.csp(&sin).contains("connect-src 'none'"));
        assert!(m.csp(&con).contains("connect-src https://abc.supabase.co;"));
        assert!(m.csp(&con).contains("default-src 'none'"));
        assert!(!m.csp(&con).contains("unsafe-eval"));
        assert!(!m.csp(&sin).contains("script-src 'self' 'unsafe-inline'"), "sin scripts en línea");
    }

    #[test]
    fn nombres_de_permisos() {
        assert_eq!(Permission::Windows.to_string(), "windows");
        assert_eq!(Permission::Notifications.to_string(), "notifications");
        for p in Permission::ALL {
            assert!(!p.describe().is_empty());
        }
    }
}
