//! Instalación, registro y archivos de los módulos (§10, Fase 7).
//!
//! Carpetas (bajo `%APPDATA%\window-manager`, o `WM_CONFIG_DIR` en pruebas):
//! - `modules\<id>\`      archivos del módulo (module.json, index.html, ...), de solo lectura para el módulo
//! - `module-data\<id>\`  carpeta propia que el módulo puede leer y escribir si tiene el permiso `files`
//! - `modules.json`       permisos concedidos y si está activo

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::domain::modules::{is_safe_relative_path, Manifest, Permission, Version};

/// Límites para que un módulo no pueda llenar el disco ni la memoria.
pub const MAX_MODULE_FILES: usize = 300;
pub const MAX_MODULE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_DATA_FILE_BYTES: u64 = 1024 * 1024;
pub const MAX_DATA_FILES: usize = 100;

pub const SDK_PATH: &str = "__wm/sdk.js";
const SDK_JS: &str = include_str!("../../module-sdk.js");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct RegistryEntry {
    granted: BTreeSet<Permission>,
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct Registry {
    modules: BTreeMap<String, RegistryEntry>,
}

/// Vista de un módulo instalado para la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstalledModule {
    pub manifest: Manifest,
    pub granted: Vec<Permission>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PermissionRequest {
    pub permission: Permission,
    pub description: String,
}

/// Lo que se le enseña al usuario ANTES de instalar (§10.3).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstallPreview {
    pub manifest: Manifest,
    pub permissions: Vec<PermissionRequest>,
    pub network_hosts: Vec<String>,
    /// Versión ya instalada si esto sería una actualización.
    pub installed_version: Option<String>,
    pub files: usize,
    pub bytes: u64,
}

/// Respuesta del protocolo `wmmodule://` (ya lista para devolverse al webview).
#[derive(Debug, Clone, PartialEq)]
pub struct Served {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub csp: Option<String>,
}

impl Served {
    fn not_found() -> Served {
        Served { status: 404, content_type: "text/plain; charset=utf-8", body: b"no encontrado".to_vec(), csp: None }
    }
}

pub fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub struct ModuleStore {
    modules_dir: PathBuf,
    data_dir: PathBuf,
    registry_path: PathBuf,
    core_version: Version,
    registry: Mutex<Registry>,
}

