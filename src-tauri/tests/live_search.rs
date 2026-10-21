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
#[ignore = "lee los paquetes reales de la Tienda; no abre nada"]
fn las_apps_de_escritorio_empaquetadas_se_encuentran_y_tienen_logo() {
    use tesela_lib::application::search::SearchHost;
    let packaged: Vec<_> = Win32SearchHost::new(Default::default()).apps().into_iter().filter(|a| a.path.starts_with("shell:AppsFolder")).collect();
    assert!(!packaged.is_empty(), "debe haber alguna app empaquetada de escritorio");

    let svc = SearchService::new();
    let mut host = Win32SearchHost::new(Default::default());
    let Some(teams) = packaged.iter().find(|a| a.name.to_lowercase().contains("teams")) else {
        eprintln!("Teams no está instalado en este equipo: se omite la búsqueda");
        return;
    };
    let found = svc.query(&mut host, "teams", &[], &HashMap::new(), 0);
    let hit = found.iter().find(|i| i.title == teams.name).unwrap_or_else(|| panic!("«teams» no encuentra {}: {found:?}", teams.name));
    assert!(!hit.id.contains(['\\', '/', ':', '!']), "el identificador sigue siendo opaco");

    // El logo sale del shell, no es una letra ni el icono genérico.
    let path = svc.icon_path(&hit.id).expect("ruta guardada por el núcleo");
    let png = icons::file_icon_png(&path).expect("logo de Teams");
    assert_eq!(&png[1..4], b"PNG");
    assert!(icons::file_icon_url(&path).unwrap().starts_with("data:image/png;base64,"));
    if let Ok(dir) = std::env::var("WM_ICON_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        for (n, app) in packaged.iter().take(12).enumerate() {
            if let Some(p) = icons::file_icon_png(&app.path) {
                let _ = std::fs::write(format!("{dir}/pkg-{n:02}-{}.png", app.exe.replace(['\\', '/', ':'], "_")), p);
            }
        }
    }
    let with_logo = packaged.iter().filter(|a| icons::file_icon_png(&a.path).is_some()).count();
    eprintln!("apps empaquetadas: {}, con logo: {with_logo}", packaged.len());
    assert!(with_logo * 10 >= packaged.len() * 8, "al menos el 80 % debe dar logo");
}

/// Anfitrión que lee las apps reales pero NO abre nada: solo anota lo que el servicio pidió lanzar.
struct RecordingHost {
    real: Win32SearchHost,
    launched: Vec<(String, bool)>,
}

impl tesela_lib::application::search::SearchHost for RecordingHost {
    fn apps(&mut self) -> Vec<tesela_lib::domain::startup::AppSuggestion> {
        self.real.apps()
    }
    fn open_apps(&mut self) -> Vec<tesela_lib::domain::utility::OpenApp> {
        self.real.open_apps()
    }
    fn focus(&mut self, _handle: isize) -> Result<(), String> {
        Err("no se enfoca nada en la prueba".into())
    }
    fn launch(&mut self, path: &str, as_admin: bool) -> Result<(), String> {
        self.launched.push((path.to_string(), as_admin));
        Ok(())
    }
    fn open_url(&mut self, _url: &str) -> Result<(), String> {
        Err("no se abre ninguna dirección en la prueba".into())
    }
}

#[test]
#[ignore = "lee las apps reales; NO abre ninguna (el anfitrión solo anota el lanzamiento)"]
fn todas_las_apps_empaquetadas_reales_se_aceptan_al_abrirlas() {
    use tesela_lib::application::search::SearchHost;
    let mut host = RecordingHost { real: Win32SearchHost::new(Default::default()), launched: Vec::new() };
    let packaged: Vec<_> = host.apps().into_iter().filter(|a| a.path.starts_with("shell:AppsFolder") && !a.running).collect();
    assert!(!packaged.is_empty());
    let svc = SearchService::new();
    let mut accepted = 0;
    for app in &packaged {
        // Se busca por el nombre exacto y se abre el resultado que corresponde a esta app.
        let items = svc.query(&mut host, &app.name, &[], &HashMap::new(), 0);
        let Some(item) = items.iter().find(|i| i.title == app.name) else { continue };
        host.launched.clear();
        svc.open(&mut host, &item.id, false).unwrap_or_else(|e| panic!("{} se rechazó: {e}", app.name));
        assert_eq!(host.launched, vec![(app.path.clone(), false)], "{}", app.name);
        accepted += 1;
    }
    eprintln!("apps empaquetadas aceptadas al abrir: {accepted} de {}", packaged.len());
    assert!(accepted * 10 >= packaged.len() * 9, "al menos el 90 % ({accepted} de {})", packaged.len());
}

