//! Fase 3: atajos reales desde un archivo de configuración, modo redimensionar y recarga.
//! Usa combinaciones Ctrl+Alt+F<n> y F13/F14 (que no entran en conflicto con GlazeWM) sobre ventanas WMTEST.
//! Se ejecuta a mano: `cargo test --test phase3_hotkeys -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use common::*;
use window_manager_lib::domain::manageable::Scope;
use window_manager_lib::infrastructure::config_file::{config_path, load_file};
use window_manager_lib::infrastructure::windows::facts::primary_work_area;
use window_manager_lib::infrastructure::windows::runtime::{Runtime, RuntimeOptions};

fn config_text(extra: &str) -> String {
    format!(
        r#"
[layout]
min_column_width = 200
min_window_height = 120
inner_gap = 0
outer_top = 0
outer_right = 0
outer_bottom = 0
outer_left = 0

{extra}
"#
    )
}

fn kb(action: &str, key: &str, mode: Option<&str>) -> String {
    let mode = mode.map(|m| format!("mode = \"{m}\"\n")).unwrap_or_default();
    format!("[[keybinding]]\n{mode}action = \"{action}\"\nkeys = [\"{key}\"]\n")
}

#[test]
#[ignore]
fn atajos_modo_redimensionar_recarga_y_salida() {
    let dir = std::env::temp_dir().join("wm-test-config");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("WM_CONFIG_DIR", &dir);
    std::env::set_var("WM_STATE_DIR", std::env::temp_dir().join("wm-test-state"));

    let original = [
        kb("workspace_5", "ctrl+alt+f1", None),
        kb("workspace_1", "ctrl+alt+f2", None),
        kb("resize_mode", "ctrl+alt+f3", None),
        kb("reload_config", "ctrl+alt+f4", None),
        kb("exit", "ctrl+alt+f8", None),
        kb("resize_width_+2", "f13", Some("resize")),
        kb("exit_resize_mode", "f14", Some("resize")),
    ]
    .join("\n");
    std::fs::write(config_path(), config_text(&original)).unwrap();

    let area = primary_work_area();
    let w = spawn_windows(3, "WMTEST");
    let config = load_file().expect("config de prueba válida");
    let notices: Arc<Mutex<Vec<String>>> = Arc::default();
    let exited = Arc::new(AtomicBool::new(false));
    let options = RuntimeOptions {
        loader: Some(Box::new(load_file)),
        notice: {
            let n = notices.clone();
            Arc::new(move |t: &str| n.lock().unwrap().push(t.to_string()))
        },
        on_exit: {
            let e = exited.clone();
            Box::new(move || e.store(true, Ordering::SeqCst))
        },
        ..RuntimeOptions::from_config(Scope::TitlePrefix("WMTEST".into()), config)
    };
    let originals: Vec<_> = w.ids.iter().map(|&id| window_manager_lib::infrastructure::windows::facts::window_rect(id).unwrap()).collect();
    let rt = Runtime::start_with(options);
    let widths = || -> Vec<i32> { w.ids.iter().map(|&id| frame(id).w).collect() };

    wait_until("tres columnas iguales", || widths().iter().all(|x| (x - area.w / 3).abs() <= 3));
    assert!(notices.lock().unwrap().is_empty(), "sin avisos al arrancar: {:?}", notices.lock().unwrap());

    // 1. Atajo real -> cambia de workspace (oculta) y vuelve.
    ctrl_alt_f(1);
    wait_until("workspace 5 oculta las ventanas", || w.ids.iter().all(|&id| hidden(id)));
    ctrl_alt_f(2);
    wait_until("workspace 1 las muestra", || w.ids.iter().all(|&id| !hidden(id)));

    // 2. Modo redimensionar: F13 solo existe dentro del modo.
    let before = widths();
    ctrl_alt_f(3);
    std::thread::sleep(std::time::Duration::from_millis(500));
    press_chord(&[f(13)]);
    wait_until("el ancho de la ventana enfocada crece 2%", || {
        let now = widths();
        now.iter().zip(&before).any(|(n, b)| *n - *b >= (area.w as f64 * 0.02) as i32 - 4)
    });
    assert_eq!(widths().iter().sum::<i32>(), area.w, "la suma de anchos no cambia");
    press_chord(&[f(14)]); // salir del modo
    std::thread::sleep(std::time::Duration::from_millis(500));
    // Fuera del modo F13 ya no está registrada: no debe cambiar nada.
    let after_exit = widths();
    press_chord(&[f(13)]);
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert_eq!(widths(), after_exit);

    // 3. Recarga con configuración inválida (misma combinación en dos acciones): conserva los atajos y avisa.
    let broken = [original.clone(), kb("focus_left", "ctrl+alt+f9", None), kb("focus_right", "ctrl+alt+f9", None)].join("\n");
    std::fs::write(config_path(), config_text(&broken)).unwrap();
    ctrl_alt_f(4);
    wait_until("aviso de configuración inválida", || notices.lock().unwrap().iter().any(|n| n.contains("inválida") && n.contains("ctrl+alt+f9")));
    ctrl_alt_f(1);
    wait_until("los atajos anteriores siguen vivos", || w.ids.iter().all(|&id| hidden(id)));
    ctrl_alt_f(2);
    wait_until("vuelven", || w.ids.iter().all(|&id| !hidden(id)));

    // 4. Recarga válida con un atajo distinto: ctrl+alt+f1 deja de existir, ctrl+alt+f7 lo reemplaza.
    let remapped = original.replace("ctrl+alt+f1\"", "ctrl+alt+f7\"");
    std::fs::write(config_path(), config_text(&remapped)).unwrap();
    ctrl_alt_f(4);
    wait_until("aviso de recarga", || notices.lock().unwrap().iter().any(|n| n.contains("recargada")));
    ctrl_alt_f(1);
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert!(w.ids.iter().all(|&id| !hidden(id)), "el atajo viejo ya no hace nada");
    ctrl_alt_f(7);
    wait_until("el atajo nuevo funciona", || w.ids.iter().all(|&id| hidden(id)));
    ctrl_alt_f(2);
    wait_until("vuelven otra vez", || w.ids.iter().all(|&id| !hidden(id)));

    // 5. Salir de la app: restaura ventanas y avisa al anfitrión.
    ctrl_alt_f(8);
    wait_until("la app pide salir", || exited.load(Ordering::SeqCst));
    wait_until("ventanas restauradas al salir", || {
        w.ids.iter().zip(&originals).all(|(&id, o)| window_manager_lib::infrastructure::windows::facts::window_rect(id).unwrap() == *o)
    });
    rt.stop();
}