/// Recorre una carpeta devolviendo (ruta relativa con `/`, tamaño). Rechaza enlaces simbólicos.
fn scan(dir: &Path) -> Result<Vec<(String, u64)>, String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, u64)>) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("no se pudo leer {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
                return Err(format!("{} es un enlace simbólico; no se permiten", entry.path().display()));
            }
            let rel = entry.path().strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            if !is_safe_relative_path(&rel) {
                return Err(format!("nombre de archivo no permitido: {rel}"));
            }
            if meta.is_dir() {
                walk(base, &entry.path(), out)?;
            } else {
                out.push((rel, meta.len()));
            }
            if out.len() > MAX_MODULE_FILES {
                return Err(format!("demasiados archivos (máximo {MAX_MODULE_FILES})"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    Ok(out)
}

fn copy_tree(from: &Path, to: &Path, files: &[(String, u64)]) -> Result<(), String> {
    for (rel, _) in files {
        let dest = to.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(from.join(rel), dest).map_err(|e| format!("no se pudo copiar {rel}: {e}"))?;
    }
    Ok(())
}

impl ModuleStore {
    pub fn new(base: &Path, core_version: Version) -> ModuleStore {
        let registry_path = base.join("modules.json");
        let registry = std::fs::read_to_string(&registry_path)
            .ok()
            .and_then(|t| serde_json::from_str::<Registry>(&t).ok())
            .unwrap_or_default();
        ModuleStore {
            modules_dir: base.join("modules"),
            data_dir: base.join("module-data"),
            registry_path,
            core_version,
            registry: Mutex::new(registry),
        }
    }

    fn save(&self, registry: &Registry) -> Result<(), String> {
        if let Some(dir) = self.registry_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = self.registry_path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.registry_path).map_err(|e| e.to_string())
    }

    fn read_manifest(dir: &Path) -> Result<Manifest, Vec<String>> {
        let text = std::fs::read_to_string(dir.join("module.json"))
            .map_err(|e| vec![format!("no se encontró module.json en {}: {e}", dir.display())])?;
        Manifest::parse(&text).map_err(|e| vec![e])
    }

    // ---------- instalar ----------

    /// Revisa una carpeta de módulo sin instalar nada y devuelve lo que se le mostrará al usuario.
    pub fn preview(&self, source: &Path) -> Result<InstallPreview, Vec<String>> {
        let manifest = Self::read_manifest(source)?;
        let mut errors = manifest.validate(self.core_version);
        let files = scan(source).map_err(|e| vec![e])?;
        let bytes: u64 = files.iter().map(|f| f.1).sum();
        if bytes > MAX_MODULE_BYTES {
            errors.push(format!("pesa demasiado ({bytes} bytes; máximo {MAX_MODULE_BYTES})"));
        }
        if !files.iter().any(|(rel, _)| rel == &manifest.entry.replace('\\', "/")) {
            errors.push(format!("no existe el archivo de entrada '{}'", manifest.entry));
        }
        if files.iter().any(|(rel, _)| rel.starts_with("__wm/") || rel == "__wm") {
            errors.push("la carpeta __wm está reservada para el núcleo".into());
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let installed_version = self.installed_manifest(&manifest.id).map(|m| m.version);
        Ok(InstallPreview {
            permissions: manifest
                .permissions
                .iter()
                .map(|p| PermissionRequest { permission: *p, description: p.describe().into() })
                .collect(),
            network_hosts: manifest.network_hosts.clone(),
            installed_version,
            files: files.len(),
            bytes,
            manifest,
        })
    }

    fn dependency_errors(&self, manifest: &Manifest) -> Vec<String> {
        let mut errors = Vec::new();
        for d in &manifest.dependencies {
            match self.installed_manifest(&d.id) {
                None => errors.push(format!("necesita el módulo '{}', que no está instalado", d.id)),
                Some(m) => {
                    if let Some(min) = d.min_version.as_deref().and_then(Version::parse) {
                        if m.version() < min {
                            errors.push(format!("necesita '{}' {min} o superior (instalado: {})", d.id, m.version));
                        }
                    }
                }
            }
        }
        errors
    }

    /// Instala (o actualiza) un módulo con los permisos que el usuario concedió. No se fía de `preview`.
    pub fn install(&self, source: &Path, granted: &[Permission]) -> Result<InstalledModule, Vec<String>> {
        let preview = self.preview(source)?;
        let manifest = preview.manifest;
        let mut errors = self.dependency_errors(&manifest);
        for g in granted {
            if !manifest.permissions.contains(g) {
                errors.push(format!("se concedió '{g}', que el módulo no pidió"));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let files = scan(source).map_err(|e| vec![e])?;
        let final_dir = self.modules_dir.join(&manifest.id);
        let staging = self.modules_dir.join(format!(".{}.installing", manifest.id));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| vec![e.to_string()])?;
        if let Err(e) = copy_tree(source, &staging, &files) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(vec![e]);
        }
        let _ = std::fs::remove_dir_all(&final_dir);
        std::fs::rename(&staging, &final_dir).map_err(|e| vec![format!("no se pudo instalar: {e}")])?;

        let mut reg = self.registry.lock().unwrap();
        reg.modules.insert(manifest.id.clone(), RegistryEntry { granted: granted.iter().copied().collect(), enabled: true });
        self.save(&reg).map_err(|e| vec![e])?;
        Ok(InstalledModule { granted: reg.modules[&manifest.id].granted.iter().copied().collect(), enabled: true, manifest })
    }

    // ---------- desinstalar ----------

    /// Módulos instalados que dependen de `id`.
    pub fn dependents_of(&self, id: &str) -> Vec<String> {
        self.list().into_iter().filter(|m| m.manifest.dependencies.iter().any(|d| d.id == id)).map(|m| m.manifest.id).collect()
    }

    /// Desinstala por completo: archivos, datos propios y registro. No deja nada.
    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        // Se calculan antes de tomar el candado: `dependents_of` también lo necesita (no es reentrante).
        let dependents = self.dependents_of(id);
        let mut reg = self.registry.lock().unwrap();
        if !reg.modules.contains_key(id) {
            return Err(format!("el módulo '{id}' no está instalado"));
        }
        if !dependents.is_empty() {
            return Err(format!("no se puede desinstalar '{id}': lo usan {}", dependents.join(", ")));
        }
        reg.modules.remove(id);
        self.save(&reg)?;
        for dir in [self.modules_dir.join(id), self.data_dir.join(id)] {
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("no se pudo borrar {}: {e}", dir.display()))?;
            }
        }
        Ok(())
    }

    // ---------- consultas ----------

    fn installed_manifest(&self, id: &str) -> Option<Manifest> {
        if !self.registry.lock().unwrap().modules.contains_key(id) {
            return None;
        }
        Self::read_manifest(&self.modules_dir.join(id)).ok()
    }

    pub fn manifest(&self, id: &str) -> Option<Manifest> {
        self.installed_manifest(id)
    }

    pub fn list(&self) -> Vec<InstalledModule> {
        let entries = self.registry.lock().unwrap().modules.clone();
        entries
            .into_iter()
            .filter_map(|(id, e)| {
                Self::read_manifest(&self.modules_dir.join(&id))
                    .ok()
                    .map(|manifest| InstalledModule { manifest, granted: e.granted.into_iter().collect(), enabled: e.enabled })
            })
            .collect()
    }

    /// Permisos concedidos si el módulo existe y está activo.
    pub fn granted(&self, id: &str) -> Option<BTreeSet<Permission>> {
        self.registry.lock().unwrap().modules.get(id).filter(|e| e.enabled).map(|e| e.granted.clone())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut reg = self.registry.lock().unwrap();
        reg.modules.get_mut(id).ok_or_else(|| format!("el módulo '{id}' no está instalado"))?.enabled = enabled;
        self.save(&reg)
    }

    /// Cambia los permisos concedidos (el usuario puede revocar o conceder después de instalar).
    pub fn set_granted(&self, id: &str, granted: &[Permission]) -> Result<(), String> {
        let manifest = self.installed_manifest(id).ok_or_else(|| format!("el módulo '{id}' no está instalado"))?;
        if let Some(extra) = granted.iter().find(|g| !manifest.permissions.contains(g)) {
            return Err(format!("el módulo no pidió '{extra}'"));
        }
        let mut reg = self.registry.lock().unwrap();
        reg.modules.get_mut(id).unwrap().granted = granted.iter().copied().collect();
        self.save(&reg)
    }

    // ---------- archivos ----------

    /// Archivo de un módulo ACTIVO dentro de su carpeta (nunca fuera de ella).
    pub fn resolve_module_file(&self, id: &str, rel: &str) -> Option<PathBuf> {
        self.granted(id)?;
        if !is_safe_relative_path(rel) {
            return None;
        }
        let base = self.modules_dir.join(id).canonicalize().ok()?;
        let path = base.join(rel).canonicalize().ok()?;
        (path.starts_with(&base) && path.is_file()).then_some(path)
    }

    /// Respuesta del protocolo `wmmodule://<id>/<ruta>`: archivo del módulo + CSP según sus permisos concedidos.
    pub fn serve(&self, id: &str, rel: &str) -> Served {
        let (Some(granted), Some(manifest)) = (self.granted(id), self.installed_manifest(id)) else {
            return Served::not_found();
        };
        let csp = Some(manifest.csp(&granted));
        let rel = rel.trim_start_matches('/');
        let rel = if rel.is_empty() { manifest.entry.as_str() } else { rel };
        if rel == SDK_PATH {
            return Served { status: 200, content_type: mime_for("sdk.js"), body: SDK_JS.as_bytes().to_vec(), csp };
        }
        match self.resolve_module_file(id, rel).and_then(|p| std::fs::read(p).ok()) {
            Some(body) => Served { status: 200, content_type: mime_for(rel), body, csp },
            None => Served::not_found(),
        }
    }

    /// Carpeta de datos propia del módulo (se crea al usarla).
    pub fn data_dir_of(&self, id: &str) -> PathBuf {
        self.data_dir.join(id)
    }

    /// Ruta dentro de la carpeta de datos del módulo; rechaza cualquier intento de salirse de ella.
    pub fn resolve_data_path(&self, id: &str, rel: &str) -> Result<PathBuf, String> {
        if !is_safe_relative_path(rel) {
            return Err(format!("ruta no permitida: {rel}"));
        }
        let base = self.data_dir_of(id);
        let path = base.join(rel);
        // Aunque no exista, no puede contener componentes raros.
        if path.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) && !path.starts_with(&base) {
            return Err(format!("ruta no permitida: {rel}"));
        }
        Ok(path)
    }
}

