//! Abre las aplicaciones de arranque junto con Tesela y avisa al gestor de a qué workspace va cada una.

use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::application::controller::{Message, StartupExpect};
use crate::application::system::AppsControl;
use crate::domain::rules::normalize_exe;
use crate::domain::startup::{window_exe, StartupApp, ASSIGN_WINDOW_SECS};
use crate::infrastructure::windows::facts::{facts_of, top_level_windows};
use crate::infrastructure::windows::system::apps::Win32Apps;

/// Pausa entre una aplicación y la siguiente: abrirlas todas de golpe satura el arranque.
const STAGGER: Duration = Duration::from_millis(500);

/// Ejecutables con ventana abierta ahora (para no abrir dos veces lo que ya está).
fn running_exes() -> HashSet<String> {
    top_level_windows()
        .into_iter()
        .filter_map(facts_of)
        .filter(|f| f.visible)
        .map(|f| normalize_exe(&f.exe))
        .collect()
}

/// Qué aplicaciones hay que abrir: las que no están ya abiertas (si no se sabe su ejecutable, siempre).
pub fn pending(apps: &[StartupApp], running: &HashSet<String>) -> Vec<StartupApp> {
    apps.iter().filter(|a| a.exe.is_empty() || !running.contains(&a.exe)).cloned().collect()
}

/// Lo que se le dice al gestor: la ventana nueva de cada aplicación abierta va a su workspace (1 a 9 → 0 a 8).
/// `move_existing`: además, las ventanas que esa aplicación ya tenía abiertas pasan a su workspace.
pub fn expectations(apps: &[StartupApp], move_existing: bool) -> Vec<StartupExpect> {
    apps.iter()
        .filter(|a| !a.exe.is_empty())
        .map(|a| StartupExpect {
            exe: window_exe(&a.exe),
            workspace: (a.workspace as usize).saturating_sub(1),
            ttl: Duration::from_secs(ASSIGN_WINDOW_SECS),
            move_existing,
        })
        .collect()
}

/// Abre las aplicaciones (en un hilo aparte) y, si hay gestor, le avisa antes de abrir cada ronda.
pub fn launch_all(apps: Vec<StartupApp>, manager: Option<Sender<Message>>) {
    if apps.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new().name("wm-startup-apps".into()).spawn(move || {
        let running = running_exes();
        let todo = pending(&apps, &running);
        if let Some(tx) = &manager {
            let _ = tx.send(Message::ExpectStartup(expectations(&todo, false)));
            // Las que ya estaban abiertas no se relanzan, pero sí van a su workspace.
            let open: Vec<StartupApp> = apps.iter().filter(|a| !a.exe.is_empty() && running.contains(&a.exe)).cloned().collect();
            if !open.is_empty() {
                let _ = tx.send(Message::ExpectStartup(expectations(&open, true)));
            }
        }
        let mut launcher = Win32Apps::new(Default::default());
        for app in todo {
            if let Err(e) = launcher.launch(&app.path) {
                eprintln!("[wm] aplicación de arranque «{}»: {e}", app.name);
            }
            std::thread::sleep(STAGGER);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(exe: &str, ws: u8) -> StartupApp {
        StartupApp { name: exe.into(), path: format!("C:/{exe}.exe"), exe: exe.into(), workspace: ws }
    }

    #[test]
    fn no_se_vuelve_a_abrir_lo_que_ya_esta_abierto() {
        let running: HashSet<String> = ["spotify".to_string()].into();
        let todo = pending(&[app("spotify", 2), app("code", 1), app("", 3)], &running);
        assert_eq!(todo.iter().map(|a| a.exe.as_str()).collect::<Vec<_>>(), ["code", ""]);
    }

    #[test]
    fn el_workspace_de_la_interfaz_pasa_a_indice_del_gestor() {
        let e = expectations(&[app("spotify", 1), app("code", 9), app("", 4), app("chrome_proxy", 6)], false);
        assert_eq!(e.iter().map(|x| (x.exe.as_str(), x.workspace)).collect::<Vec<_>>(), [("spotify", 0), ("code", 8), ("chrome", 5)]);
        assert!(expectations(&[app("code", 2)], true)[0].move_existing);
        assert!(e.iter().all(|x| x.ttl == Duration::from_secs(ASSIGN_WINDOW_SECS)));
    }
}
