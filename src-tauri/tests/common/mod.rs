//! Utilidades para las pruebas reales: ventanas de prueba (WMTEST) y teclas inyectadas.
#![cfg(windows)]
#![allow(dead_code)]

pub mod e2e;

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use tesela_lib::domain::geometry::Rect;
use tesela_lib::infrastructure::windows::facts::{facts_of, visible_frame, window_rect};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
    VK_MENU,
};

pub struct TestWindows {
    child: Child,
    pub ids: Vec<isize>,
}

impl Drop for TestWindows {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Abre `count` ventanas WPF tituladas "<prefix> <i>" desde `tools/spawn-test-windows.ps1`,
/// ejecutado con una copia de powershell.exe llamada Lively.exe (ver DECISIONS.md D-15).
pub fn spawn_windows(count: usize, prefix: &str) -> TestWindows {
    spawn_windows_with(count, prefix, false)
}

/// Como `spawn_windows`, opcionalmente sin barra de título (para simular pantalla completa).
pub fn spawn_windows_with(count: usize, prefix: &str, borderless: bool) -> TestWindows {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../tools/spawn-test-windows.ps1");
    let dir = std::env::temp_dir().join("wm-test");
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("Lively.exe");
    if !exe.exists() {
        let src = std::path::Path::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        std::fs::copy(src, &exe).unwrap();
    }
    let mut args: Vec<String> = ["-NoProfile", "-ExecutionPolicy", "Bypass", "-STA", "-File", script, "-Count"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.extend([count.to_string(), "-Prefix".into(), prefix.to_string()]);
    if borderless {
        args.push("-Borderless".into());
    }
    let mut child = Command::new(exe).args(&args).stdout(Stdio::piped()).spawn().expect("powershell");
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

pub fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("tiempo agotado esperando: {what}");
}

/// Rectángulo visible de la ventana (sin sombra).
pub fn frame(id: isize) -> Rect {
    visible_frame(id).or_else(|| window_rect(id)).unwrap()
}

pub fn hidden(id: isize) -> bool {
    facts_of(id).map(|f| f.cloaked || !f.visible).unwrap_or(false)
}

/// Inyecta una combinación de teclas (todas abajo, luego todas arriba).
pub fn press_chord(keys: &[VIRTUAL_KEY]) {
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

/// Tecla de función F1..F24.
pub fn f(n: u16) -> VIRTUAL_KEY {
    VIRTUAL_KEY(0x6F + n)
}

/// Ctrl+Alt+F<n>.
pub fn ctrl_alt_f(n: u16) {
    press_chord(&[VK_CONTROL, VK_MENU, f(n)]);
}

/// Mueve una ventana (de otro proceso) a un rectángulo exacto, sin compensar sombras.
pub fn force_rect(id: isize, r: Rect) {
    use tesela_lib::infrastructure::windows::facts::hwnd_from;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};
    unsafe {
        let _ = SetWindowPos(hwnd_from(id), None, r.x, r.y, r.w, r.h, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}