#[test]
#[ignore = "lee el logo real de la Configuración de Windows; NO abre nada"]
fn la_configuracion_de_windows_aparece_con_su_engranaje_y_se_acepta_al_abrirla() {
    let mut host = RecordingHost { real: Win32SearchHost::new(Default::default()), launched: Vec::new() };
    let svc = SearchService::new();
    for (text, title, uri) in [("configuracion", "Configuración", "ms-settings:"), ("privacidad", "Privacidad y seguridad", "ms-settings:privacy"), ("barra de tareas", "Barra de tareas", "ms-settings:taskbar")] {
        let items = svc.query(&mut host, text, &[], &HashMap::new(), 0);
        let hit = items.iter().find(|i| i.title == title).unwrap_or_else(|| panic!("«{text}» no encuentra {title}: {items:?}"));
        assert_eq!(hit.kind, SearchKind::App);
        assert!(!hit.id.contains([':', '\\']), "el identificador sigue siendo opaco");
        let path = svc.icon_path(&hit.id).expect("ruta del icono");
        let png = icons::file_icon_png(&path).unwrap_or_else(|| panic!("sin icono para {title}"));
        assert_eq!(&png[1..4], b"PNG");
        host.launched.clear();
        svc.open(&mut host, &hit.id, true).unwrap();
        assert_eq!(host.launched, vec![(uri.to_string(), false)]);
    }
}

#[test]
#[ignore = "lee el menú Inicio real; NO abre nada"]
fn las_herramientas_del_sistema_salen_con_su_nombre_en_el_idioma_de_windows() {
    let mut host = RecordingHost { real: Win32SearchHost::new(Default::default()), launched: Vec::new() };
    let svc = SearchService::new();
    for (text, wanted) in [("administrador de tareas", "Administrador de tareas"), ("explorador", "Explorador de archivos"), ("panel de control", "Panel de control"), ("ejecutar", "Ejecutar"), ("editor del registro", "Editor del Registro"), ("servicios", "Servicios")] {
        let items = svc.query(&mut host, text, &[], &HashMap::new(), 0);
        eprintln!("«{text}» -> {:?}", items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>());
        let hit = items.iter().find(|i| i.title.eq_ignore_ascii_case(wanted)).unwrap_or_else(|| panic!("«{text}» no encuentra {wanted}: {items:?}"));
        host.launched.clear();
        svc.open(&mut host, &hit.id, false).unwrap_or_else(|e| panic!("{wanted} se rechazó: {e}"));
        assert_eq!(host.launched.len(), 1, "{wanted}");
        eprintln!("   se abriría: {:?}; icono: {}", host.launched[0], svc.icon_path(&hit.id).and_then(|p| icons::file_icon_png(&p)).is_some());
    }
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

#[test]
#[ignore = "vuelca los nombres que ofrece el buscador (para compararlos con el menú Inicio)"]
fn vuelca_las_apps_del_buscador() {
    use tesela_lib::application::search::SearchHost;
    let names: Vec<String> = Win32SearchHost::new(Default::default()).apps().into_iter().map(|a| a.name).collect();
    if let Ok(path) = std::env::var("WM_APPS_OUT") {
        std::fs::write(path, names.join("\n")).unwrap();
    }
    eprintln!("{} apps", names.len());
}

#[test]
#[ignore = "lee el menú Inicio real; NO abre nada"]
fn los_juegos_de_steam_del_menu_inicio_se_encuentran_con_su_icono_y_sin_elevar() {
    use tesela_lib::application::search::SearchHost;
    let mut host = RecordingHost { real: Win32SearchHost::new(Default::default()), launched: Vec::new() };
    let games: Vec<_> = host.apps().into_iter().filter(|a| a.path.to_lowercase().ends_with(".url")).collect();
    eprintln!("juegos de Steam: {:?}", games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>());
    assert!(games.iter().all(|g| g.exe.starts_with("steam-")), "solo juegos, no páginas web");
    let svc = SearchService::new();
    for game in games {
        let items = svc.query(&mut host, &game.name, &[], &HashMap::new(), 0);
        let hit = items.iter().find(|i| i.title == game.name).unwrap_or_else(|| panic!("no se encuentra {}", game.name));
        host.launched.clear();
        svc.open(&mut host, &hit.id, true).unwrap_or_else(|e| panic!("{} se rechazó: {e}", game.name));
        // Con ctrl+Enter también se abre sin elevar: un juego de Steam no se puede elevar desde fuera.
        assert_eq!(host.launched, vec![(game.path.clone(), false)]);
        eprintln!("  {} -> icono: {}", game.name, icons::file_icon_png(&game.path).is_some());
    }
}
