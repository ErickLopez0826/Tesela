use std::path::Path;

use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

use crate::application::ports::WindowEnumerator;
use crate::domain::window::WindowCandidate;

pub struct Win32WindowEnumerator;

impl WindowEnumerator for Win32WindowEnumerator {
    fn enumerate(&self) -> Result<Vec<WindowCandidate>, String> {
        let mut found: Vec<WindowCandidate> = Vec::new();
        unsafe {
            EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize))
                .map_err(|e| e.to_string())?;
        }
        Ok(found)
    }
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let found = &mut *(lparam.0 as *mut Vec<WindowCandidate>);

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));

    let mut cloaked = 0u32;
    let _ = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut _ as *mut _,
        size_of::<u32>() as u32,
    );

    found.push(WindowCandidate {
        handle: hwnd.0 as isize,
        exe: exe_name(pid),
        title: window_title(hwnd),
        visible: IsWindowVisible(hwnd).as_bool(),
        cloaked: cloaked != 0,
        tool_window: (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0,
        own_process: pid == GetCurrentProcessId(),
    });
    BOOL(1)
}

unsafe fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// Nombre del ejecutable (p. ej. `code.exe`); vacío si el proceso no es accesible.
unsafe fn exe_name(pid: u32) -> String {
    let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
        return String::new();
    };
    let mut buf = [0u16; MAX_PATH as usize * 2];
    let mut len = buf.len() as u32;
    let result = QueryFullProcessImageNameW(
        process,
        PROCESS_NAME_FORMAT(0),
        PWSTR(buf.as_mut_ptr()),
        &mut len,
    );
    let _ = CloseHandle(process);
    if result.is_err() {
        return String::new();
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::list_windows::list_windows;

    /// Requiere una sesión de escritorio real: `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn enumera_ventanas_reales() {
        let windows = list_windows(&Win32WindowEnumerator).unwrap();
        for w in &windows {
            println!("{:<28} | {}", w.exe, w.title);
        }
        assert!(!windows.is_empty());
    }
}
