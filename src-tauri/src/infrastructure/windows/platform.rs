//! Implementación Win32 del puerto `WindowPlatform`.

use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT};
use windows::Win32::UI::WindowsAndMessaging::{
    IsIconic, IsZoomed, PostMessageW, PostThreadMessageW, SetForegroundWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SW_MINIMIZE,
    SWP_NOOWNERZORDER, SWP_NOZORDER, SW_HIDE, SW_RESTORE, SW_SHOWNA, WM_CLOSE,
};

use crate::application::ports::WindowPlatform;
use crate::domain::actions::Action;
use crate::domain::shortcuts::Chord;
use crate::domain::snapshot::StateSnapshot;
use crate::infrastructure::windows::runtime::{HotkeyRequest, HOTKEY_REQUEST, WM_APP_SET_HOTKEYS};
use crate::domain::effects::{Effect, WindowId};
use crate::domain::desktop::Monitor;
use crate::domain::geometry::Rect;
use crate::domain::manageable::WindowFacts;
use crate::infrastructure::windows::facts::{
    facts_of, fullscreen_state, hwnd_from, list_monitors, top_level_windows, visible_frame, window_exists, window_rect,
};

/// `DWMWA_CLOAK`: oculta la ventana sin minimizarla (§17 del plan).
const DWMWA_CLOAK: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(13);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HideMethod {
    Cloak,
    SwHide,
}

/// Avisos hacia la interfaz que la plataforma invoca.
#[derive(Clone)]
pub struct Callbacks {
    pub notice: Arc<dyn Fn(&str) + Send + Sync>,
    pub bar: Arc<dyn Fn(u32, bool) + Send + Sync>,
    pub state: Arc<dyn Fn(&StateSnapshot) + Send + Sync>,
    pub panel: Arc<dyn Fn(u32) + Send + Sync>,
}

impl Callbacks {
    /// Callbacks que no hacen nada (pruebas).
    pub fn silent() -> Self {
        Callbacks { notice: Arc::new(|_| {}), bar: Arc::new(|_, _| {}), state: Arc::new(|_| {}), panel: Arc::new(|_| {}) }
    }
}

#[derive(Default)]
pub struct Win32Platform {
    /// Hilo con el bucle de mensajes que registra los atajos (ver `runtime`).
    hotkey_thread: Option<u32>,
    /// Receptor de avisos para el usuario.
    notice: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Receptor de la visibilidad de la barra por monitor.
    bar: Option<Arc<dyn Fn(u32, bool) + Send + Sync>>,
    /// Receptor del estado visible para la barra y el panel.
    state: Option<Arc<dyn Fn(&StateSnapshot) + Send + Sync>>,
    /// Abre o cierra el panel desplegable de un monitor.
    panel: Option<Arc<dyn Fn(u32) + Send + Sync>>,
    /// Posición original de cada ventana la primera vez que se toca.
    originals: HashMap<WindowId, Rect>,
    /// Ventanas ocultas y con qué método.
    hidden: HashMap<WindowId, HideMethod>,
    /// Último método de ocultación que funcionó (informe para PROGRESS.md).
    pub last_hide_method: Option<HideMethod>,
}

/// Carpeta de estado de la app (`WM_STATE_DIR` en pruebas; `%LOCALAPPDATA%\window-manager` normalmente).
pub fn state_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("WM_STATE_DIR") {
        return d.into();
    }
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&base).join("window-manager")
}

/// Ventanas que la app tiene ocultas por estar en otro workspace (siguen siendo "apps abiertas").
static MANAGED_HIDDEN: std::sync::Mutex<Vec<WindowId>> = std::sync::Mutex::new(Vec::new());

pub fn hidden_by_manager() -> std::collections::HashSet<WindowId> {
    MANAGED_HIDDEN.lock().unwrap().iter().copied().collect()
}

/// Da el foco a una ventana (con el truco de entrada nula para obtener permiso de primer plano).
pub fn focus_window(id: WindowId) {
    Win32Platform::focus(id);
}

fn recovery_file() -> std::path::PathBuf {
    state_dir().join("hidden-windows.txt")
}

