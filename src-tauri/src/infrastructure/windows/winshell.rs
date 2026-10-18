//! La barra de tareas de Windows: ocultarla mientras Tesela pone la suya y devolverla siempre al salir.
//!
//! Se oculta la ventana de la barra (`Shell_TrayWnd` y las de otros monitores) y se pone en «ocultar automáticamente»,
//! así Windows libera su franja y el área útil crece. Explorer a veces la vuelve a mostrar (al reiniciarse, al cambiar de
//! resolución, al acercar el ratón al borde): un hilo la vigila y la oculta de nuevo. El estado original se anota en un
//! archivo; si Tesela se cierra a la fuerza, el siguiente arranque (o `--recover`) lo restaura.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetClassNameW, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOWNA,
};

/// ¿Hay que mantener oculta la barra de Windows?
static HIDING: AtomicBool = AtomicBool::new(false);
/// Estado de la barra antes de tocarla (bits de `ABM_GETSTATE`).
static ORIGINAL: AtomicU32 = AtomicU32::new(0);
static GUARD_STARTED: AtomicBool = AtomicBool::new(false);
static MARKER: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

fn marker_path() -> std::path::PathBuf {
    MARKER.lock().unwrap().clone().unwrap_or_else(|| crate::infrastructure::windows::platform::state_dir().join("taskbar-hidden"))
}

/// Ventanas de la barra de tareas de Windows (la principal y las de los otros monitores).
fn tray_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<HWND>);
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf);
        if n > 0 && String::from_utf16_lossy(&buf[..n as usize]) == "Shell_SecondaryTrayWnd" {
            list.push(hwnd);
        }
        BOOL(1)
    }
    let mut list: Vec<HWND> = Vec::new();
    unsafe {
        if let Ok(main) = FindWindowW(windows::core::w!("Shell_TrayWnd"), PCWSTR::null()) {
            if !main.0.is_null() {
                list.push(main);
            }
        }
        let _ = EnumWindows(Some(collect), LPARAM(&mut list as *mut _ as isize));
    }
    list
}

fn appbar_state(set: Option<u32>) -> u32 {
    unsafe {
        let Some(main) = tray_windows().first().copied() else { return 0 };
        let mut abd = APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: main, ..Default::default() };
        match set {
            Some(state) => {
                abd.lParam = LPARAM(state as isize);
                SHAppBarMessage(ABM_SETSTATE, &mut abd);
                state
            }
            None => SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32,
        }
    }
}

fn show_all(visible: bool) {
    for hwnd in tray_windows() {
        unsafe {
            let _ = ShowWindow(hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }
}

/// Oculta la barra de tareas de Windows y la mantiene oculta, o la devuelve tal como estaba.
pub fn hide_taskbar(on: bool) {
    if on == HIDING.load(Ordering::SeqCst) {
        return;
    }
    if on {
        let original = appbar_state(None);
        ORIGINAL.store(original, Ordering::SeqCst);
        let path = marker_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, original.to_string());
        HIDING.store(true, Ordering::SeqCst);
        appbar_state(Some(original | ABS_AUTOHIDE));
        show_all(false);
        start_guard();
    } else {
        HIDING.store(false, Ordering::SeqCst);
        appbar_state(Some(ORIGINAL.load(Ordering::SeqCst)));
        show_all(true);
        let _ = std::fs::remove_file(marker_path());
    }
}

/// Hilo que vuelve a ocultar la barra de Windows si Explorer la muestra.
fn start_guard() {
    if GUARD_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new().name("wm-taskbar-guard".into()).spawn(|| loop {
        std::thread::sleep(Duration::from_millis(300));
        if !HIDING.load(Ordering::SeqCst) {
            continue;
        }
        for hwnd in tray_windows() {
            unsafe {
                if IsWindowVisible(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    });
}

/// Tras un cierre brusco: si quedó la marca de «barra oculta», devuelve la barra de Windows a como estaba. Devuelve si hizo algo.
pub fn recover_taskbar() -> bool {
    let path = marker_path();
    let Ok(text) = std::fs::read_to_string(&path) else { return false };
    let original = text.trim().parse::<u32>().unwrap_or(0);
    appbar_state(Some(original));
    show_all(true);
    let _ = std::fs::remove_file(path);
    true
}
