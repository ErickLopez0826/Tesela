//! Aplicaciones que el usuario tiene: las instaladas (accesos directos del menú Inicio, de los que se lee el programa al
//! que apuntan) y las abiertas ahora. Sirven para elegir qué excluir y qué abrir al arrancar.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::core::{Interface, HSTRING};
use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use crate::application::system::AppsControl;
use crate::domain::startup::{exe_of_path, merge_suggestions, AppSuggestion};
use crate::infrastructure::windows::system::apps::Win32Apps;

/// El menú Inicio cambia poco: se reutiliza la lectura un rato.
const CACHE_FOR: Duration = Duration::from_secs(90);
static CACHE: Mutex<Option<(Instant, Vec<AppSuggestion>)>> = Mutex::new(None);

/// Accesos directos que no son programas que uno quiera abrir.
const NOISE: [&str; 10] = ["uninstall", "desinstal", "readme", "léeme", "leeme", "help", "ayuda", "website", "sitio web", "license"];

/// ¿Parece un acceso directo a un programa de verdad (y no a su desinstalador o su ayuda)?
pub fn is_program_name(stem: &str) -> bool {
    let lower = stem.to_lowercase();
    !NOISE.iter().any(|n| lower.contains(n))
}

fn start_menu_roots() -> Vec<PathBuf> {
    ["ProgramData", "APPDATA"]
        .iter()
        .filter_map(|var| std::env::var_os(var))
        .map(|base| PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"))
        .collect()
}

fn collect_shortcuts(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < 4 {
                collect_shortcuts(&path, depth + 1, out);
            }
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            out.push(path);
        }
    }
}

/// Programa al que apunta un acceso directo (`None` si no apunta a un `.exe`, como las apps de la Tienda).
fn shortcut_target(lnk: &Path) -> Option<String> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.cast::<IPersistFile>().ok()?.Load(&HSTRING::from(lnk.as_os_str()), STGM_READ).ok()?;
        let mut buf = [0u16; 520];
        link.GetPath(&mut buf, std::ptr::null_mut::<WIN32_FIND_DATAW>(), 0).ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let target = String::from_utf16_lossy(&buf[..len]);
        target.to_lowercase().ends_with(".exe").then_some(target)
    }
}

fn scan_installed() -> Vec<AppSuggestion> {
    let mut shortcuts = Vec::new();
    for root in start_menu_roots() {
        collect_shortcuts(&root, 0, &mut shortcuts);
    }
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let mut apps = Vec::new();
    for lnk in shortcuts {
        let stem = lnk.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if !is_program_name(&stem) {
            continue;
        }
        let Some(target) = shortcut_target(&lnk) else { continue };
        let exe = exe_of_path(&target);
        if exe.is_empty() {
            continue;
        }
        apps.push(AppSuggestion { name: stem, exe, path: lnk.to_string_lossy().to_string(), running: false });
    }
    if init.is_ok() {
        unsafe { CoUninitialize() };
    }
    apps
}

fn running_apps() -> Vec<AppSuggestion> {
    let mut seen = std::collections::HashSet::new();
    Win32Apps::new(Default::default())
        .open_apps()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| {
            let exe = exe_of_path(&a.exe);
            (!exe.is_empty() && !a.path.is_empty() && seen.insert(exe.clone())).then(|| AppSuggestion { name: exe.clone(), exe, path: a.path, running: true })
        })
        .collect()
}

/// Las aplicaciones que tiene el usuario, instaladas y abiertas, para elegir una.
pub fn suggestions() -> Vec<AppSuggestion> {
    let installed = {
        let mut cache = CACHE.lock().unwrap();
        match cache.as_ref() {
            Some((at, list)) if at.elapsed() < CACHE_FOR => list.clone(),
            _ => {
                let list = scan_installed();
                *cache = Some((Instant::now(), list.clone()));
                list
            }
        }
    };
    merge_suggestions(installed, running_apps())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_descartan_desinstaladores_y_ayudas() {
        assert!(is_program_name("Spotify"));
        assert!(is_program_name("Visual Studio Code"));
        assert!(!is_program_name("Uninstall Spotify"));
        assert!(!is_program_name("Desinstalar Foo"));
        assert!(!is_program_name("Léeme"));
        assert!(!is_program_name("Foo Help"));
    }

    /// Solo lectura: recorre el menú Inicio real y las ventanas abiertas.
    #[test]
    fn lee_las_apps_instaladas_y_abiertas_reales() {
        let apps = suggestions();
        println!("apps: {}", apps.len());
        for a in apps.iter().take(10) {
            println!("  {} | {} | {} | abierta={}", a.name, a.exe, a.path, a.running);
        }
        assert!(!apps.is_empty());
        assert!(apps.iter().all(|a| !a.name.is_empty() && !a.exe.is_empty() && !a.path.is_empty()));
        assert!(apps.windows(2).all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase()), "ordenadas por nombre");
    }
}
