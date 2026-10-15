//! Hilos de ejecución del gestor: bucle de mensajes de Windows (hooks y atajos)
//! y el hilo del controlador que decide y aplica.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    VK_F12,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WINEVENT_OUTOFCONTEXT, WM_APP, WM_HOTKEY, WM_QUIT,
};

use crate::application::controller::{ConfigLoader, Controller, Message};
use crate::domain::actions::Action;
use crate::domain::config::ValidConfig;
use crate::domain::desktop::Settings;
use crate::domain::manageable::Scope;
use crate::domain::rules::{ExclusionList, WindowRule};
use crate::domain::shortcuts::{Chord, Keymap};
use crate::infrastructure::windows::facts::id_of;
use crate::infrastructure::windows::platform::{Callbacks, Win32Platform};

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;

const EMERGENCY_HOTKEY_ID: i32 = 1;
const FIRST_BINDING_ID: i32 = 100;
/// Mensaje al hilo de plataforma: reemplazar los atajos registrados (ver `HOTKEY_REQUEST`).
pub const WM_APP_SET_HOTKEYS: u32 = WM_APP + 1;

/// Emisor global para los callbacks de hooks (que no admiten estado propio).
static EVENT_TX: Mutex<Option<Sender<Message>>> = Mutex::new(None);

/// Petición pendiente de cambio de atajos, con el canal para devolver los que fallaron.
pub struct HotkeyRequest {
    pub bindings: Vec<(Chord, Action)>,
    pub reply: Sender<Vec<String>>,
}

pub static HOTKEY_REQUEST: Mutex<Option<HotkeyRequest>> = Mutex::new(None);

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != 0 || id_child != 0 || hwnd.0.is_null() {
        return;
    }
    let id = id_of(hwnd);
    let msg = match event {
        EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => Message::Opened(id),
        EVENT_OBJECT_DESTROY => Message::Closed(id),
        EVENT_SYSTEM_FOREGROUND => Message::Focused(id),
        EVENT_SYSTEM_MINIMIZESTART => Message::Minimized(id),
        EVENT_SYSTEM_MINIMIZEEND => Message::Restored(id),
        EVENT_SYSTEM_MOVESIZEEND => Message::MoveEnded(id),
        _ => return,
    };
    if let Some(tx) = EVENT_TX.lock().unwrap().as_ref() {
        let _ = tx.send(msg);
    }
}

pub struct RuntimeOptions {
    pub scope: Scope,
    pub settings: Settings,
    pub keymap: Keymap,
    pub loader: Option<ConfigLoader>,
    /// Avisos hacia la interfaz: avisos, barra, estado y panel.
    pub callbacks: Callbacks,
    pub exclusions: ExclusionList,
    pub rules: Vec<WindowRule>,
    /// Se llama si el usuario pide salir de la app (alt+shift+e).
    pub on_exit: Box<dyn FnOnce() + Send>,
}

impl RuntimeOptions {
    /// Sin atajos configurables ni avisos: para pruebas.
    pub fn basic(scope: Scope) -> Self {
        RuntimeOptions {
            scope,
            settings: Settings::default(),
            keymap: Keymap::default(),
            loader: None,
            callbacks: Callbacks::silent(),
            exclusions: ExclusionList::default(),
            rules: vec![],
            on_exit: Box::new(|| {}),
        }
    }

    pub fn from_config(scope: Scope, config: ValidConfig) -> Self {
        RuntimeOptions { settings: config.settings, keymap: config.keymap, rules: config.rules, ..Self::basic(scope) }
    }
}

pub struct Runtime {
    tx: Sender<Message>,
    platform_thread_id: u32,
    platform_thread: Option<JoinHandle<()>>,
    controller_thread: Option<JoinHandle<()>>,
}

impl Runtime {
    /// Arranca el gestor con el alcance indicado y sin atajos configurables (pruebas).
    pub fn start(scope: Scope) -> Runtime {
        Self::start_with(RuntimeOptions::basic(scope))
    }

    /// Arranca el gestor. Adopta las ventanas existentes dentro del alcance.
    pub fn start_with(options: RuntimeOptions) -> Runtime {
        let (tx, rx) = channel::<Message>();
        *EVENT_TX.lock().unwrap() = Some(tx.clone());

        let (ready_tx, ready_rx) = channel::<u32>();
        let platform_thread = thread::Builder::new()
            .name("wm-platform".into())
            .spawn(move || platform_loop(ready_tx))
            .expect("hilo de plataforma");
        let platform_thread_id = ready_rx.recv().expect("id de hilo");

        let controller_thread = thread::Builder::new()
            .name("wm-controller".into())
            .spawn(move || controller_loop(rx, platform_thread_id, options))
            .expect("hilo del controlador");

        Runtime {
            tx,
            platform_thread_id,
            platform_thread: Some(platform_thread),
            controller_thread: Some(controller_thread),
        }
    }

    pub fn send(&self, msg: Message) {
        let _ = self.tx.send(msg);
    }

    /// Emisor para que otras partes de la app (p. ej. el módulo de apps) manden mensajes al gestor.
    pub fn sender(&self) -> Sender<Message> {
        self.tx.clone()
    }

