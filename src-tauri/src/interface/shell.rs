//! Lo que Tesela reemplaza de la interfaz de Windows: su barra de tareas y el toque de la tecla Windows. Se guarda en
//! `shell.json` y se aplica al arrancar y al cambiarlo desde Ajustes.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tauri::AppHandle;

use crate::domain::shell::ShellSettings;
use crate::infrastructure::shell_store;
use crate::infrastructure::windows::{facts::list_monitors, winkey, winshell};

static CURRENT: Mutex<Option<ShellSettings>> = Mutex::new(None);
static HOVER: std::sync::Once = std::sync::Once::new();
/// La ventana que tenía el foco justo antes de abrir el panel: al cerrarlo se le devuelve, para que Windows no tenga que
/// elegir una por su cuenta (con su barra oculta, esa elección acababa en el menú Inicio).
static PREVIOUS: Mutex<isize> = Mutex::new(0);
static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();
static REFRESH_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static GUARD: std::sync::Once = std::sync::Once::new();

pub fn get() -> ShellSettings {
    CURRENT.lock().unwrap().unwrap_or_default()
}

/// Monitor donde está el cursor (o el principal).
pub fn cursor_monitor() -> Option<u32> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let monitors = list_monitors();
    let mut p = POINT::default();
    let under = unsafe { GetCursorPos(&mut p).ok() }
        .and_then(|_| monitors.iter().find(|m| p.x >= m.bounds.x && p.x < m.bounds.right() && p.y >= m.bounds.y && p.y < m.bounds.bottom()));
    under.or_else(|| monitors.iter().find(|m| m.primary)).or(monitors.first()).map(|m| m.id)
}

/// ¿Es `hwnd` el menú Inicio o la búsqueda de Windows?
fn is_start_host(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use crate::infrastructure::windows::facts::{facts_of, id_of};
    if hwnd.0.is_null() {
        return false;
    }
    facts_of(id_of(hwnd)).is_some_and(|f| {
        let exe = f.exe.to_lowercase();
        ["startmenuexperiencehost", "searchhost", "searchapp"].iter().any(|n| exe.starts_with(n))
    })
}

/// Cierra Inicio o la búsqueda: la oculta al instante (para que no llegue a verse) y le manda Escape para que su estado
/// interno se cierre también.
fn dismiss_start(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_ESCAPE};
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let key = |flags| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_ESCAPE, dwFlags: flags, ..Default::default() } } };
        SendInput(&[key(Default::default()), key(KEYEVENTF_KEYUP)], size_of::<INPUT>() as i32);
    }
}

/// Si el menú Inicio (o la búsqueda) de Windows está al frente, lo cierra.
fn close_windows_start() {
    let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    if is_start_host(hwnd) {
        dismiss_start(hwnd);
    }
}

unsafe extern "system" fn foreground_changed(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    hwnd: windows::Win32::Foundation::HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    use crate::infrastructure::windows::facts::{facts_of, id_of};
    let what = facts_of(id_of(hwnd)).map(|f| format!("{} «{}»", f.exe, f.title)).unwrap_or_else(|| "(ninguna)".into());
    winkey::log(format!("[{}] foco -> {what}", winkey::now_ms()));
    if winkey::is_enabled() && is_start_host(hwnd) {
        dismiss_start(hwnd);
    }
    if what.starts_with("tesela.exe") {
        keep_hook_fresh();
    }
}

/// Con una ventana de Tesela (un WebView) en uso, el gancho de teclado ya puesto deja de recibir teclas: se vuelve a poner
/// cada poco mientras el panel esté a la vista (el nuevo se instala antes de retirar el viejo, sin huecos). También se
/// llama al hacer clic dentro del panel, que es cuando el WebView toma el foco de teclado.
pub fn keep_hook_fresh() {
    if REFRESH_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new().name("wm-winkey-refresh".into()).spawn(|| {
        let started = std::time::Instant::now();
        // Dura mientras el panel esté a la vista (con un tope por si algo falla).
        while started.elapsed() < std::time::Duration::from_secs(600) {
            std::thread::sleep(std::time::Duration::from_millis(400));
            winkey::reinstall();
            let open = APP.get().is_some_and(crate::interface::bar::panel_open);
            if !open && started.elapsed() > std::time::Duration::from_millis(1500) {
                break;
            }
        }
        REFRESH_RUNNING.store(false, Ordering::SeqCst);
    });
}

