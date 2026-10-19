//! Pruebas en vivo de la interfaz de Windows que Tesela reemplaza. Tocan el escritorio real (ocultan la barra de tareas
//! un instante y la devuelven), por eso van marcadas con `#[ignore]`: `cargo test --test live_shell -- --ignored --nocapture`.

#![cfg(windows)]

use std::time::Duration;

use tesela_lib::infrastructure::windows::{facts, icons, winshell};

fn tray_visible() -> bool {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible};
    unsafe {
        FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).map(|h| IsWindowVisible(h).as_bool()).unwrap_or(false)
    }
}

#[test]
#[ignore = "oculta la barra de tareas real un instante"]
fn la_barra_de_windows_se_oculta_y_vuelve() {
    let _ = winshell::recover_taskbar();
    assert!(tray_visible(), "la prueba parte con la barra de Windows visible");
    winshell::hide_taskbar(true);
    std::thread::sleep(Duration::from_millis(900));
    let hidden = !tray_visible();
    winshell::hide_taskbar(false);
    std::thread::sleep(Duration::from_millis(400));
    assert!(hidden, "la barra de Windows debía quedar oculta");
    assert!(tray_visible(), "la barra de Windows debía volver");
    assert!(!winshell::recover_taskbar(), "al volver no debe quedar la marca de «oculta»");
}

#[test]
#[ignore = "lee iconos de ventanas reales"]
fn los_iconos_de_las_ventanas_abiertas_salen_como_png() {
    let mut found = 0;
    for id in facts::top_level_windows() {
        let Some(f) = facts::facts_of(id) else { continue };
        if !f.visible || f.title.trim().is_empty() {
            continue;
        }
        if let Some(png) = icons::app_icon_png(id) {
            assert_eq!(&png[1..4], b"PNG");
            if let Ok(dir) = std::env::var("WM_ICON_OUT") {
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::fs::write(format!("{dir}/{}-{}.png", found, f.exe.replace(['\\', '/', ':'], "_")), &png);
            }
            found += 1;
        }
        if found >= 6 {
            break;
        }
    }
    eprintln!("iconos leídos: {found}");
    assert!(found > 0, "con ventanas abiertas debería salir al menos un icono");
}

#[test]
#[ignore = "pulsa la tecla Windows de verdad (inyectada) y mira si se abre Inicio"]
fn tocar_windows_dos_veces_nunca_abre_inicio() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tesela_lib::infrastructure::windows::winkey;
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_LWIN};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let taps = Arc::new(AtomicUsize::new(0));
    let t = taps.clone();
    winkey::set_enabled(true, Some(Arc::new(move || { t.fetch_add(1, Ordering::SeqCst); })));
    std::thread::sleep(Duration::from_millis(500));
    let key = |flags| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(VK_LWIN.0), dwFlags: flags, ..Default::default() } } };
    let start_open = || unsafe {
        let hwnd = GetForegroundWindow();
        facts::facts_of(facts::id_of(hwnd)).map(|f| f.exe.to_lowercase()).unwrap_or_default()
    };
    for n in 1..=4 {
        unsafe {
            SendInput(&[key(Default::default())], size_of::<INPUT>() as i32);
            std::thread::sleep(Duration::from_millis(60));
            SendInput(&[key(KEYEVENTF_KEYUP)], size_of::<INPUT>() as i32);
        }
        std::thread::sleep(Duration::from_millis(900));
        let fg = start_open();
        println!("toque {n}: toques vistos = {}, ventana al frente = {fg}", taps.load(Ordering::SeqCst));
        assert!(!fg.starts_with("startmenuexperiencehost") && !fg.starts_with("searchhost"), "se abrió Inicio en el toque {n}");
    }
    winkey::set_enabled(false, None);
    assert_eq!(taps.load(Ordering::SeqCst), 4);
}

#[test]
#[ignore = "lee la bandeja y los procesos reales"]
fn las_apps_en_segundo_plano_salen_de_la_bandeja_y_estan_en_marcha() {
    use tesela_lib::infrastructure::windows::system::background::background_apps;
    let _ = background_apps(); // primera muestra de CPU
    std::thread::sleep(Duration::from_millis(1200));
    let apps = background_apps();
    for a in &apps {
        println!("{:<28} {:<22} {:>5.1}% {:>7} MB  x{}  {}", a.name, a.exe, a.cpu_percent, a.ram_bytes / 1_000_000, a.processes, a.path);
    }
    assert!(!apps.is_empty(), "en este equipo hay apps con icono de bandeja en marcha");
}