    /// Detiene el gestor devolviendo las ventanas a su posición original.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Message::Shutdown);
        if let Some(h) = self.controller_thread.take() {
            let _ = h.join();
        }
        unsafe {
            let _ = PostThreadMessageW(self.platform_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(h) = self.platform_thread.take() {
            let _ = h.join();
        }
        *EVENT_TX.lock().unwrap() = None;
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if self.controller_thread.is_some() {
            self.shutdown();
        }
    }
}

fn controller_loop(rx: Receiver<Message>, platform_thread_id: u32, options: RuntimeOptions) {
    let RuntimeOptions { scope, settings, keymap, loader, callbacks, exclusions, rules, on_exit } = options;
    let platform = Win32Platform::with_callbacks(platform_thread_id, callbacks);
    // Si el controlador falla por un pánico, las ventanas ocultas se restauran antes de salir.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut controller = Controller::with_all(platform, scope, settings, keymap, exclusions, rules);
        if let Some(l) = loader {
            controller.set_loader(l);
        }
        let guard = RestoreOnPanic(&mut controller);
        loop {
            // Cada 0,5 s se revisan monitores (conectar/desconectar) y pantallas completas.
            let msg = match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => Message::Tick,
                Err(RecvTimeoutError::Disconnected) => break,
            };
            if !guard.0.handle(msg) {
                break;
            }
        }
        let requested = guard.0.exit_requested();
        std::mem::forget(guard);
        requested
    }));
    match result {
        Ok(true) => on_exit(),
        Ok(false) => {}
        Err(_) => eprintln!("[wm] el controlador falló; ventanas restauradas"),
    }
}

/// Al desenrollarse un pánico, devuelve las ventanas a su sitio.
struct RestoreOnPanic<'a>(&'a mut Controller<Win32Platform>);

impl Drop for RestoreOnPanic<'_> {
    fn drop(&mut self) {
        self.0.handle(Message::Shutdown);
    }
}

fn modifiers_of(chord: &Chord) -> HOT_KEY_MODIFIERS {
    let mut m = HOT_KEY_MODIFIERS(0);
    if chord.mods.ctrl {
        m |= MOD_CONTROL;
    }
    if chord.mods.alt {
        m |= MOD_ALT;
    }
    if chord.mods.shift {
        m |= MOD_SHIFT;
    }
    if chord.mods.win {
        m |= MOD_WIN;
    }
    m
}

fn platform_loop(ready: Sender<u32>) {
    unsafe {
        let hooks: Vec<HWINEVENTHOOK> = [
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_MOVESIZEEND,
            EVENT_SYSTEM_MINIMIZESTART,
            EVENT_SYSTEM_MINIMIZEEND,
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_UNCLOAKED,
        ]
        .iter()
        .map(|&ev| SetWinEventHook(ev, ev, None, Some(win_event_proc), 0, 0, WINEVENT_OUTOFCONTEXT))
        .collect();

        let mods: HOT_KEY_MODIFIERS = MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT;
        let hotkey_ok = RegisterHotKey(None, EMERGENCY_HOTKEY_ID, mods, VK_F12.0 as u32).is_ok();
        if !hotkey_ok {
            eprintln!("[wm] no se pudo registrar el atajo de emergencia Ctrl+Alt+Shift+F12");
        }

        let _ = ready.send(GetCurrentThreadId());

        // id de hotkey -> acción, solo para los atajos configurables.
        let mut actions: HashMap<i32, Action> = HashMap::new();
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_HOTKEY => {
                    let id = msg.wParam.0 as i32;
                    if std::env::var("WM_TRACE").is_ok() {
                        eprintln!("[wm-trace] WM_HOTKEY id={id}");
                    }
                    let out = if id == EMERGENCY_HOTKEY_ID {
                        Some(Message::Emergency)
                    } else {
                        actions.get(&id).map(|a| Message::Action(*a))
                    };
                    if let (Some(m), Some(tx)) = (out, EVENT_TX.lock().unwrap().as_ref()) {
                        let _ = tx.send(m);
                    }
                }
                WM_APP_SET_HOTKEYS => {
                    let Some(req) = HOTKEY_REQUEST.lock().unwrap().take() else { continue };
                    for id in actions.keys() {
                        let _ = UnregisterHotKey(None, *id);
                    }
                    actions.clear();
                    let mut failed = Vec::new();
                    for (i, (chord, action)) in req.bindings.iter().enumerate() {
                        let id = FIRST_BINDING_ID + i as i32;
                        if RegisterHotKey(None, id, modifiers_of(chord), chord.key.virtual_key()).is_ok() {
                            actions.insert(id, *action);
                        } else {
                            failed.push(chord.to_string());
                        }
                    }
                    if std::env::var("WM_TRACE").is_ok() {
                        eprintln!("[wm-trace] hotkeys registrados: {} fallidos: {:?}", actions.len(), failed);
                    }
                    let _ = req.reply.send(failed);
                }
                _ => {}
            }
        }

        for id in actions.keys() {
            let _ = UnregisterHotKey(None, *id);
        }
        if hotkey_ok {
            let _ = UnregisterHotKey(None, EMERGENCY_HOTKEY_ID);
        }
        for h in hooks {
            let _ = UnhookWinEvent(h);
        }
    }
}
