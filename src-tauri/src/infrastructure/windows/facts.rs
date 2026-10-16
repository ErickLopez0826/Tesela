//! Lectura de hechos de ventanas y monitores vía Win32.

use std::path::Path;

use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, MAX_PATH, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetParent, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, GWL_EXSTYLE,
    GWL_STYLE, GW_OWNER, MONITORINFOF_PRIMARY, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::domain::desktop::Monitor;
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
    monitor_info(id).map(|(_, bounds, work)| (bounds, work))
}

/// Monitor que contiene la ventana: (id, pantalla completa, área útil).
fn monitor_info(id: isize) -> Option<(u32, Rect, Rect)> {
    unsafe {
        let mon = MonitorFromWindow(hwnd_from(id), MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(mon, &mut info).ok().ok()?;
        Some((mon.0 as usize as u32, to_rect(info.rcMonitor), to_rect(info.rcWork)))
    }
}

/// ¿La ventana cubre todo su monitor sin barra de título (juego o F11)? Y en qué monitor está (§6.4).
pub fn fullscreen_state(id: isize) -> Option<(bool, Option<u32>)> {
    if !window_exists(id) {
        return None;
    }
    let hwnd = hwnd_from(id);
    let rect = window_rect(id)?;
    let (mon_id, bounds, _) = monitor_info(id)?;
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let has_caption = style & WS_CAPTION.0 == WS_CAPTION.0;
        let covers = rect.x <= bounds.x && rect.y <= bounds.y && rect.right() >= bounds.right() && rect.bottom() >= bounds.bottom();
        let on = IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool();
        Some((covers && !has_caption && on, Some(mon_id)))
    }
}

/// Área útil del monitor principal (escritorio como referencia).
pub fn primary_work_area() -> Rect {
    unsafe {
        let desktop = GetDesktopWindow();
        monitor_rects(id_of(desktop)).map(|(_, work)| work).unwrap_or(Rect::new(0, 0, 1920, 1040))
    }
}

fn dpi_scale(mon: HMONITOR) -> f64 {
    let (mut x, mut y) = (96u32, 96u32);
    unsafe {
        if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut x, &mut y).is_ok() && x > 0 {
            return x as f64 / 96.0;
        }
    }
    1.0
}

/// Monitores conectados, de izquierda a derecha y de arriba abajo.
pub fn list_monitors() -> Vec<Monitor> {
    unsafe extern "system" fn collect(mon: HMONITOR, _dc: HDC, _rc: *mut RECT, lparam: LPARAM) -> BOOL {
        let v = &mut *(lparam.0 as *mut Vec<Monitor>);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(mon, &mut info).as_bool() {
            v.push(Monitor {
                id: mon.0 as usize as u32,
                work_area: to_rect(info.rcWork),
                bounds: to_rect(info.rcMonitor),
                dpi_scale: dpi_scale(mon),
                primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        BOOL(1)
    }
    let mut v: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut v as *mut _ as isize));
    }
    v.sort_by_key(|m| (m.work_area.x, m.work_area.y));
    v
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
        let (fullscreen, monitor) = fullscreen_state(id).unwrap_or((false, None));

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
            monitor,
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

/// Ruta completa del ejecutable del proceso dueño de la ventana (para fijar y abrir apps).
pub fn exe_path_of(id: isize) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd_from(id), Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; MAX_PATH as usize * 2];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_FORMAT(0), PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Ventana en primer plano que merece un resaltado: visible, sin minimizar, sin ocultar por DWM, que no sea el
/// escritorio ni parte del shell de Windows, ni una ventana propia (salvo la principal) ni una a pantalla completa.
/// Devuelve su rectángulo visible (sin sombra), en píxeles físicos.
pub fn highlight_target() -> Option<Rect> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() || !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, size_of::<u32>() as u32);
        if cloaked != 0 {
            return None;
        }
        const SHELL: [&str; 9] = [
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "NotifyIconOverflowWindow",
            "TopLevelWindowForOverflowXamlIsland",
            "XamlExplorerHostIslandWindow",
            "Windows.UI.Core.CoreWindow",
            "ForegroundStaging",
        ];
        if SHELL.contains(&class_name(hwnd).as_str()) {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() && window_title(hwnd) != "Tesela" {
            return None;
        }
        let id = id_of(hwnd);
        if fullscreen_state(id).is_some_and(|(full, _)| full) {
            return None;
        }
        let r = visible_frame(id).or_else(|| window_rect(id))?;
        (r.w > 40 && r.h > 40).then_some(r)
    }
}
