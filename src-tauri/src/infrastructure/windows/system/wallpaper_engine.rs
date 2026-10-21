//! Fondos de Wallpaper Engine: lista de la biblioteca (Steam Workshop y proyectos propios), cuál está puesto en cada
//! monitor y cambio de fondo.
//!
//! El cambio no toca nada de Windows ni la configuración de Wallpaper Engine: se le pide a su propia aplicación con su
//! línea de comandos (`-control openWallpaper`), así el cambio lo hace él y con su transición. Solo se aceptan fondos
//! de la lista (nada de rutas libres).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde_json::Value;

use crate::application::system::EngineControl;
use crate::domain::system::{ActionOutcome, EngineCurrent, EngineLibrary, EngineWallpaper};

const STEAM_DEFAULT: &str = r"C:\Program Files (x86)\Steam";
/// Dónde guarda Tesela la carpeta de Wallpaper Engine que indicó la persona (cuando no está en la ruta de siempre).
const SETTINGS_FILE: &str = "wallpaper-engine.json";
/// Identificador de Wallpaper Engine en Steam.
const APP_ID: &str = "431960";

pub struct WallpaperEngine {
    install: PathBuf,
    /// Carpetas con fondos: Workshop, proyectos de ejemplo y proyectos propios.
    sources: Vec<PathBuf>,
    cache: Mutex<Option<(Vec<Option<SystemTime>>, Vec<EngineWallpaper>)>>,
}

fn normalize(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn json_text(bytes: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Un fondo a partir de su carpeta: `project.json` con título, tipo y previsualización.
pub fn read_project(dir: &Path) -> Option<EngineWallpaper> {
    let json = json_text(&std::fs::read(dir.join("project.json")).ok()?)?;
    let title = json.get("title").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty());
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let preview = json
        .get("preview")
        .and_then(Value::as_str)
        // Solo un archivo dentro de la propia carpeta del fondo.
        .filter(|f| !f.contains(['/', '\\', ':']) && !f.is_empty())
        .map(|f| dir.join(f))
        .filter(|p| p.is_file())
        .map(|p| normalize(&p));
    Some(EngineWallpaper {
        id: normalize(dir),
        title: title.map(str::to_string).unwrap_or(folder),
        kind: json.get("type").and_then(Value::as_str).unwrap_or("").to_lowercase(),
        preview,
    })
}

/// Fondos puestos ahora, de la sección `selectedwallpapers` de la configuración de Wallpaper Engine.
pub fn parse_current(config: &Value) -> Vec<EngineCurrent> {
    let mut out = Vec::new();
    // La configuración cuelga de un objeto con el nombre del usuario de Windows; se busca la clave donde esté.
    fn find<'a>(v: &'a Value, key: &str, depth: usize) -> Option<&'a Value> {
        if depth > 4 {
            return None;
        }
        let obj = v.as_object()?;
        if let Some(found) = obj.get(key) {
            return Some(found);
        }
        obj.values().find_map(|c| find(c, key, depth + 1))
    }
    let Some(selected) = find(config, "wallpaperconfig", 0).and_then(|c| c.get("selectedwallpapers")).and_then(Value::as_object) else {
        return out;
    };
    for (name, entry) in selected {
        let Some(n) = name.strip_prefix("Monitor").and_then(|n| n.parse::<u32>().ok()) else { continue };
        let Some(file) = entry.get("file").and_then(Value::as_str) else { continue };
        let dir = Path::new(&file.replace('\\', "/")).parent().map(|p| p.to_string_lossy().replace('\\', "/"));
        if let Some(id) = dir {
            out.push(EngineCurrent { monitor: n, id });
        }
    }
    out.sort_by_key(|c| c.monitor);
    out
}

pub fn settings_file() -> PathBuf {
    crate::infrastructure::config_file::config_dir().join(SETTINGS_FILE)
}

/// La carpeta guardada por la persona, si el archivo existe y es válido.
pub fn read_saved_path(file: &Path) -> Option<PathBuf> {
    let value = json_text(&std::fs::read(file).ok()?)?;
    value.get("path").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty()).map(PathBuf::from)
}

pub fn write_saved_path(file: &Path, install: &Path) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::json!({ "path": install.to_string_lossy() }).to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

fn has_exe(dir: &Path) -> bool {
    ["wallpaper64.exe", "wallpaper32.exe"].iter().any(|n| dir.join(n).is_file())
}

