//! Fase 4: ventanas excluidas y pantalla completa con ventanas reales (WMTEST).
//! "Juego" = ventana sin barra de título agrandada hasta cubrir el monitor; "app excluida" = ventana normal
//! cuyo título contiene GAME y cuyo ejecutable (Lively) coincide con la regla de exclusión.
//! Se ejecuta a mano: `cargo test --test phase4_exclusions -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use window_manager_lib::domain::geometry::Rect;
use window_manager_lib::domain::manageable::Scope;
use window_manager_lib::domain::rules::ExclusionList;
use window_manager_lib::infrastructure::windows::facts::{list_monitors, primary_work_area, window_rect};
use window_manager_lib::infrastructure::windows::platform::Callbacks;
use window_manager_lib::infrastructure::windows::runtime::{Runtime, RuntimeOptions};

#[test]
#[ignore]
fn juego_y_app_excluida_conviven_con_el_gestor() {
    std::env::set_var("WM_STATE_DIR", std::env::temp_dir().join("wm-test-state"));
    let monitor = list_monitors().into_iter().find(|m| m.primary).expect("monitor principal");
    let area = primary_work_area();

    // Dos ventanas gestionadas, una "app excluida" y un "juego" (sin borde, todavía pequeño).
    let managed = spawn_windows(2, "WMTEST app");
    let excluded_app = spawn_windows(1, "WMTEST GAME app");
    let game = spawn_windows_with(1, "WMTEST GAME juego", true);
    let app_before = window_rect(excluded_app.ids[0]).unwrap();
    let game_before = window_rect(game.ids[0]).unwrap();

    let mut exclusions = ExclusionList::default();
    exclusions.add("lively.exe", Some("GAME"));
    let bar: Arc<Mutex<Vec<(u32, bool)>>> = Arc::default();
    let options = RuntimeOptions {
        exclusions,
        callbacks: Callbacks {
            bar: {
                let b = bar.clone();
                Arc::new(move |m, v| b.lock().unwrap().push((m, v)))
            },
            ..Callbacks::silent()
        },
        ..RuntimeOptions::basic(Scope::TitlePrefix("WMTEST".into()))
    };
    let rt = Runtime::start_with(options);

    // 1. Las gestionadas se reparten todo el ancho; las excluidas ni se enteran.
    wait_until("las 2 gestionadas ocupan la mitad cada una", || {
        managed.ids.iter().all(|&id| (frame(id).w - area.w / 2).abs() <= 3)
    });
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert_eq!(window_rect(excluded_app.ids[0]).unwrap(), app_before, "la app excluida no se mueve");
    assert_eq!(window_rect(game.ids[0]).unwrap(), game_before, "el juego no se mueve");
    assert!(bar.lock().unwrap().is_empty(), "la barra sigue visible: {:?}", bar.lock().unwrap());

    // 2. El juego pasa a pantalla completa: la barra se oculta en su monitor y nada se reacomoda.
    force_rect(game.ids[0], monitor.bounds);
    wait_until("la barra se oculta", || bar.lock().unwrap().contains(&(monitor.id, false)));
    assert!(managed.ids.iter().all(|&id| (frame(id).w - area.w / 2).abs() <= 3), "el reparto no cambia");

    // 3. Sale de pantalla completa: la barra reaparece y el juego queda donde el sistema lo dejó.
    force_rect(game.ids[0], Rect::new(300, 300, 500, 400));
    wait_until("la barra reaparece", || bar.lock().unwrap().last() == Some(&(monitor.id, true)));
    assert_eq!(window_rect(game.ids[0]).unwrap(), Rect::new(300, 300, 500, 400), "no lo recoloca");

    // 4. Una ventana gestionada en pantalla completa sale del reparto; al salir vuelve a su columna.
    let managed_game = spawn_windows_with(1, "WMTEST borderless", true);
    wait_until("entra al reparto", || (frame(managed_game.ids[0]).w - area.w / 3).abs() <= 3);
    force_rect(managed_game.ids[0], monitor.bounds);
    wait_until("la barra se oculta otra vez", || bar.lock().unwrap().last() == Some(&(monitor.id, false)));
    wait_until("las otras dos vuelven a repartirse el ancho", || {
        managed.ids.iter().all(|&id| (frame(id).w - area.w / 2).abs() <= 3)
    });
    force_rect(managed_game.ids[0], Rect::new(500, 200, 400, 300));
    wait_until("la barra reaparece de nuevo", || bar.lock().unwrap().last() == Some(&(monitor.id, true)));
    wait_until("vuelve a su columna", || {
        (frame(managed_game.ids[0]).w - area.w / 3).abs() <= 3 && managed.ids.iter().all(|&id| (frame(id).w - area.w / 3).abs() <= 3)
    });

    // 5. Marcar la ventana gestionada como excluida desde el "panel": se queda donde está.
    let mut list = ExclusionList::default();
    list.add("lively.exe", Some("GAME"));
    list.add("lively.exe", Some("borderless"));
    let pos_before = window_rect(managed_game.ids[0]).unwrap();
    rt.send(window_manager_lib::application::controller::Message::SetExclusions(list));
    wait_until("las otras dos ocupan la mitad", || managed.ids.iter().all(|&id| (frame(id).w - area.w / 2).abs() <= 3));
    assert_eq!(window_rect(managed_game.ids[0]).unwrap(), pos_before, "la recién excluida se queda donde está");

    rt.stop();
    assert!(bar.lock().unwrap().last().map(|b| b.1).unwrap_or(true), "al terminar la barra no queda oculta");
}
