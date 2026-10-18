//! Aplicaciones de arranque: programas que se abren junto con Tesela, cada uno con el workspace donde se queda.

use serde::{Deserialize, Serialize};

use crate::domain::rules::normalize_exe;

/// Cuántas aplicaciones de arranque se admiten.
pub const MAX_STARTUP_APPS: usize = 40;
/// Cuánto después de abrirla se sigue esperando la ventana de una aplicación para mandarla a su workspace.
pub const ASSIGN_WINDOW_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupApp {
    /// Nombre para mostrar.
    pub name: String,
    /// Qué se abre: la ruta del programa o de su acceso directo.
    pub path: String,
    /// Nombre del ejecutable de su ventana (sin `.exe`, en minúsculas): con él se reconoce la ventana para llevarla a su workspace.
    pub exe: String,
    /// Workspace 1 a 9.
    pub workspace: u8,
}

/// Una aplicación que se puede elegir (instalada o abierta ahora) para excluirla o abrirla al arrancar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSuggestion {
    pub name: String,
    /// Ejecutable de su ventana, normalizado (sin `.exe`, en minúsculas).
    pub exe: String,
    /// Qué se abre: el acceso directo del menú Inicio o la ruta del programa.
    pub path: String,
    /// ¿Está abierta ahora?
    pub running: bool,
}

/// Junta las apps instaladas con las abiertas: las instaladas que están abiertas se marcan, las abiertas que no están
/// instaladas (portátiles, de otras carpetas) se añaden. Sin repetir (mismo ejecutable y nombre) y por orden alfabético.
pub fn merge_suggestions(installed: Vec<AppSuggestion>, running: Vec<AppSuggestion>) -> Vec<AppSuggestion> {
    let running_exes: std::collections::HashSet<String> = running.iter().map(|a| a.exe.clone()).collect();
    let mut out: Vec<AppSuggestion> = Vec::new();
    for mut app in installed {
        app.running = running_exes.contains(&app.exe);
        if !out.iter().any(|o| o.exe == app.exe && o.name.eq_ignore_ascii_case(&app.name)) {
            out.push(app);
        }
    }
    for app in running {
        if !out.iter().any(|o| o.exe == app.exe) {
            out.push(AppSuggestion { running: true, ..app });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.exe.cmp(&b.exe)));
    out
}

/// Nombre normalizado del ejecutable de una ruta (`C:\Programas\Spotify.exe` → `spotify`).
pub fn exe_of_path(path: &str) -> String {
    let file = path.trim().trim_matches('"').rsplit(['\\', '/']).next().unwrap_or("");
    normalize_exe(file)
}

/// Ejecutable al que pertenece la ventana de una aplicación: los accesos directos de las «apps» de Chrome, Edge o Brave
/// apuntan a `chrome_proxy.exe`, pero su ventana es de `chrome.exe`.
pub fn window_exe(exe: &str) -> String {
    let exe = normalize_exe(exe);
    exe.strip_suffix("_proxy").map(str::to_string).filter(|e| !e.is_empty()).unwrap_or(exe)
}