/// De lo que escribió la persona saca la carpeta de instalación de Wallpaper Engine. Acepta la propia carpeta, el
/// ejecutable, o la carpeta de Steam / su biblioteca (de donde cuelga `steamapps\common\wallpaper_engine`).
pub fn resolve_install(input: &str) -> Result<PathBuf, String> {
    let text = input.trim().trim_matches('"').trim();
    if text.is_empty() {
        return Err("Escribe la carpeta donde está instalado Wallpaper Engine".into());
    }
    let mut base = PathBuf::from(text);
    if base.is_file() {
        if let Some(parent) = base.parent() {
            base = parent.to_path_buf();
        }
    }
    let candidates = [
        base.clone(),
        base.join("steamapps").join("common").join("wallpaper_engine"),
        base.join("common").join("wallpaper_engine"),
        base.join("wallpaper_engine"),
    ];
    candidates
        .into_iter()
        .find(|c| has_exe(c))
        .ok_or_else(|| format!("No hay Wallpaper Engine en «{text}»: falta wallpaper64.exe en esa carpeta"))
}

/// La carpeta de Steam (o de su biblioteca) de la que cuelga la instalación; si no cuelga de ninguna, la de siempre.
fn steam_root_of(install: &Path) -> PathBuf {
    let steamapps = install.parent().filter(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("common"))).and_then(Path::parent);
    steamapps.filter(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(STEAM_DEFAULT))
}

impl WallpaperEngine {
    /// Wallpaper Engine en la carpeta que indicó la persona o, si no, en la instalación estándar de Steam; `None` si no está.
    pub fn detect() -> Option<Self> {
        if let Some(saved) = read_saved_path(&settings_file()).and_then(|p| resolve_install(&p.to_string_lossy()).ok()) {
            return Some(Self::from_install(saved));
        }
        let install = PathBuf::from(STEAM_DEFAULT).join("steamapps").join("common").join("wallpaper_engine");
        has_exe(&install).then(|| Self::from_install(install))
    }

    /// A partir de una carpeta de instalación ya comprobada.
    pub fn from_install(install: PathBuf) -> Self {
        let steam = steam_root_of(&install);
        Self::at(install, steam)
    }

    fn at(install: PathBuf, steam: PathBuf) -> Self {
        let sources = vec![
            steam.join("steamapps").join("workshop").join("content").join(APP_ID),
            install.join("projects").join("defaultprojects"),
            install.join("projects").join("myprojects"),
        ];
        WallpaperEngine { install, sources, cache: Mutex::new(None) }
    }

    /// Carpetas que la interfaz puede leer (para el protocolo de archivos).
    pub fn source_dirs(&self) -> &[PathBuf] {
        &self.sources
    }

    fn stamps(&self) -> Vec<Option<SystemTime>> {
        self.sources.iter().map(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok()).collect()
    }

    fn items(&self) -> Vec<EngineWallpaper> {
        let stamps = self.stamps();
        let mut cache = self.cache.lock().unwrap();
        if let Some((s, items)) = cache.as_ref() {
            if *s == stamps {
                return items.clone();
            }
        }
        let mut items: Vec<EngineWallpaper> = Vec::new();
        for source in &self.sources {
            let Ok(entries) = std::fs::read_dir(source) else { continue };
            for e in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
                if let Some(w) = read_project(&e.path()) {
                    items.push(w);
                }
            }
        }
        items.sort_by_key(|w| w.title.to_lowercase());
        *cache = Some((stamps, items.clone()));
        items
    }

    fn current(&self) -> Vec<EngineCurrent> {
        std::fs::read(self.install.join("config.json")).ok().and_then(|b| json_text(&b)).map(|c| parse_current(&c)).unwrap_or_default()
    }

    fn exe(&self) -> Result<PathBuf, String> {
        ["wallpaper64.exe", "wallpaper32.exe"]
            .iter()
            .map(|n| self.install.join(n))
            .find(|p| p.is_file())
            .ok_or_else(|| "no se encuentra Wallpaper Engine".to_string())
    }
}

impl EngineControl for WallpaperEngine {
    fn library(&mut self) -> EngineLibrary {
        EngineLibrary { available: true, items: self.items(), current: self.current() }
    }

    fn apply(&mut self, id: &str, monitor: Option<u32>) -> Result<ActionOutcome, String> {
        let item = self.items().into_iter().find(|w| w.id == id).ok_or("ese fondo ya no está en la biblioteca")?;
        let project = Path::new(&item.id.replace('/', "\\")).join("project.json");
        let targets: Vec<Option<u32>> = match monitor {
            Some(m) => vec![Some(m)],
            None => {
                let known: Vec<u32> = self.current().iter().map(|c| c.monitor).collect();
                if known.is_empty() { vec![None] } else { known.into_iter().map(Some).collect() }
            }
        };
        let exe = self.exe()?;
        for target in targets {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new(&exe);
            cmd.current_dir(&self.install).args(["-control", "openWallpaper", "-file"]).arg(&project);
            if let Some(m) = target {
                cmd.arg("-monitor").arg(m.to_string());
            }
            cmd.creation_flags(0x0800_0000).spawn().map_err(|e| format!("no se pudo pedir el cambio a Wallpaper Engine: {e}"))?;
        }
        Ok(ActionOutcome::done(format!("Fondo «{}» enviado a Wallpaper Engine", item.title)))
    }

