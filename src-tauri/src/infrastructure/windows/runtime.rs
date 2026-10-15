//! Hilos de ejecución del gestor: bucle de mensajes de Windows (hooks y atajos)
//! y el hilo del controlador que decide y aplica.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
    VK_F12,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WINEVENT_OUTOFCONTEXT, WM_HOTKEY, WM_QUIT,
};

use crate::application::controller::{Controller, Message};
use crate::domain::manageable::Scope;
use crate::infrastructure::windows::facts::id_of;
use crate::infrastructure::windows::platform::Win32Platform;

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;

const EMERGENCY_HOTKEY_ID: i32 = 1;

/// Emisor global para los callbacks de hooks (que no admiten estado propio).
static EVENT_TX: Mutex<Option<Sender<Message>>> = Mutex::new(None);

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

pub struct Runtime {
    tx: Sender<Message>,
    platform_thread_id: u32,
    platform_thread: Option<JoinHandle<()>>,
    controller_thread: Option<JoinHandle<()>>,
}

impl Runtime {
    /// Arranca el gestor con el alcance indicado. Adopta las ventanas existentes dentro del alcance.
    pub fn start(scope: Scope) -> Runtime {
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
            .spawn(move || controller_loop(rx, scope))
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

fn controller_loop(rx: Receiver<Message>, scope: Scope) {
    // Si el controlador falla por un pánico, las ventanas ocultas se restauran antes de salir.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut controller = Controller::new(Win32Platform::new(), scope);
        let guard = RestoreOnPanic(&mut controller);
        while let Ok(msg) = rx.recv() {
            if !guard.0.handle(msg) {
                break;
            }
        }
        std::mem::forget(guard);
    }));
    if result.is_err() {
        eprintln!("[wm] el controlador falló; ventanas restauradas");
    }
}

/// Al desenrollarse un pánico, devuelve las ventanas a su sitio.
struct RestoreOnPanic<'a>(&'a mut Controller<Win32Platform>);

impl Drop for RestoreOnPanic<'_> {
    fn drop(&mut self) {
        self.0.handle(Message::Shutdown);
    }
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

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == EMERGENCY_HOTKEY_ID {
                if let Some(tx) = EVENT_TX.lock().unwrap().as_ref() {
                    let _ = tx.send(Message::Emergency);
                }
            }
        }

        if hotkey_ok {
            let _ = UnregisterHotKey(None, EMERGENCY_HOTKEY_ID);
        }
        for h in hooks {
            let _ = UnhookWinEvent(h);
        }
    }
}
