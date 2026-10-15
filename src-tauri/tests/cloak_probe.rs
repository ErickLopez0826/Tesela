#![cfg(windows)]
//! Sonda: ¿DwmSetWindowAttribute(DWMWA_CLOAK) funciona sobre ventanas de otro proceso?
use windows::core::BOOL;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use window_manager_lib::infrastructure::windows::facts::{facts_of, hwnd_from};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

#[test]
#[ignore]
fn sonda_cloak() {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../tools/spawn-test-windows.ps1");
    let exe = std::env::temp_dir().join("wm-test").join("Lively.exe");
    let mut child = Command::new(exe)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-STA", "-File", script, "-Count", "1", "-Prefix", "WMTEST-probe"])
        .stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let id: isize = line.trim().strip_prefix("HANDLES ").unwrap().parse().unwrap();
    let v = BOOL(1);
    let r = unsafe { DwmSetWindowAttribute(hwnd_from(id), DWMWINDOWATTRIBUTE(13), &v as *const _ as *const _, 4) };
    println!("cloak resultado: {r:?}");
    println!("facts: {:?}", facts_of(id).map(|f| (f.cloaked, f.visible)));
    let v0 = BOOL(0);
    let _ = unsafe { DwmSetWindowAttribute(hwnd_from(id), DWMWINDOWATTRIBUTE(13), &v0 as *const _ as *const _, 4) };
    let _ = child.kill();
}
