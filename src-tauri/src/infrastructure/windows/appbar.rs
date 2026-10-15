//! AppBar de Windows (§9.1): la barra reserva su franja en el borde superior del monitor,
//! así el área útil de los demás ya no la incluye y las ventanas no se encima con ella.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, SetWindowPos, ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE, GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    WM_USER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::domain::geometry::Rect;
use crate::infrastructure::windows::facts::hwnd_from;

fn data(hwnd: HWND) -> APPBARDATA {
    APPBARDATA { cbSize: size_of::<APPBARDATA>() as u32, hWnd: hwnd, uCallbackMessage: WM_USER + 1, ..Default::default() }
}

/// Registra la ventana como AppBar en el borde superior de `monitor` con `height` píxeles físicos
/// y la coloca. Devuelve el rectángulo que quedó reservado (puede ir debajo de otra AppBar).
pub fn dock_top(window: isize, monitor: Rect, height: i32) -> Rect {
    let hwnd = hwnd_from(window);
    unsafe {
        // Sin foco y fuera del Alt+Tab: es una barra, no una ventana de trabajo.
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, (ex | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as i32);

        let mut abd = data(hwnd);
        SHAppBarMessage(ABM_NEW, &mut abd);
        abd.uEdge = ABE_TOP;
        abd.rc = RECT { left: monitor.x, top: monitor.y, right: monitor.right(), bottom: monitor.y + height };
        SHAppBarMessage(ABM_QUERYPOS, &mut abd);
        abd.rc.bottom = abd.rc.top + height;
        SHAppBarMessage(ABM_SETPOS, &mut abd);
        let rc = abd.rc;
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            rc.left,
            rc.top,
            rc.right - rc.left,
            rc.bottom - rc.top,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        Rect::new(rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top)
    }
}

/// Libera la franja reservada.
pub fn undock(window: isize) {
    unsafe {
        let mut abd = data(hwnd_from(window));
        SHAppBarMessage(ABM_REMOVE, &mut abd);
    }
}

/// Oculta o muestra la barra sin activarla; la franja reservada se mantiene.
pub fn set_visible(window: isize, visible: bool) {
    unsafe {
        let _ = ShowWindow(hwnd_from(window), if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
}
