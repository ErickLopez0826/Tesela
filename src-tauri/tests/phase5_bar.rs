//! Fase 5: la barra se acopla como AppBar, reserva su espacio y las ventanas no se le encima.
//! Arranca la app real con `--manage prefix:WMTEST --bar` y ventanas de prueba WMTEST.
//! Reserva brevemente una franja en el borde superior de tus monitores (se libera al cerrar la app).
//! Se ejecuta a mano: `cargo test --test phase5_bar -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::process::{Child, Command};
use std::time::Duration;

use common::*;
use tesela_lib::domain::desktop::Monitor;
use tesela_lib::domain::geometry::Rect;
use tesela_lib::infrastructure::windows::facts::{facts_of, list_monitors, top_level_windows};

struct App(Child);

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn bars() -> Vec<(isize, Rect)> {
    top_level_windows()
        .into_iter()
        .filter_map(|id| {
            let f = facts_of(id)?;
            (f.title == "Tesela Bar" && f.exe.eq_ignore_ascii_case("tesela.exe"))
                .then(|| (id, tesela_lib::infrastructure::windows::facts::window_rect(id).unwrap()))
        })
        .collect()
}

fn monitor_of_bar(monitors: &[Monitor], bar: &Rect) -> Monitor {
    monitors.iter().find(|m| m.bounds.contains_rect(&Rect::new(bar.x, bar.y, bar.w, 1))).cloned().expect("la barra está dentro de un monitor")
}

#[test]
#[ignore]
fn la_barra_reserva_su_espacio_y_las_ventanas_no_se_le_encimas() {
    let state = std::env::temp_dir().join("wm-test-state");
    let config = std::env::temp_dir().join("wm-test-config-bar");
    let _ = std::fs::remove_dir_all(&config);
    let baseline = list_monitors();
    assert!(!baseline.is_empty());
    let primary = baseline.iter().find(|m| m.primary).unwrap().clone();

    let windows = spawn_windows(3, "WMTEST app");
    wait_until("las ventanas ya se reparten el área útil original", || {
        windows.ids.iter().all(|&id| (frame(id).w - (primary.work_area.w - 80) / 3).abs() < 40 || frame(id).w > 100)
    });

    let _app = App(
        Command::new(env!("CARGO_BIN_EXE_tesela"))
            .args(["--manage", "prefix:WMTEST", "--bar"])
            .env("WM_CONFIG_DIR", &config)
            .env("WM_STATE_DIR", &state)
            .spawn()
            .expect("arrancar la app"),
    );

    // 1. Una barra por monitor.
    wait_until("hay una barra por monitor", || bars().len() == baseline.len());
    let monitors_now = || list_monitors();
    std::thread::sleep(Duration::from_millis(1500));

    for (id, rect) in bars() {
        let mon = monitor_of_bar(&baseline, &rect);
        let expected_h = (40.0 * mon.dpi_scale).round() as i32;
        println!("barra {id}: {rect:?} en monitor {:?}", mon.bounds);
        assert_eq!(rect.w, mon.bounds.w, "ocupa todo el ancho del monitor");
        assert_eq!(rect.h, expected_h, "alto esperado");
        assert!(rect.y >= mon.bounds.y && rect.y < mon.bounds.y + 200, "pegada al borde superior");

        // 2. El área útil del monitor ya no incluye la barra.
        let now = monitors_now().into_iter().find(|m| m.id == mon.id).unwrap();
        assert!(now.work_area.y >= rect.bottom(), "área útil {:?} debajo de la barra {rect:?}", now.work_area);
    }

    // 3. Las ventanas gestionadas se reacomodan debajo de la barra del monitor principal.
    let primary_bar = bars().into_iter().map(|b| b.1).find(|r| primary.bounds.contains_rect(&Rect::new(r.x, r.y, r.w, 1))).unwrap();
    wait_until("las ventanas quedan debajo de la barra", || windows.ids.iter().all(|&id| frame(id).y >= primary_bar.bottom()));
    for &id in &windows.ids {
        let f = frame(id);
        println!("ventana {id}: {f:?}");
        assert!(f.y >= primary_bar.bottom(), "la ventana {f:?} se encima con la barra {primary_bar:?}");
        let work = monitors_now().into_iter().find(|m| m.id == primary.id).unwrap().work_area;
        assert!(work.contains_rect(&f) || (f.x >= work.x - 2 && f.right() <= work.right() + 2 && f.bottom() <= work.bottom() + 2));
    }

    // 4. Al cerrar la app se libera la franja reservada.
    drop(_app);
    wait_until("el área útil vuelve a la original", || {
        let now = list_monitors();
        baseline.iter().all(|b| now.iter().any(|n| n.id == b.id && n.work_area == b.work_area))
    });
}

/// Ventanas visibles de un proceso.
fn visible_windows_of(pid: u32) -> Vec<(String, String)> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible};
    struct Ctx {
        pid: u32,
        found: Vec<isize>,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        if owner == ctx.pid && IsWindowVisible(hwnd).as_bool() {
            ctx.found.push(hwnd.0 as isize);
        }
        true.into()
    }
    let mut ctx = Ctx { pid, found: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.found
        .into_iter()
        .filter_map(|h| facts_of(h))
        .map(|f| (f.title, f.class))
        .collect()
}

#[test]
#[ignore = "arranca la app real unos segundos, sin barra y sin gestionar ninguna ventana tuya"]
fn al_abrir_la_app_no_aparece_ninguna_ventana_principal() {
    let config = std::env::temp_dir().join("wm-test-config-nowindow");
    let state = std::env::temp_dir().join("wm-test-state-nowindow");
    let _ = std::fs::remove_dir_all(&config);
    let exe = std::env::var_os("WM_TEST_EXE").unwrap_or_else(|| env!("CARGO_BIN_EXE_tesela").into());
    let mut app = Command::new(exe)
        .args(["--manage", "prefix:WMNADA"])
        .env("WM_CONFIG_DIR", &config)
        .env("WM_STATE_DIR", &state)
        .spawn()
        .expect("arrancar la app");
    std::thread::sleep(Duration::from_secs(5));
    let alive = app.try_wait().expect("estado del proceso").is_none();
    // Internas y sin imagen: el borde animado (transparente, sin ventana a la que rodear) y el receptor de eventos del sistema.
    let visible: Vec<_> = visible_windows_of(app.id())
        .into_iter()
        .filter(|(title, class)| title != "Tesela Border" && class != "Tao Thread Event Target")
        .collect();
    let _ = app.kill();
    assert!(alive, "la app no debe terminar sola por no tener ventanas");
    assert!(visible.is_empty(), "no debe haber ninguna ventana visible, pero hay: {visible:?}");
}