impl Win32Platform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_callbacks(thread_id: u32, cb: Callbacks) -> Self {
        Win32Platform {
            hotkey_thread: Some(thread_id),
            notice: Some(cb.notice),
            bar: Some(cb.bar),
            state: Some(cb.state),
            panel: Some(cb.panel),
            ..Default::default()
        }
    }

    /// Guarda las ventanas ocultas para poder recuperarlas con `--recover` tras un cierre brusco (D-3).
    fn persist_hidden(&self) {
        *MANAGED_HIDDEN.lock().unwrap() = self.hidden.keys().copied().collect();
        let path = recovery_file();
        if self.hidden.is_empty() {
            let _ = std::fs::remove_file(&path);
            return;
        }
        let body: String = self
            .hidden
            .keys()
            .map(|&id| format!("{id}\t{}\n", facts_of(id).map(|f| f.title).unwrap_or_default()))
            .collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, body);
    }

    fn remember(&mut self, id: WindowId) {
        if !self.originals.contains_key(&id) {
            if let Some(r) = window_rect(id) {
                self.originals.insert(id, r);
            }
        }
    }

    fn set_cloak(id: WindowId, on: bool) -> bool {
        let value = BOOL(on as i32);
        unsafe {
            DwmSetWindowAttribute(hwnd_from(id), DWMWA_CLOAK, &value as *const _ as *const _, size_of::<BOOL>() as u32)
                .is_ok()
        }
    }

    fn place(id: WindowId, target: Rect) {
        unsafe {
            let hwnd = hwnd_from(id);
            if IsZoomed(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
        }
        // Compensa el borde invisible de sombra para que el contenido visible llene el hueco.
        let (mut x, mut y, mut w, mut h) = (target.x, target.y, target.w, target.h);
        if let (Some(outer), Some(vis)) = (window_rect(id), visible_frame(id)) {
            x -= vis.x - outer.x;
            y -= vis.y - outer.y;
            w += outer.w - vis.w;
            h += outer.h - vis.h;
        }
        Self::set_pos(id, Rect::new(x, y, w, h));
    }

    /// Posiciona el rectángulo exterior tal cual (sin compensar sombras).
    fn set_pos(id: WindowId, r: Rect) {
        unsafe {
            let _ = SetWindowPos(
                hwnd_from(id),
                None,
                r.x,
                r.y,
                r.w,
                r.h,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            );
        }
    }

    fn focus(id: WindowId) {
        unsafe {
            // Una entrada de ratón nula concede permiso de primer plano al proceso.
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 { mi: MOUSEINPUT::default() },
            };
            SendInput(&[input], size_of::<INPUT>() as i32);
            let _ = SetForegroundWindow(hwnd_from(id));
        }
    }

    fn reveal(&mut self, id: WindowId) {
        match self.hidden.remove(&id) {
            Some(HideMethod::Cloak) => {
                Self::set_cloak(id, false);
            }
            Some(HideMethod::SwHide) => unsafe {
                let _ = ShowWindow(hwnd_from(id), SW_SHOWNA);
            },
            None => {}
        }
    }
}

impl WindowPlatform for Win32Platform {
    fn facts(&self, id: WindowId) -> Option<WindowFacts> {
        facts_of(id)
    }

    fn top_level_windows(&self) -> Vec<WindowId> {
        top_level_windows()
    }

    fn monitors(&self) -> Vec<Monitor> {
        list_monitors()
    }

