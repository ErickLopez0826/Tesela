//! Aplicaciones en segundo plano: las que tienen icono en la bandeja de Windows y están en ejecución.
//!
//! Windows no ofrece una API pública para leer la bandeja, pero en Windows 11 guarda cada app que registró un icono en
//! `HKCU\Control Panel\NotifyIconSettings\*` (ruta del ejecutable y texto del icono). Eso se cruza con los procesos en
//! marcha para saber cuáles siguen vivas y cuánto consumen.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};
use windows::core::PWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ};

use crate::infrastructure::windows::autostart;

const SETTINGS: &str = r"Control Panel\NotifyIconSettings";

/// Un icono de bandeja guardado: nombre del ejecutable (minúsculas) y el texto que le puso la app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayEntry {
    pub exe: String,
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BackgroundApp {
    /// Nombre para mostrar.
    pub name: String,
    /// Nombre del ejecutable, en minúsculas.
    pub exe: String,
    /// Ruta real del ejecutable en marcha (sirve para el icono y para abrirla).
    pub path: String,
    pub cpu_percent: f32,
    pub ram_bytes: u64,
    pub processes: u32,
}

/// Un proceso en marcha, tal como lo necesita la unión.
#[derive(Debug, Clone)]
pub struct Running {
    pub exe: String,
    pub path: String,
    pub cpu_percent: f32,
    pub ram_bytes: u64,
}

/// Nombre del ejecutable (minúsculas) de una ruta guardada, que puede empezar por el GUID de una carpeta conocida.
pub fn exe_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).trim().to_lowercase()
}

/// Nombre para mostrar: la primera línea del texto del icono, sin el estado que muchas apps le añaden («Steam - …»);
/// si no hay, el nombre del ejecutable con la inicial en mayúscula.
pub fn display_name(tooltip: Option<&str>, exe: &str) -> String {
    let from_tip = tooltip
        .and_then(|t| t.lines().next())
        .map(|l| l.split(" - ").next().unwrap_or(l).trim())
        .filter(|l| !l.is_empty() && l.chars().count() <= 28);
    if let Some(t) = from_tip {
        return t.to_string();
    }
    let stem = exe.trim_end_matches(".exe");
    let mut chars = stem.chars();
    chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

/// Programas propios o del sistema que no interesan en esta lista.
fn ignored(entry_exe: &str, path: &str) -> bool {
    let p = path.to_lowercase();
    matches!(entry_exe, "explorer.exe" | "tesela.exe" | "window-manager.exe" | "securityhealthsystray.exe" | "ctfmon.exe")
        || entry_exe == "jucheck.exe"
        || (p.contains(r"\windows\") && !p.contains(r"\windowsapps\"))
}

/// Une los iconos de bandeja guardados con los procesos en ejecución: una entrada por ejecutable, con lo que suman sus
/// procesos. Ordenadas por nombre.
pub fn merge(entries: &[TrayEntry], running: &[Running]) -> Vec<BackgroundApp> {
    let mut by_exe: HashMap<String, BackgroundApp> = HashMap::new();
    for r in running {
        let Some(entry) = entries.iter().find(|e| e.exe == r.exe) else { continue };
        if ignored(&r.exe, &r.path) {
            continue;
        }
        let name = display_name(entry.tooltip.as_deref(), &r.exe);
        let app = by_exe.entry(r.exe.clone()).or_insert_with(|| BackgroundApp {
            name,
            exe: r.exe.clone(),
            path: r.path.clone(),
            cpu_percent: 0.0,
            ram_bytes: 0,
            processes: 0,
        });
        app.cpu_percent += r.cpu_percent;
        app.ram_bytes += r.ram_bytes;
        app.processes += 1;
    }
    let mut list: Vec<BackgroundApp> = by_exe.into_values().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

/// La aplicación de la lista con ese ejecutable (sin distinguir mayúsculas).
pub fn find_by_exe(list: &[BackgroundApp], exe: &str) -> Option<BackgroundApp> {
    list.iter().find(|a| a.exe.eq_ignore_ascii_case(exe.trim())).cloned()
}

/// La ruta de la lista que coincide exactamente con `path`; nunca devuelve una que no esté en la lista.
pub fn known_path(list: &[BackgroundApp], path: &str) -> Option<String> {
    list.iter().find(|a| a.path == path).map(|a| a.path.clone())
}

/// Subclaves de `NotifyIconSettings`.
fn subkeys() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let mut h = HKEY::default();
        let path: Vec<u16> = SETTINGS.encode_utf16().chain(std::iter::once(0)).collect();
        if RegOpenKeyExW(HKEY_CURRENT_USER, windows::core::PCWSTR(path.as_ptr()), None, KEY_READ, &mut h) != ERROR_SUCCESS {
            return out;
        }
        let mut index = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(h, index, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None) != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&name[..len as usize]));
            index += 1;
        }
        let _ = RegCloseKey(h);
    }
    out
}