    fn current_preview(&mut self) -> Option<String> {
        preview_of_current(&self.current(), &self.items())
    }

    fn current_preview_for(&mut self, monitor: u32) -> Option<String> {
        preview_of_monitor(&self.current(), &self.items(), monitor)
    }

    fn current(&mut self) -> Vec<EngineCurrent> {
        WallpaperEngine::current(self)
    }
}

/// Previsualización del fondo del monitor 0 (o del primero que haya), si la biblioteca la conoce.
pub fn preview_of_current(current: &[EngineCurrent], items: &[EngineWallpaper]) -> Option<String> {
    let mut sorted: Vec<&EngineCurrent> = current.iter().collect();
    sorted.sort_by_key(|c| c.monitor);
    sorted.iter().find_map(|c| items.iter().find(|w| w.id == c.id).and_then(|w| w.preview.clone()))
}

/// Previsualización del fondo del monitor indicado; si ese monitor no tiene una conocida, la de `preview_of_current`.
pub fn preview_of_monitor(current: &[EngineCurrent], items: &[EngineWallpaper], monitor: u32) -> Option<String> {
    current
        .iter()
        .find(|c| c.monitor == monitor)
        .and_then(|c| items.iter().find(|w| w.id == c.id))
        .and_then(|w| w.preview.clone())
        .or_else(|| preview_of_current(current, items))
}

/// Cuando Wallpaper Engine no está instalado.
pub struct NoEngine;

impl EngineControl for NoEngine {
    fn library(&mut self) -> EngineLibrary {
        EngineLibrary::default()
    }
    fn apply(&mut self, _: &str, _: Option<u32>) -> Result<ActionOutcome, String> {
        Err("Wallpaper Engine no está instalado".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wm-we-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn project(root: &Path, folder: &str, json: &str, preview: Option<&str>) {
        let d = root.join(folder);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("project.json"), json).unwrap();
        if let Some(p) = preview {
            std::fs::write(d.join(p), b"x").unwrap();
        }
    }

    fn fake_install(root: &Path) -> PathBuf {
        let install = root.join("steamapps").join("common").join("wallpaper_engine");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("wallpaper64.exe"), b"x").unwrap();
        install
    }

    #[test]
    fn la_ruta_se_acepta_como_carpeta_ejecutable_o_biblioteca_de_steam() {
        let root = temp("ruta");
        let install = fake_install(&root);
        for input in [
            install.to_string_lossy().to_string(),
            format!("  \"{}\"  ", install.display()),
            install.join("wallpaper64.exe").to_string_lossy().to_string(),
            root.to_string_lossy().to_string(),
            root.join("steamapps").to_string_lossy().to_string(),
        ] {
            assert_eq!(resolve_install(&input).unwrap(), install, "{input}");
        }
        assert!(resolve_install("").unwrap_err().contains("Escribe la carpeta"));
        assert!(resolve_install(&root.join("otra").to_string_lossy()).unwrap_err().contains("falta wallpaper64.exe"));
    }

    #[test]
    fn la_ruta_elegida_se_guarda_y_se_lee_y_un_archivo_roto_se_ignora() {
        let root = temp("guardada");
        let file = root.join("ajustes").join("wallpaper-engine.json");
        assert_eq!(read_saved_path(&file), None);
        write_saved_path(&file, Path::new(r"D:\Juegos\Steam\steamapps\common\wallpaper_engine")).unwrap();
        assert_eq!(read_saved_path(&file), Some(PathBuf::from(r"D:\Juegos\Steam\steamapps\common\wallpaper_engine")));
        std::fs::write(&file, "{ roto").unwrap();
        assert_eq!(read_saved_path(&file), None);
    }

    #[test]
    fn los_fondos_se_buscan_en_la_biblioteca_de_steam_de_la_instalacion() {
        let root = temp("biblioteca-steam");
        let install = fake_install(&root);
        let engine = WallpaperEngine::from_install(install.clone());
        assert!(engine.source_dirs().contains(&root.join("steamapps").join("workshop").join("content").join(APP_ID)));
        assert!(engine.source_dirs().contains(&install.join("projects").join("myprojects")));
        // Una instalación suelta (sin Steam) sigue sin romperse: usa la ruta de Steam de siempre.
        assert_eq!(steam_root_of(Path::new(r"E:\Portable\WE")), PathBuf::from(STEAM_DEFAULT));
    }

