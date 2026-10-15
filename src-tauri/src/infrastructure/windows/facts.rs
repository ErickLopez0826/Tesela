//! Lectura de hechos de ventanas y monitores vía Win32.

use std::path::Path;

use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetDesktopWindow, GetParent, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, GWL_EXSTYLE,
    GW_OWNER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::domain::geometry::Rect;
use crate::domain::manageable::WindowFacts;

pub fn hwnd_from(id: isize) -> HWND {
    HWND(id as *mut std::ffi::c_void)
}

pub fn id_of(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

pub fn window_exists(id: isize) -> bool {
    unsafe { IsWindow(Some(hwnd_from(id))).as_bool() }
}

fn to_rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

/// Rectángulo del marco ventana (incluye el borde invisible de sombra).
pub fn window_rect(id: isize) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd_from(id), &mut r).ok()? };
    Some(to_rect(r))
}

/// Rectángulo visible (sin sombra); `None` si DWM no lo informa.
pub fn visible_frame(id: isize) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd_from(id),
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut _,
            size_of::<RECT>() as u32,
        )
        .ok()?;
    }
    Some(to_rect(r))
}

fn monitor_rects(id: isize) -> Option<(Rect, Rect)> {
    unsafe {
        let mon = MonitorFromWindow(hwnd_from(id), MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(mon, &mut info).ok().ok()?;
        Some((to_rect(info.rcMonitor), to_rect(info.rcWork)))
    }
}

/// Área útil del monitor principal (escritorio como referencia).
pub fn primary_work_area() -> Rect {
    unsafe {
        let desktop = GetDesktopWindow();
        monitor_rects(id_of(desktop)).map(|(_, work)| work).unwrap_or(Rect::new(0, 0, 1920, 1040))
    }
}

pub fn facts_of(id: isize) -> Option<WindowFacts> {
    if !window_exists(id) {
        return None;
    }
    let hwnd = hwnd_from(id);
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            size_of::<u32>() as u32,
        );

        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let rect = window_rect(id).unwrap_or_default();
        let fullscreen = monitor_rects(id)
            .map(|(mon, _)| rect.w >= mon.w && rect.h >= mon.h && rect.x <= mon.x && rect.y <= mon.y)
            .unwrap_or(false);

        Some(WindowFacts {
            handle: id,
            exe: exe_name(pid),
            title: window_title(hwnd),
            class: class_name(hwnd),
            visible: IsWindowVisible(hwnd).as_bool(),
            cloaked: cloaked != 0,
            tool_window: ex & WS_EX_TOOLWINDOW.0 != 0,
            no_activate: ex & WS_EX_NOACTIVATE.0 != 0,
            owned: GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.0.is_null())
                || GetParent(hwnd).is_ok_and(|p| !p.0.is_null()),
            own_process: pid == GetCurrentProcessId(),
            minimized: IsIconic(hwnd).as_bool(),
            fullscreen,
        })
    }
}

pub fn top_level_windows() -> Vec<isize> {
    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let v = &mut *(lparam.0 as *mut Vec<isize>);
        v.push(id_of(hwnd));
        BOOL(1)
    }
    let mut v: Vec<isize> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut v as *mut _ as isize));
    }
    v
}

pub unsafe fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

unsafe fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

/// Nombre del ejecutable (p. ej. `code.exe`); vacío si el proceso no es accesible.
pub unsafe fn exe_name(pid: u32) -> String {
    let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
        return String::new();
    };
    let mut buf = [0u16; MAX_PATH as usize * 2];
    let mut len = buf.len() as u32;
    let result =
        QueryFullProcessImageNameW(process, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut len);
    let _ = CloseHandle(process);
    if result.is_err() {
        return String::new();
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