/// Los iconos de bandeja que Windows recuerda.
pub fn tray_entries() -> Vec<TrayEntry> {
    subkeys()
        .into_iter()
        .filter_map(|k| {
            let sub = format!(r"{SETTINGS}\{k}");
            let path = autostart::status(&sub, "ExecutablePath")?;
            Some(TrayEntry { exe: exe_name(&path), tooltip: autostart::status(&sub, "InitialTooltip") })
        })
        .filter(|e| e.exe.ends_with(".exe"))
        .collect()
}

static SYS: Mutex<Option<System>> = Mutex::new(None);

fn running_processes() -> Vec<Running> {
    let mut guard = SYS.lock().unwrap_or_else(|e| e.into_inner());
    let sys = guard.get_or_insert_with(System::new);
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;
    sys.processes()
        .values()
        .filter_map(|p| {
            let path = p.exe()?.to_string_lossy().into_owned();
            Some(Running { exe: exe_name(&path), path, cpu_percent: p.cpu_usage() / cores, ram_bytes: p.memory() })
        })
        .collect()
}

/// Los procesos en marcha cuyo ejecutable es exactamente `path`.
pub fn pids_for(path: &str) -> Vec<u32> {
    let mut guard = SYS.lock().unwrap_or_else(|e| e.into_inner());
    let sys = guard.get_or_insert_with(System::new);
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes().values().filter(|p| p.exe().is_some_and(|e| e.to_string_lossy() == path)).map(|p| p.pid().as_u32()).collect()
}

/// Cierra todos los procesos de esa aplicación.
pub fn kill_all(path: &str) -> usize {
    let mut guard = SYS.lock().unwrap_or_else(|e| e.into_inner());
    let sys = guard.get_or_insert_with(System::new);
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes().values().filter(|p| p.exe().is_some_and(|e| e.to_string_lossy() == path)).filter(|p| p.kill()).count()
}

/// Las aplicaciones en segundo plano ahora mismo.
pub fn background_apps() -> Vec<BackgroundApp> {
    merge(&tray_entries(), &running_processes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(exe: &str, tip: Option<&str>) -> TrayEntry {
        TrayEntry { exe: exe.into(), tooltip: tip.map(String::from) }
    }
    fn run(exe: &str, path: &str, cpu: f32, ram: u64) -> Running {
        Running { exe: exe.into(), path: path.into(), cpu_percent: cpu, ram_bytes: ram }
    }

    #[test]
    fn el_nombre_sale_de_la_ruta_aunque_empiece_por_un_guid() {
        assert_eq!(exe_name(r"{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}\Battle.net\Battle.net.exe"), "battle.net.exe");
        assert_eq!(exe_name(r"C:\Users\x\AppData\Local\Discord\Discord.exe"), "discord.exe");
    }

    #[test]
    fn solo_salen_las_que_estan_en_marcha_y_se_suman_sus_procesos() {
        let entries = [entry("discord.exe", Some("Discord")), entry("spotify.exe", None), entry("viejo.exe", None)];
        let running = [
            run("discord.exe", r"C:\d\Discord.exe", 1.0, 100),
            run("discord.exe", r"C:\d\Discord.exe", 0.5, 50),
            run("spotify.exe", r"C:\s\Spotify.exe", 2.0, 300),
            run("otro.exe", r"C:\o\otro.exe", 9.0, 9),
        ];
        let list = merge(&entries, &running);
        assert_eq!(list.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Discord", "Spotify"]);
        assert_eq!((list[0].processes, list[0].ram_bytes), (2, 150));
        assert!((list[0].cpu_percent - 1.5).abs() < 1e-6);
    }

    #[test]
    fn solo_se_abre_o_se_da_icono_de_lo_que_esta_en_la_lista() {
        let list = merge(&[entry("discord.exe", None)], &[run("discord.exe", r"C:\d\Discord.exe", 0.0, 1)]);
        assert_eq!(find_by_exe(&list, "DISCORD.exe").map(|a| a.path), Some(r"C:\d\Discord.exe".to_string()));
        assert!(find_by_exe(&list, "cmd.exe").is_none());
        assert!(find_by_exe(&list, r"C:\Windows\System32\cmd.exe").is_none());
        assert_eq!(known_path(&list, r"C:\d\Discord.exe").as_deref(), Some(r"C:\d\Discord.exe"));
        assert!(known_path(&list, r"\\servidor\x.exe").is_none());
    }

    #[test]
    fn el_nombre_quita_el_estado_y_las_lineas_extra() {
        assert_eq!(display_name(Some("Steam - synchronizing"), "steam.exe"), "Steam");
        assert_eq!(display_name(Some("Descarga de Java
0% completa"), "x.exe"), "Descarga de Java");
        assert_eq!(display_name(None, "signalrgb.exe"), "Signalrgb");
        assert_eq!(display_name(Some("   "), "a.exe"), "A");
    }

    #[test]
    fn se_ignoran_el_explorador_tesela_y_lo_de_system32() {
        let entries = [entry("explorer.exe", None), entry("tesela.exe", None), entry("sec.exe", None)];
        let running = [
            run("explorer.exe", r"C:\Windows\explorer.exe", 0.0, 1),
            run("tesela.exe", r"C:\t\tesela.exe", 0.0, 1),
            run("sec.exe", r"C:\Windows\System32\sec.exe", 0.0, 1),
        ];
        assert!(merge(&entries, &running).is_empty());
    }
}
