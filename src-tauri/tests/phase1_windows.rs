//! Fase 1: prueba real con ventanas creadas por `tools/spawn-test-windows.ps1`.
//! Solo se mueven ventanas con título "WMTEST ..." (alcance `prefix:WMTEST`).
//! Se ejecuta a mano: `cargo test --test phase1_windows -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use window_manager_lib::application::controller::Message;
use window_manager_lib::domain::geometry::Rect;
use window_manager_lib::domain::manageable::Scope;
use window_manager_lib::infrastructure::windows::facts::{
    facts_of, hwnd_from, primary_work_area, visible_frame, window_exists, window_rect,
};
use window_manager_lib::infrastructure::windows::platform::recover_cloaked_windows;
use window_manager_lib::infrastructure::windows::runtime::Runtime;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_F12, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

/// Inyecta una combinación de teclas (todas abajo, luego todas arriba).
fn press_chord(keys: &[VIRTUAL_KEY]) {
    let make = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, dwFlags: flags, ..Default::default() } },
    };
    let mut inputs: Vec<INPUT> = keys.iter().map(|&k| make(k, KEYBD_EVENT_FLAGS(0))).collect();
    inputs.extend(keys.iter().rev().map(|&k| make(k, KEYEVENTF_KEYUP)));
    unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

struct TestWindows {
    child: Child,
    ids: Vec<isize>,
}

impl Drop for TestWindows {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn spawn_windows(count: usize, prefix: &str) -> TestWindows {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../tools/spawn-test-windows.ps1");
    // powershell.exe copiado como Lively.exe: GlazeWM ignora sus ventanas WPF (ver el script).
    let dir = std::env::temp_dir().join("wm-test");
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("Lively.exe");
    if !exe.exists() {
        std::fs::copy(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe", &exe).unwrap();
    }
    let mut child = Command::new(exe)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-STA", "-File", script, "-Count", &count.to_string(), "-Prefix", prefix])
        .stdout(Stdio::piped())
        .spawn()
        .expect("powershell");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix("HANDLES ") {
                let ids: Vec<isize> = rest.split_whitespace().filter_map(|s| s.parse().ok()).collect();
                let _ = tx.send(ids);
                break;
            }
        }
    });
    let ids = rx.recv_timeout(Duration::from_secs(30)).expect("el script no devolvió handles");
    TestWindows { child, ids }
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("tiempo agotado esperando: {what}");
}

fn frame(id: isize) -> Rect {
    visible_frame(id).or_else(|| window_rect(id)).unwrap()
}

fn within(area: &Rect, r: &Rect) -> bool {
    // Tolerancia de 2 px por redondeos de DPI.
    r.x >= area.x - 2 && r.y >= area.y - 2 && r.right() <= area.right() + 2 && r.bottom() <= area.bottom() + 2
}

fn cloaked(id: isize) -> bool {
    facts_of(id).map(|f| f.cloaked || !f.visible).unwrap_or(false)
}

#[test]
#[ignore]
fn ciclo_completo_abrir_cerrar_cambiar_y_emergencia() {
    std::env::set_var("WM_STATE_DIR", std::env::temp_dir().join("wm-test-state"));
    let area = primary_work_area();
    let a = spawn_windows(3, "WMTEST");
    let originals: Vec<Rect> = a.ids.iter().map(|&id| window_rect(id).unwrap()).collect();

    let rt = Runtime::start(Scope::TitlePrefix("WMTEST".into()));

    // 1. Adopción inicial: tres columnas de ancho igual dentro del área útil.
    wait_until("las 3 ventanas están en columnas", || {
        a.ids.iter().all(|&id| within(&area, &frame(id)) && (frame(id).w - area.w / 3).abs() <= 3)
    });
    let widths: Vec<i32> = a.ids.iter().map(|&id| frame(id).w).collect();
    println!("anchos tras adopción: {widths:?} (área {area:?})");
    let max = *widths.iter().max().unwrap();
    let min = *widths.iter().min().unwrap();
    assert!(max - min <= 3, "las columnas no son iguales: {widths:?}");

    // 2. Cambiar de workspace oculta; volver muestra y en su sitio.
    rt.send(Message::SwitchWorkspace(1));
    wait_until("ventanas ocultas en otro workspace", || a.ids.iter().all(|&id| cloaked(id)));
    let f = facts_of(a.ids[0]).unwrap();
    println!("método de ocultación observado: cloaked={} visible={}", f.cloaked, f.visible);
    // Simula un cierre brusco: `--recover` debe volver a mostrar las ventanas ocultas.
    let recovered = recover_cloaked_windows();
    println!("recuperadas con --recover: {recovered}");
    assert_eq!(recovered, 3);
    wait_until("ventanas recuperadas visibles", || a.ids.iter().all(|&id| !cloaked(id)));
    rt.send(Message::SwitchWorkspace(0));
    wait_until("ventanas visibles de nuevo", || a.ids.iter().all(|&id| !cloaked(id) && within(&area, &frame(id))));

    // 3. Una ventana nueva (evento en vivo) se adopta; otra fuera de alcance no se toca.
    let b = spawn_windows(1, "WMTEST-extra");
    let outsider = spawn_windows(1, "AJENA");
    let outsider_before = window_rect(outsider.ids[0]).unwrap();
    wait_until("la ventana nueva entra en el layout", || {
        let f = frame(b.ids[0]);
        within(&area, &f) && f.w < area.w / 2
    });
    assert_eq!(window_rect(outsider.ids[0]).unwrap(), outsider_before, "no debe tocar ventanas fuera del alcance");

    // 4. Cerrar una ventana: las demás se reparten de nuevo y nada queda fuera de pantalla.
    unsafe {
        PostMessageW(Some(hwnd_from(a.ids[1])), WM_CLOSE, Default::default(), Default::default()).unwrap();
    }
    wait_until("la ventana cerrada desaparece", || !window_exists(a.ids[1]));
    wait_until("las 3 restantes se reparten el ancho", || {
        let ids = [a.ids[0], a.ids[2], b.ids[0]];
        println!("anchos tras cerrar: {:?}", ids.iter().map(|&i| frame(i)).collect::<Vec<_>>());
        ids.iter().all(|&id| within(&area, &frame(id)) && frame(id).w > area.w / 3 - 8)
    });

    // 5. Atajo de emergencia REAL (Ctrl+Alt+Shift+F12 inyectado): cada ventana vuelve a su sitio.
    press_chord(&[VK_CONTROL, VK_MENU, VK_SHIFT, VK_F12]);
    wait_until("ventanas restauradas", || {
        window_rect(a.ids[0]).unwrap() == originals[0]
            && window_rect(a.ids[2]).unwrap() == originals[2]
            && !cloaked(a.ids[0])
            && !cloaked(a.ids[2])
    });

    rt.stop();
    assert!(window_exists(a.ids[0]));
}