/// Valida y limpia la lista que llega de la interfaz: sin vacíos ni repetidos (misma ruta), workspace entre 1 y 9.
pub fn validate(list: Vec<StartupApp>) -> Result<Vec<StartupApp>, String> {
    let mut out: Vec<StartupApp> = Vec::new();
    for mut app in list {
        app.path = app.path.trim().trim_matches('"').to_string();
        if app.path.is_empty() {
            return Err("falta la ruta de una aplicación".into());
        }
        if !(1..=9).contains(&app.workspace) {
            return Err(format!("el workspace de «{}» debe estar entre 1 y 9", app.path));
        }
        if out.iter().any(|a| a.path.eq_ignore_ascii_case(&app.path)) {
            continue;
        }
        let from_path = exe_of_path(&app.path);
        app.exe = if app.exe.trim().is_empty() || from_path.is_empty() && app.exe.trim().is_empty() { from_path } else { normalize_exe(&app.exe) };
        app.name = if app.name.trim().is_empty() { app.exe.clone() } else { app.name.trim().to_string() };
        out.push(app);
    }
    if out.len() > MAX_STARTUP_APPS {
        return Err(format!("como máximo {MAX_STARTUP_APPS} aplicaciones de arranque"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(path: &str, workspace: u8) -> StartupApp {
        StartupApp { name: String::new(), path: path.into(), exe: String::new(), workspace }
    }

    #[test]
    fn el_ejecutable_sale_de_la_ruta_sin_exe_ni_mayusculas() {
        assert_eq!(exe_of_path(r"C:\Program Files\Spotify\Spotify.exe"), "spotify");
        assert_eq!(exe_of_path("\"C:/x/Code.EXE\""), "code");
        assert_eq!(exe_of_path("notepad"), "notepad");
        assert_eq!(exe_of_path(""), "");
    }

    #[test]
    fn las_apps_de_chrome_y_edge_se_reconocen_por_el_navegador() {
        assert_eq!(window_exe("chrome_proxy"), "chrome");
        assert_eq!(window_exe("MSEdge_Proxy.exe"), "msedge");
        assert_eq!(window_exe("spotify"), "spotify");
        assert_eq!(window_exe("_proxy"), "_proxy");
        assert_eq!(window_exe(""), "");
    }

    #[test]
    fn juntar_sugerencias_marca_las_abiertas_anade_las_que_faltan_y_no_repite() {
        let app = |name: &str, exe: &str, running: bool| AppSuggestion { name: name.into(), exe: exe.into(), path: format!("C:/{exe}.exe"), running };
        let installed = vec![app("Spotify", "spotify", false), app("Discord", "update", false), app("Slack", "update", false), app("spotify", "spotify", false)];
        let running = vec![app("spotify", "spotify", true), app("portatil", "portatil", true)];
        let merged = merge_suggestions(installed, running);
        let names: Vec<_> = merged.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Discord", "portatil", "Slack", "Spotify"], "ordenadas sin distinguir mayúsculas y sin repetir");
        assert!(merged.iter().find(|a| a.name == "Spotify").unwrap().running);
        assert!(merged.iter().find(|a| a.name == "portatil").unwrap().running);
        assert!(!merged.iter().find(|a| a.name == "Discord").unwrap().running, "Discord y Slack comparten update.exe pero son distintas");
    }

    #[test]
    fn validar_completa_nombre_y_ejecutable_y_quita_repetidas() {
        let list = validate(vec![app(r"C:\a\Discord.exe", 2), app(r"c:\A\discord.EXE", 5), app(r"C:\b\Code.exe", 1)]).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].exe.as_str(), list[0].name.as_str(), list[0].workspace), ("discord", "discord", 2));
        assert_eq!(list[1].exe, "code");
    }

    #[test]
    fn validar_respeta_el_ejecutable_y_el_nombre_que_ya_traen() {
        let a = StartupApp { name: "Mi música".into(), path: r"C:\x\Lanzador.lnk".into(), exe: "Spotify.exe".into(), workspace: 3 };
        let list = validate(vec![a]).unwrap();
        assert_eq!((list[0].exe.as_str(), list[0].name.as_str()), ("spotify", "Mi música"));
    }

    #[test]
    fn validar_rechaza_rutas_vacias_workspaces_fuera_de_rango_y_demasiadas() {
        assert!(validate(vec![app("  ", 1)]).is_err());
        assert!(validate(vec![app(r"C:\a.exe", 0)]).is_err());
        assert!(validate(vec![app(r"C:\a.exe", 10)]).is_err());
        let many: Vec<_> = (0..=MAX_STARTUP_APPS).map(|i| app(&format!(r"C:\a{i}.exe"), 1)).collect();
        assert!(validate(many).is_err());
        assert_eq!(validate(vec![]).unwrap(), vec![]);
    }
}
