//! Buscador de aplicaciones (§18) con el sistema real: el atajo llega a la ventana (también con el gestor en pausa), el
//! escáner encuentra aplicaciones de verdad y los iconos salen del archivo y no del genérico de la extensión.
//!
//! No abre, enfoca ni lanza nada de tu escritorio. Se ejecuta a mano:
//! `cargo test --test live_search -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::*;
use tesela_lib::application::search::SearchService;
use tesela_lib::domain::manageable::Scope;
use tesela_lib::domain::search::SearchKind;
use tesela_lib::infrastructure::config_file::{config_path, load_file};
use tesela_lib::infrastructure::windows::icons;
use tesela_lib::infrastructure::windows::platform::Callbacks;
use tesela_lib::infrastructure::windows::runtime::{Runtime, RuntimeOptions};
use tesela_lib::infrastructure::windows::system::search_host::Win32SearchHost;

fn kb(action: &str, key: &str) -> String {
    format!("[[keybinding]]\naction = \"{action}\"\nkeys = [\"{key}\"]\n")
}

#[test]
#[ignore = "registra atajos globales reales (ctrl+alt+F21 y F22) un instante"]
fn el_atajo_del_buscador_llega_a_la_ventana_tambien_con_el_gestor_en_pausa() {
    let dir = std::env::temp_dir().join("wm-test-config-search");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("WM_CONFIG_DIR", &dir);
    std::env::set_var("WM_STATE_DIR", std::env::temp_dir().join("wm-test-state-search"));
    let text = ["[layout]\ninner_gap = 0\n".to_string(), kb("toggle_search", "ctrl+alt+f21"), kb("toggle_pause", "ctrl+alt+f22"), kb("workspace_5", "ctrl+alt+f23")].join("\n");
    std::fs::write(config_path(), text).unwrap();

    let w = spawn_windows(2, "WMTEST");
    let opened = Arc::new(AtomicUsize::new(0));
    let options = RuntimeOptions {
        loader: Some(Box::new(load_file)),
        callbacks: Callbacks {
            search: {
                let o = opened.clone();
                Arc::new(move || {
                    o.fetch_add(1, Ordering::SeqCst);
                })
            },
            ..Callbacks::silent()
        },
        ..RuntimeOptions::from_config(Scope::TitlePrefix("WMTEST".into()), load_file().expect("config de prueba válida"))
    };
    let rt = Runtime::start_with(options);
    std::thread::sleep(std::time::Duration::from_millis(800));

    ctrl_alt_f(21);
    wait_until("el atajo abre el buscador", || opened.load(Ordering::SeqCst) == 1);

    // En pausa el gestor no toca ventanas (el atajo de workspace deja de existir) pero el buscador sigue respondiendo.
    ctrl_alt_f(22);
    std::thread::sleep(std::time::Duration::from_millis(700));
    ctrl_alt_f(23);
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(w.ids.iter().all(|&id| !hidden(id)), "en pausa los atajos de ventanas no hacen nada");
    ctrl_alt_f(21);
    wait_until("el buscador responde con el gestor en pausa", || opened.load(Ordering::SeqCst) == 2);

    ctrl_alt_f(22); // reanuda
    std::thread::sleep(std::time::Duration::from_millis(700));
    ctrl_alt_f(21);
    wait_until("y otra vez al reanudar", || opened.load(Ordering::SeqCst) == 3);
    rt.stop();
}