/// Red de seguridad: avisa en cuanto cambia la ventana al frente (sin sondear) y, si es Inicio o la búsqueda de Windows
/// mientras la tecla Windows es de Tesela, la cierra al momento.
fn start_guard() {
    use windows::Win32::UI::Accessibility::SetWinEventHook;
    use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT};
    let _ = std::thread::Builder::new().name("wm-start-guard".into()).spawn(|| unsafe {
        let hook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(foreground_changed), 0, 0, WINEVENT_OUTOFCONTEXT);
        if hook.0.is_null() {
            eprintln!("[wm] no se pudo vigilar el foco (Inicio podría verse un instante)");
            return;
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

/// Apunta la ventana con el foco (si no es de Tesela) para devolvérselo al cerrar el panel.
fn remember_foreground() {
    use crate::infrastructure::windows::facts::{facts_of, id_of};
    let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    let mine = facts_of(id_of(hwnd)).is_none_or(|f| f.exe.to_lowercase().starts_with("tesela"));
    *PREVIOUS.lock().unwrap() = if mine { 0 } else { id_of(hwnd) };
}

/// Devuelve el foco a la ventana que lo tenía antes del panel, si sigue existiendo.
fn restore_previous_focus() {

    use crate::infrastructure::windows::facts::window_exists;
    let id = std::mem::take(&mut *PREVIOUS.lock().unwrap());
    if id != 0 && window_exists(id) {
        crate::infrastructure::windows::platform::focus_window(id);
    }
}

fn apply(app: &AppHandle, s: ShellSettings) {
    let handle = app.clone();
    let on_tap: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        close_windows_start();
        let Some(id) = cursor_monitor() else { return };
        // Un toque despliega el panel y la barra de tareas; si alguno está a la vista, los cierra.
        let (panel, bar) = (crate::interface::bar::panel_open(&handle), crate::interface::taskbar::is_shown(id));
        winkey::log(format!("[{}]   acción: panel abierto={panel}, barra a la vista={bar}, monitor={id}", winkey::now_ms()));
        if panel || bar {
            if panel {
                restore_previous_focus();
                crate::interface::bar::hide_panel(&handle);
            }
            crate::interface::taskbar::hide(&handle, id);
        } else {
            remember_foreground();
            crate::interface::bar::toggle_panel_unfocused(&handle, id);
            crate::interface::taskbar::show(&handle, id);
        }
    });
    winkey::set_enabled(s.win_key_panel, Some(on_tap));
    winshell::hide_taskbar(s.replace_taskbar);
    crate::interface::taskbar::set_enabled(s.replace_taskbar);
    // Red de seguridad: si por lo que sea Inicio llega a abrirse mientras la tecla Windows es de Tesela, se cierra al instante.
    let _ = APP.set(app.clone());
    GUARD.call_once(start_guard);
    HOVER.call_once(|| crate::interface::taskbar::start_hover_trigger(app));
    // Crear o quitar ventanas desde un hilo aparte: en el principal bloquearía el bucle de eventos.
    let app = app.clone();
    let _ = std::thread::Builder::new().name("wm-taskbar-sync".into()).spawn(move || crate::interface::taskbar::sync(&app, &list_monitors()));
}

/// Arranque: devuelve la barra de Windows si un cierre brusco la dejó oculta y aplica los ajustes guardados.
pub fn init(app: &AppHandle) {
    if winshell::recover_taskbar() {
        eprintln!("[wm] se devolvió la barra de tareas de Windows (cierre anterior inesperado)");
    }
    let s = shell_store::load(&shell_store::default_path());
    *CURRENT.lock().unwrap() = Some(s);
    apply(app, s);
}

pub fn set(app: &AppHandle, s: ShellSettings) -> Result<ShellSettings, String> {
    shell_store::save(&shell_store::default_path(), s)?;
    *CURRENT.lock().unwrap() = Some(s);
    apply(app, s);
    Ok(s)
}

/// Al salir: la barra de Windows vuelve a estar como estaba.
pub fn shutdown(_app: &AppHandle) {
    winkey::set_enabled(false, None);
    winshell::hide_taskbar(false);
}