impl crate::application::modules::ModuleHost for ModuleStore {
    fn granted(&self, id: &str) -> Option<BTreeSet<Permission>> {
        ModuleStore::granted(self, id)
    }

    fn display_name(&self, id: &str) -> Option<String> {
        self.manifest(id).map(|m| m.name)
    }

    fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        ModuleStore::set_enabled(self, id, enabled)
    }

    fn data_dir(&self, id: &str) -> PathBuf {
        self.data_dir_of(id)
    }

    fn resolve_data_path(&self, id: &str, rel: &str) -> Result<PathBuf, String> {
        ModuleStore::resolve_data_path(self, id, rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wm-mod-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn core() -> Version {
        Version(0, 1, 0)
    }

    fn manifest_json(id: &str, extra: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"Módulo {id}","version":"1.0.0","author":"Erick","description":"d","min_core_version":"0.1.0",
            "permissions":["windows","files"],"ui":{{"panel":{{"title":"{id}"}}}}{extra}}}"#
        )
    }

    fn make_module(dir: &Path, id: &str, extra: &str) -> PathBuf {
        let src = dir.join(format!("src-{id}"));
        fs::create_dir_all(src.join("assets")).unwrap();
        fs::write(src.join("module.json"), manifest_json(id, extra)).unwrap();
        fs::write(src.join("index.html"), "<h1>hola</h1>").unwrap();
        fs::write(src.join("assets/app.js"), "console.log(1)").unwrap();
        src
    }

    fn store(base: &Path) -> ModuleStore {
        ModuleStore::new(&base.join("app"), core())
    }

    fn all_files(dir: &Path) -> Vec<String> {
        if !dir.exists() {
            return vec![];
        }
        scan(dir).unwrap().into_iter().map(|f| f.0).collect()
    }

    #[test]
    fn la_vista_previa_muestra_los_permisos_sin_instalar_nada() {
        let base = tmp("preview");
        let src = make_module(&base, "hello", "");
        let s = store(&base);
        let p = s.preview(&src).unwrap();
        assert_eq!(p.manifest.id, "hello");
        assert_eq!(p.permissions.iter().map(|x| x.permission).collect::<Vec<_>>(), vec![Permission::Windows, Permission::Files]);
        assert!(p.permissions.iter().all(|x| !x.description.is_empty()));
        assert_eq!(p.files, 3);
        assert!(s.list().is_empty(), "no instaló nada");
        assert!(!base.join("app/modules").exists());
    }

    #[test]
    fn rechaza_modulos_invalidos_con_todos_los_motivos() {
        let base = tmp("invalid");
        let s = store(&base);
        // Sin module.json.
        let empty = base.join("vacio");
        fs::create_dir_all(&empty).unwrap();
        assert!(s.preview(&empty).is_err());
        // Falta el archivo de entrada y el id es inválido.
        let bad = base.join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("module.json"), manifest_json("MAL ID", "").replace("\"permissions\"", "\"entry\":\"falta.html\",\"permissions\"")).unwrap();
        let e = s.preview(&bad).unwrap_err();
        assert!(e.iter().any(|x| x.contains("id 'MAL ID'")) && e.iter().any(|x| x.contains("falta.html")), "{e:?}");
        // Carpeta reservada.
        let src = make_module(&base, "x", "");
        fs::create_dir_all(src.join("__wm")).unwrap();
        fs::write(src.join("__wm/sdk.js"), "malo").unwrap();
        assert!(s.preview(&src).unwrap_err().iter().any(|x| x.contains("__wm")));
    }

    #[test]
    fn instala_copia_los_archivos_y_registra_solo_lo_concedido() {
        let base = tmp("install");
        let src = make_module(&base, "hello", "");
        let s = store(&base);
        let m = s.install(&src, &[Permission::Windows]).unwrap();
        assert_eq!(m.granted, vec![Permission::Windows]);
        assert_eq!(all_files(&base.join("app/modules/hello")), vec!["assets/app.js", "index.html", "module.json"]);
        assert_eq!(s.granted("hello"), Some([Permission::Windows].into()));
        // Persistencia: otra instancia del almacén lo ve.
        let again = store(&base);
        assert_eq!(again.list().len(), 1);
        assert_eq!(again.list()[0].granted, vec![Permission::Windows]);
    }

    #[test]
    fn no_se_puede_conceder_lo_que_el_modulo_no_pidio() {
        let base = tmp("extra");
        let src = make_module(&base, "hello", "");
        let s = store(&base);
        let e = s.install(&src, &[Permission::Power]).unwrap_err();
        assert!(e[0].contains("power"), "{e:?}");
        assert!(s.list().is_empty());
        assert!(!base.join("app/modules/hello").exists(), "no quedó ni la carpeta");
    }

    #[test]
    fn dependencias_y_version_minima() {
        let base = tmp("deps");
        let s = store(&base);
        let dep = make_module(&base, "dependiente", r#","dependencies":[{"id":"base","min_version":"2.0.0"}]"#);
        let e = s.install(&dep, &[]).unwrap_err();
        assert!(e[0].contains("necesita el módulo 'base'"), "{e:?}");
        let base_src = make_module(&base, "base", "");
        s.install(&base_src, &[]).unwrap();
        let e = s.install(&dep, &[]).unwrap_err();
        assert!(e[0].contains("2.0.0"), "versión insuficiente: {e:?}");
        let newer = base.join("src-base2");
        fs::create_dir_all(&newer).unwrap();
        fs::write(newer.join("module.json"), manifest_json("base", "").replace("\"1.0.0\"", "\"2.1.0\"")).unwrap();
        fs::write(newer.join("index.html"), "x").unwrap();
        s.install(&newer, &[]).unwrap();
        s.install(&dep, &[]).unwrap();
        // Con dependientes no se puede desinstalar la base.
        assert!(s.uninstall("base").unwrap_err().contains("dependiente"));
        s.uninstall("dependiente").unwrap();
        s.uninstall("base").unwrap();
    }

    #[test]
    fn actualizar_reemplaza_los_archivos_y_conserva_los_datos() {
        let base = tmp("update");
        let src = make_module(&base, "hello", "");
        let s = store(&base);
        s.install(&src, &[Permission::Files]).unwrap();
        let data = s.data_dir_of("hello");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("estado.json"), "{}").unwrap();
        fs::write(src.join("nuevo.txt"), "n").unwrap();
        fs::remove_file(src.join("assets/app.js")).unwrap();
        s.install(&src, &[Permission::Files]).unwrap();
        assert_eq!(all_files(&base.join("app/modules/hello")), vec!["index.html", "module.json", "nuevo.txt"]);
        assert!(data.join("estado.json").exists(), "los datos del usuario sobreviven a una actualización");
        assert!(!all_files(&base.join("app/modules")).iter().any(|f| f.contains("installing")));
    }

    #[test]
    fn desinstalar_no_deja_nada() {
        let base = tmp("uninstall");
        let src = make_module(&base, "hello", "");
        let s = store(&base);
        s.install(&src, &[Permission::Files]).unwrap();
        let data = s.data_dir_of("hello");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("a.txt"), "x").unwrap();
        s.uninstall("hello").unwrap();
        assert!(s.list().is_empty());
        assert_eq!(s.granted("hello"), None);
        assert!(!base.join("app/modules/hello").exists());
        assert!(!data.exists(), "también se borran sus datos");
        assert!(store(&base).list().is_empty(), "y el registro persistido");
        assert!(s.uninstall("hello").is_err());
        assert_eq!(all_files(&base.join("app/modules")), Vec::<String>::new());
    }

    #[test]
    fn archivos_del_modulo_solo_dentro_de_su_carpeta_y_si_esta_activo() {
        let base = tmp("serve-paths");
        let src = make_module(&base, "hello", "");
        fs::write(base.join("app-secreto.txt"), "secreto").unwrap();
        let s = store(&base);
        s.install(&src, &[]).unwrap();
        assert!(s.resolve_module_file("hello", "index.html").is_some());
        assert!(s.resolve_module_file("hello", "assets/app.js").is_some());
        for bad in ["../hello/index.html", "..\\..\\app-secreto.txt", "/index.html", "C:\\Windows\\win.ini", "noexiste.js", "assets"] {
            assert!(s.resolve_module_file("hello", bad).is_none(), "'{bad}'");
        }
        assert!(s.resolve_module_file("otro", "index.html").is_none());
        s.set_enabled("hello", false).unwrap();
        assert!(s.resolve_module_file("hello", "index.html").is_none(), "un módulo desactivado no sirve archivos");
        assert_eq!(s.serve("hello", "index.html").status, 404);
    }

    #[test]
    fn el_protocolo_sirve_con_csp_segun_lo_concedido_y_el_sdk_del_nucleo() {
        let base = tmp("serve");
        let extra = r#","network_hosts":["https://abc.supabase.co"]"#;
        let src = make_module(&base, "red", extra);
        fs::write(src.join("module.json"), manifest_json("red", extra).replace("\"files\"", "\"files\",\"network\"")).unwrap();
        let s = store(&base);
        s.install(&src, &[Permission::Files]).unwrap();
        let r = s.serve("red", "");
        assert_eq!((r.status, r.content_type), (200, "text/html; charset=utf-8"));
        assert_eq!(r.body, b"<h1>hola</h1>");
        assert!(r.csp.as_deref().unwrap().contains("connect-src 'none'"), "sin network concedido no puede conectarse");
        let js = s.serve("red", "assets/app.js");
        assert_eq!(js.content_type, "text/javascript; charset=utf-8");
        let sdk = s.serve("red", SDK_PATH);
        assert_eq!(sdk.status, 200);
        assert!(String::from_utf8(sdk.body).unwrap().contains("window.wm"));
        assert_eq!(s.serve("red", "../x").status, 404);
        assert_eq!(s.serve("no-existe", "").status, 404);
        // Al conceder network, la CSP lo permite solo hacia el host declarado.
        s.set_granted("red", &[Permission::Files, Permission::Network]).unwrap();
        assert!(s.serve("red", "").csp.unwrap().contains("connect-src https://abc.supabase.co;"));
        assert!(s.set_granted("red", &[Permission::Power]).is_err());
    }

    #[test]
    fn carpeta_de_datos_sin_escapes() {
        let base = tmp("data");
        let s = store(&base);
        let ok = s.resolve_data_path("hello", "notas/a.txt").unwrap();
        assert!(ok.starts_with(s.data_dir_of("hello")));
        for bad in ["../otro/a.txt", "..", "/a", "C:\\a", "a/../../b", ""] {
            assert!(s.resolve_data_path("hello", bad).is_err(), "'{bad}'");
        }
    }

    #[test]
    fn limites_de_tamano() {
        let base = tmp("limits");
        let src = make_module(&base, "grande", "");
        fs::write(src.join("enorme.bin"), vec![0u8; (MAX_MODULE_BYTES + 1) as usize]).unwrap();
        let e = store(&base).preview(&src).unwrap_err();
        assert!(e.iter().any(|x| x.contains("pesa demasiado")), "{e:?}");
    }
}