    fn apply(&mut self, effect: &Effect) -> Result<(), String> {
        if let Effect::SetBar { monitor, visible } = effect {
            if let Some(bar) = &self.bar {
                bar(*monitor, *visible);
            }
            return Ok(());
        }
        let id = match effect {
            Effect::Place { id, .. }
            | Effect::Hide(id)
            | Effect::Show(id)
            | Effect::Focus(id)
            | Effect::Close(id)
            | Effect::Minimize(id)
            | Effect::Restore(id) => *id,
            Effect::SetBar { .. } => unreachable!("tratado arriba"),
        };
        if !window_exists(id) {
            return Err("la ventana ya no existe".into());
        }
        match effect {
            Effect::Place { rect, .. } => {
                if unsafe { IsIconic(hwnd_from(id)).as_bool() } {
                    return Ok(());
                }
                self.remember(id);
                Self::place(id, *rect);
            }
            Effect::Hide(_) => {
                if self.hidden.contains_key(&id) {
                    return Ok(());
                }
                self.remember(id);
                if Self::set_cloak(id, true) {
                    self.hidden.insert(id, HideMethod::Cloak);
                    self.last_hide_method = Some(HideMethod::Cloak);
                    self.persist_hidden();
                } else {
                    unsafe {
                        let _ = ShowWindow(hwnd_from(id), SW_HIDE);
                    }
                    self.hidden.insert(id, HideMethod::SwHide);
                    self.last_hide_method = Some(HideMethod::SwHide);
                    self.persist_hidden();
                }
            }
            Effect::SetBar { .. } => {}
            Effect::Minimize(_) => unsafe {
                let _ = ShowWindow(hwnd_from(id), SW_MINIMIZE);
            },
            Effect::Restore(_) => unsafe {
                let _ = ShowWindow(hwnd_from(id), SW_RESTORE);
            },
            Effect::Show(_) => {
                self.reveal(id);
                self.persist_hidden();
            }
            Effect::Focus(_) => Self::focus(id),
            Effect::Close(_) => unsafe {
                let _ = PostMessageW(Some(hwnd_from(id)), WM_CLOSE, Default::default(), Default::default());
            },
        }
        Ok(())
    }

    fn forget(&mut self, id: WindowId) {
        self.originals.remove(&id);
        if self.hidden.remove(&id).is_some() {
            self.persist_hidden();
        }
    }

    fn fullscreen_state(&self, id: WindowId) -> Option<(bool, Option<u32>)> {
        fullscreen_state(id)
    }

    fn set_hotkeys(&mut self, bindings: &[(Chord, Action)]) -> Vec<String> {
        let Some(thread) = self.hotkey_thread else { return vec![] };
        let (reply, rx) = channel();
        *HOTKEY_REQUEST.lock().unwrap() = Some(HotkeyRequest { bindings: bindings.to_vec(), reply });
        let posted = unsafe { PostThreadMessageW(thread, WM_APP_SET_HOTKEYS, WPARAM(0), LPARAM(0)) };
        if posted.is_err() {
            return bindings.iter().map(|(c, _)| c.to_string()).collect();
        }
        rx.recv_timeout(Duration::from_secs(3)).unwrap_or_default()
    }

    fn publish_state(&mut self, state: &StateSnapshot) {
        if let Some(cb) = &self.state {
            cb(state);
        }
    }

    fn toggle_panel(&mut self, monitor: u32) {
        if let Some(cb) = &self.panel {
            cb(monitor);
        }
    }

    fn open_terminal(&mut self) {
        // CREATE_NEW_CONSOLE
        let _ = std::process::Command::new("cmd").creation_flags(0x0000_0010).spawn();
    }

    fn notify(&mut self, text: &str) {
        eprintln!("[wm] {text}");
        if let Some(n) = &self.notice {
            n(text);
        }
    }

    fn restore_all(&mut self) {
        let ids: Vec<WindowId> = self.hidden.keys().copied().collect();
        for id in ids {
            if window_exists(id) {
                self.reveal(id);
            }
        }
        for (&id, &r) in &self.originals {
            if window_exists(id) {
                Self::set_pos(id, r);
            }
        }
        self.originals.clear();
        self.persist_hidden();
    }
}

/// Tras un cierre brusco: quita el cloak de todas las ventanas de nivel superior (D-3).
pub fn recover_cloaked_windows() -> usize {
    let mut n = 0;
    // Ventanas ocultas con SW_HIDE que quedaron registradas: solo si siguen existiendo con el mismo título.
    if let Ok(text) = std::fs::read_to_string(recovery_file()) {
        for line in text.lines() {
            let mut parts = line.splitn(2, '\t');
            let (Some(id), Some(title)) = (parts.next().and_then(|v| v.parse::<isize>().ok()), parts.next()) else {
                continue;
            };
            if facts_of(id).is_some_and(|f| f.title == title && !f.visible) {
                unsafe {
                    let _ = ShowWindow(hwnd_from(id), SW_SHOWNA);
                }
                n += 1;
            }
        }
        let _ = std::fs::remove_file(recovery_file());
    }
    for id in top_level_windows() {
        if facts_of(id).is_some_and(|f| f.cloaked) && Win32Platform::set_cloak(id, false) {
            n += 1;
        }
    }
    n
}