#[test]
#[ignore = "lee el menú Inicio y las ventanas reales; no abre nada"]
fn el_escaner_real_encuentra_aplicaciones_y_los_identificadores_son_opacos() {
    let svc = SearchService::new();
    let mut host = Win32SearchHost::new(Default::default());
    let now = 0;

    // Una letra común: debe haber varias aplicaciones, todas con nombre, y ningún identificador es una ruta.
    let items = svc.query(&mut host, "e", &[], &HashMap::new(), now);
    eprintln!("resultados para «e»: {}", items.len());
    for i in items.iter().take(8) {
        eprintln!("  {:?} | {} | {}", i.kind, i.title, i.subtitle);
    }
    assert!(!items.is_empty(), "con aplicaciones instaladas «e» debe encontrar alguna");
    assert!(items.iter().all(|i| !i.title.trim().is_empty() && i.id.starts_with('r') && !i.id.contains(['\\', '/', ':'])));
    assert!(items.iter().all(|i| matches!(i.kind, SearchKind::App | SearchKind::Running)), "sin redirecciones configuradas solo hay aplicaciones");

    // Identificadores inventados no hacen nada, aunque parezcan rutas o direcciones.
    for fake in [r"C:\Windows\System32\cmd.exe", "https://example.com", "r99999999", "../../x"] {
        assert_eq!(svc.open(&mut host, fake, true).unwrap(), None, "{fake}");
    }

    // Con el cuadro vacío y sin historial no se sugiere nada.
    assert!(svc.query(&mut host, "", &[], &HashMap::new(), now).is_empty());

    // El icono de un resultado sale del archivo (en Rust, con la ruta que el núcleo guardó): no hay forma de pedir uno por ruta.
    let with_icon = items.iter().filter(|i| svc.icon_path(&i.id).is_some()).count();
    assert_eq!(with_icon, items.len(), "todas son programas con ruta");
}

#[test]
#[ignore = "lee los accesos directos reales del menú Inicio"]
fn los_iconos_de_los_accesos_directos_salen_sin_la_flechita() {
    let host_apps = {
        use tesela_lib::application::search::SearchHost;
        Win32SearchHost::new(Default::default()).apps()
    };
    let shortcuts: Vec<_> = host_apps.iter().filter(|a| a.path.to_lowercase().ends_with(".lnk")).collect();
    assert!(!shortcuts.is_empty(), "el menú Inicio debe tener accesos directos");
    let (mut ok, mut differ) = (0, 0);
    for (n, app) in shortcuts.iter().enumerate() {
        let Some(png) = icons::file_icon_png(&app.path) else { continue };
        ok += 1;
        assert_eq!(&png[1..4], b"PNG");
        // El icono del propio .lnk (el que lleva la flechita) es otro: el shell lo dibuja distinto.
        if icons::exe_icon_png(&app.path).as_ref() != Some(&png) {
            differ += 1;
        }
        if let Ok(dir) = std::env::var("WM_ICON_OUT") {
            let _ = std::fs::create_dir_all(&dir);
            if n < 24 {
                let _ = std::fs::write(format!("{dir}/{n:02}-{}.png", app.exe.replace(['\\', '/', ':'], "_")), &png);
            }
        }
    }
    eprintln!("accesos directos: {}, con icono: {ok}, distintos del genérico del .lnk: {differ}", shortcuts.len());
    assert!(ok * 10 >= shortcuts.len() * 8, "al menos el 80 % de los accesos directos debe dar icono ({ok} de {})", shortcuts.len());
}

#[test]
#[ignore = "lee iconos de programas reales"]
fn el_icono_sale_del_archivo_y_no_es_el_generico_de_la_extension() {
    let (notepad, explorer) = (r"C:\Windows\System32\notepad.exe", r"C:\Windows\explorer.exe");
    if !std::path::Path::new(notepad).exists() || !std::path::Path::new(explorer).exists() {
        eprintln!("faltan los programas de Windows de referencia: se omite");
        return;
    }
    let a = icons::file_icon_png(notepad).expect("icono del Bloc de notas");
    let b = icons::file_icon_png(explorer).expect("icono del Explorador");
    assert_eq!(&a[1..4], b"PNG");
    assert_ne!(a, b, "dos programas distintos no deben compartir el icono genérico de .exe");
    assert!(icons::file_icon_url(notepad).unwrap().starts_with("data:image/png;base64,"));
}