    #[test]
    fn lee_titulo_tipo_y_preview_aceptando_bom() {
        let root = temp("proyecto");
        project(&root, "123", "\u{feff}{\"title\":\"  Lluvia \",\"type\":\"Scene\",\"preview\":\"preview.gif\"}", Some("preview.gif"));
        let w = read_project(&root.join("123")).unwrap();
        assert_eq!(w.title, "Lluvia");
        assert_eq!(w.kind, "scene");
        assert!(w.preview.unwrap().ends_with("123/preview.gif"));
        assert!(w.id.ends_with("/123") && !w.id.contains('\\'));
    }

    #[test]
    fn sin_titulo_usa_la_carpeta_y_ignora_previews_peligrosas() {
        let root = temp("sin-titulo");
        project(&root, "456", "{\"preview\":\"..\\\\fuera.jpg\"}", None);
        let w = read_project(&root.join("456")).unwrap();
        assert_eq!(w.title, "456");
        assert_eq!(w.kind, "");
        assert_eq!(w.preview, None);
        project(&root, "789", "esto no es json", None);
        assert!(read_project(&root.join("789")).is_none());
        assert!(read_project(&root.join("no-existe")).is_none());
    }

    #[test]
    fn la_biblioteca_junta_las_carpetas_y_solo_se_aplica_lo_que_esta_en_ella() {
        let root = temp("biblioteca");
        let ws = root.join("workshop");
        project(&ws, "1", "{\"title\":\"Beta\"}", None);
        project(&ws, "2", "{\"title\":\"alfa\"}", None);
        std::fs::create_dir_all(root.join("sin-proyecto")).unwrap();
        let engine = WallpaperEngine { install: root.clone(), sources: vec![ws, root.join("no-hay")], cache: Mutex::new(None) };
        let titles: Vec<String> = engine.items().into_iter().map(|w| w.title).collect();
        assert_eq!(titles, ["alfa", "Beta"], "ordenados sin distinguir mayúsculas");
        let mut engine = engine;
        assert!(engine.apply(r"C:\Windows\System32", None).is_err());
        assert!(engine.apply("../../etc", Some(0)).is_err());
    }

    #[test]
    fn la_previsualizacion_actual_es_la_del_monitor_principal() {
        let item = |id: &str, p: Option<&str>| EngineWallpaper { id: id.into(), title: id.into(), kind: String::new(), preview: p.map(str::to_string) };
        let items = vec![item("a", Some("a.jpg")), item("b", Some("b.gif")), item("c", None)];
        let cur = |m, id: &str| EngineCurrent { monitor: m, id: id.into() };
        assert_eq!(preview_of_current(&[cur(1, "b"), cur(0, "a")], &items), Some("a.jpg".into()));
        // Si el del monitor 0 no tiene imagen se usa el siguiente que sí la tenga.
        assert_eq!(preview_of_current(&[cur(0, "c"), cur(1, "b")], &items), Some("b.gif".into()));
        assert_eq!(preview_of_current(&[cur(0, "zzz")], &items), None);
        assert_eq!(preview_of_current(&[], &items), None);
    }

    #[test]
    fn cada_monitor_usa_la_previsualizacion_de_su_propio_fondo() {
        let item = |id: &str, p: Option<&str>| EngineWallpaper { id: id.into(), title: id.into(), kind: String::new(), preview: p.map(str::to_string) };
        let items = vec![item("a", Some("a.jpg")), item("b", Some("b.gif")), item("c", None)];
        let cur = |m, id: &str| EngineCurrent { monitor: m, id: id.into() };
        let current = [cur(0, "a"), cur(1, "b"), cur(2, "c")];
        assert_eq!(preview_of_monitor(&current, &items, 0), Some("a.jpg".into()));
        assert_eq!(preview_of_monitor(&current, &items, 1), Some("b.gif".into()));
        // Sin imagen propia (o sin entrada) se cae a la del principal.
        assert_eq!(preview_of_monitor(&current, &items, 2), Some("a.jpg".into()));
        assert_eq!(preview_of_monitor(&current, &items, 7), Some("a.jpg".into()));
    }

    #[test]
    fn el_fondo_puesto_se_lee_de_la_configuracion_de_wallpaper_engine() {
        let config = serde_json::json!({
            "?installdirectory": "C:/x",
            "erick": { "general": { "wallpaperconfig": { "layout": 0, "selectedwallpapers": {
                "Monitor1": { "file": "C:/Steam/workshop/content/431960/222/scene.pkg" },
                "Monitor0": { "file": "C:\\Steam\\workshop\\content\\431960\\111\\scene.pkg" },
                "Otro": { "file": "x" }
            } } } }
        });
        assert_eq!(
            parse_current(&config),
            vec![
                EngineCurrent { monitor: 0, id: "C:/Steam/workshop/content/431960/111".into() },
                EngineCurrent { monitor: 1, id: "C:/Steam/workshop/content/431960/222".into() },
            ]
        );
        assert!(parse_current(&serde_json::json!({})).is_empty());
    }
}
