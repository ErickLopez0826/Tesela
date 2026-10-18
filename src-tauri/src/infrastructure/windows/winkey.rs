//! La tecla Windows: tocarla sola nunca llega a Windows (jamás se abre Inicio) y hace lo que Tesela decida (su panel). Los atajos que combinan
//! Windows con otra tecla (Win+Shift+S para capturas, Win+V, Win+D…) siguen funcionando igual; también se anula Ctrl+Esc,
//! que abre Inicio. Es un gancho de teclado de bajo nivel que vive solo mientras Tesela corre.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, VIRTUAL_KEY, VK_CONTROL, VK_ESCAPE, VK_LCONTROL, VK_LWIN,
    VK_RCONTROL, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, KBDLLHOOKSTRUCT, MSG,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

static ENABLED: AtomicBool = AtomicBool::new(false);
static WIN_DOWN: AtomicBool = AtomicBool::new(false);
/// Se pulsó otra tecla mientras Windows estaba pulsada: es un atajo, no un toque.
static COMBO: AtomicBool = AtomicBool::new(false);
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
static ON_TAP: Mutex<Option<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);

/// Lo que hay que hacer con una tecla, según el estado. Pura, para poder probarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Keys {
    pub win_down: bool,
    pub combo: bool,
    pub ctrl_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Deja pasar la tecla.
    Pass,
    /// Era un toque de la tecla Windows: Windows nunca vio la pulsación, así que no hay Inicio que cancelar; se avisa.
    Tap,
    /// Se pulsó otra tecla con Windows retenida: se entrega a Windows la pulsación que se había retenido y luego la tecla.
    ReleaseWinThenPass,
    /// Ctrl+Esc: se traga la tecla y se avisa.
    SwallowAndFire,
    /// El soltado de Esc de un Ctrl+Esc ya tragado.
    Swallow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Win,
    Ctrl,
    Esc,
    Other,
}

pub fn classify(vk: u32) -> Key {
    match VIRTUAL_KEY(vk as u16) {
        VK_LWIN | VK_RWIN => Key::Win,
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Key::Ctrl,
        VK_ESCAPE => Key::Esc,
        _ => Key::Other,
    }
}

/// Actualiza el estado con una pulsación (`down`) o un soltado y dice qué hacer con ella.
pub fn step(state: &mut Keys, key: Key, down: bool) -> Verdict {
    match (key, down) {
        // La pulsación de Windows no llega al sistema hasta saber si es un atajo: así Inicio no puede abrirse con un toque.
        (Key::Win, true) => {
            if !state.win_down {
                state.win_down = true;
                state.combo = false;
            }
            if state.combo { Verdict::Pass } else { Verdict::Swallow }
        }
        (Key::Win, false) => {
            let tap = state.win_down && !state.combo;
            state.win_down = false;
            if tap { Verdict::Tap } else { Verdict::Pass }
        }
        (Key::Ctrl, d) => {
            state.ctrl_down = d;
            if d && state.win_down && !state.combo {
                state.combo = true;
                return Verdict::ReleaseWinThenPass;
            }
            Verdict::Pass
        }
        (Key::Esc, true) if state.ctrl_down && !state.win_down => Verdict::SwallowAndFire,
        (Key::Esc, false) if state.ctrl_down && !state.win_down => Verdict::Swallow,
        (Key::Other | Key::Esc, true) => {
            if state.win_down && !state.combo {
                state.combo = true;
                return Verdict::ReleaseWinThenPass;
            }
            Verdict::Pass
        }
        _ => Verdict::Pass,
    }
}

/// Marca de las teclas que inyecta la propia Tesela: el gancho las deja pasar sin examinarlas. Las inyectadas por otros
/// programas (remapeadores, teclados con software propio) sí se examinan: también pueden abrir Inicio.
const OWN_TAG: usize = 0x7E5E1A;

/// Entrega a Windows la pulsación de su tecla que se había retenido (inyectada), para que el atajo funcione.
fn press_win(win_vk: u16) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(win_vk), dwFlags: Default::default(), dwExtraInfo: OWN_TAG, ..Default::default() } },
    };
    unsafe {
        SendInput(&[input], size_of::<INPUT>() as i32);
    }
}

/// Hilo que ejecuta la acción de cada toque. El gancho no puede hacer nada lento: si tarda más de unos 300 ms Windows ignora
/// lo que devuelve (la tecla pasa tal cual y se abre Inicio) y puede llegar a descartar el gancho. Por eso el gancho solo
/// avisa a este hilo y vuelve al instante.
static WORKER: Mutex<Option<std::sync::mpsc::Sender<Msg>>> = Mutex::new(None);

enum Msg {
    Tap,
    Log(String),
}

fn log_path() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("Tesela").join("state").join("winkey.log"))
}

fn ensure_worker() {
    let mut guard = WORKER.lock().unwrap();
    if guard.is_some() {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let _ = std::thread::Builder::new().name("wm-winkey-tap".into()).spawn(move || {
        use std::io::Write;
        // Registro de diagnóstico (se reinicia en cada arranque): qué ve el gancho y cuánto tarda cada acción.
        let mut file = log_path().and_then(|p| std::fs::File::create(p).ok());
        for msg in rx {
            match msg {
                Msg::Log(line) => {
                    if let Some(f) = file.as_mut() {
                        let _ = writeln!(f, "{line}");
                    }
                }
                Msg::Tap => {
                    let started = std::time::Instant::now();
                    let cb = ON_TAP.lock().unwrap().clone();
                    if let Some(cb) = cb {
                        cb();
                    }
                    if let Some(f) = file.as_mut() {
                        let _ = writeln!(f, "[{}]   acción del toque terminada en {} ms", now_ms(), started.elapsed().as_millis());
                    }
                }
            }
        }
    });
    *guard = Some(tx);
}

fn fire() {
    if let Some(tx) = WORKER.lock().unwrap().as_ref() {
        let _ = tx.send(Msg::Tap);
    }
}

/// Milisegundos de reloj (módulo un millón) para ordenar las líneas del registro.
pub fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() % 1_000_000).unwrap_or(0)
}

/// Anota una línea en `winkey.log` (la escribe el hilo de la tecla, nunca quien llama).
pub fn log(line: String) {
    if let Some(tx) = WORKER.lock().unwrap().as_ref() {
        let _ = tx.send(Msg::Log(line));
    }
}

/// ¿Está activa la anulación de la tecla Windows?
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// La última tecla Windows pulsada (izquierda o derecha), para entregarla si resulta ser un atajo.
static WIN_VK: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0x5B);

static STATE: Mutex<Keys> = Mutex::new(Keys { win_down: false, combo: false, ctrl_down: false });

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && ENABLED.load(Ordering::SeqCst) {
        let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // Las teclas que inyecta la propia Tesela no se vuelven a examinar.
        if k.dwExtraInfo != OWN_TAG {
            let message = wparam.0 as u32;
            let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
            let up = message == WM_KEYUP || message == WM_SYSKEYUP;
            if down || up {
                let key = classify(k.vkCode);
                if key == Key::Win {
                    WIN_VK.store(k.vkCode as u16, Ordering::SeqCst);
                }
                let verdict = step(&mut STATE.lock().unwrap(), key, down);
                WIN_DOWN.store(STATE.lock().unwrap().win_down, Ordering::SeqCst);
                COMBO.store(STATE.lock().unwrap().combo, Ordering::SeqCst);
                CTRL_DOWN.store(STATE.lock().unwrap().ctrl_down, Ordering::SeqCst);
                if key == Key::Win || verdict != Verdict::Pass {
                    log(format!("[{}] ", now_ms()) + &format!("{:?} vk={:#x} {} flags={:#x} extra={:#x} -> {verdict:?}", key, k.vkCode, if down { "abajo" } else { "arriba" }, k.flags.0, k.dwExtraInfo));
                }
                match verdict {
                    Verdict::Tap => {
                        fire();
                        return LRESULT(1);
                    }
                    Verdict::ReleaseWinThenPass => press_win(WIN_VK.load(Ordering::SeqCst)),
                    Verdict::SwallowAndFire => {
                        fire();
                        return LRESULT(1);
                    }
                    Verdict::Swallow => return LRESULT(1),
                    Verdict::Pass => {}
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Enciende o apaga la anulación del menú Inicio. `on_tap` se llama (desde el hilo del gancho; debe ser rápida) en cada toque.
pub fn set_enabled(on: bool, on_tap: Option<Arc<dyn Fn() + Send + Sync>>) {
    if let Some(cb) = on_tap {
        *ON_TAP.lock().unwrap() = Some(cb);
    }
    ensure_worker();
    ENABLED.store(on, Ordering::SeqCst);
    if !on {
        *STATE.lock().unwrap() = Keys::default();
    }
    if on && !STARTED.swap(true, Ordering::SeqCst) {
        spawn_hook_thread();
    }
}

static HOOK_THREAD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static HOOK: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Pone el gancho en un hilo nuevo (con su propio bucle de mensajes) y retira el anterior junto con su hilo.
fn spawn_hook_thread() {
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, UnhookWindowsHookEx, HHOOK, WM_QUIT};
    let _ = std::thread::Builder::new().name("wm-winkey".into()).spawn(|| unsafe {
        let Ok(new) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) else {
            eprintln!("[wm] no se pudo instalar el gancho de la tecla Windows");
            if HOOK.load(Ordering::SeqCst) == 0 {
                STARTED.store(false, Ordering::SeqCst);
            }
            return;
        };
        // Primero el nuevo y luego se quita el viejo: nunca hay un instante sin gancho.
        let old = HOOK.swap(new.0 as isize, Ordering::SeqCst);
        let old_thread = HOOK_THREAD.swap(GetCurrentThreadId(), Ordering::SeqCst);
        if old != 0 {
            let _ = UnhookWindowsHookEx(HHOOK(old as *mut _));
        }
        if old_thread != 0 {
            let _ = PostThreadMessageW(old_thread, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        // Un gancho de bajo nivel necesita un bucle de mensajes en el hilo que lo instala.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

/// Vuelve a poner el gancho en un hilo nuevo. Mientras el panel (un WebView) tiene el foco, el gancho que ya estaba puesto
/// deja de recibir teclas, y con él la tecla Windows llegaba a Windows (abriendo su búsqueda); uno instalado después, en un
/// hilo nuevo, sí las recibe.
pub fn reinstall() {
    if ENABLED.load(Ordering::SeqCst) {
        spawn_hook_thread();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(events: &[(Key, bool)]) -> Vec<Verdict> {
        let mut s = Keys::default();
        events.iter().map(|&(k, d)| step(&mut s, k, d)).collect()
    }

    #[test]
    fn tocar_windows_sola_es_un_toque() {
        let v = run(&[(Key::Win, true), (Key::Win, false)]);
        assert_eq!(v, [Verdict::Swallow, Verdict::Tap]);
    }

    #[test]
    fn windows_con_otra_tecla_es_un_atajo_y_no_se_toca() {
        // Win+Shift+S (captura de pantalla)
        let v = run(&[(Key::Win, true), (Key::Other, true), (Key::Other, true), (Key::Other, false), (Key::Win, false)]);
        // Windows se retiene al pulsarla, se entrega con la primera tecla y de ahí en adelante todo pasa.
        assert_eq!(v, [Verdict::Swallow, Verdict::ReleaseWinThenPass, Verdict::Pass, Verdict::Pass, Verdict::Pass]);
    }

    #[test]
    fn mantener_windows_repite_la_pulsacion_sin_contar_como_atajo() {
        let v = run(&[(Key::Win, true), (Key::Win, true), (Key::Win, true), (Key::Win, false)]);
        assert_eq!(v, [Verdict::Swallow, Verdict::Swallow, Verdict::Swallow, Verdict::Tap]);
    }

    #[test]
    fn un_atajo_no_deja_su_marca_para_el_siguiente_toque() {
        let v = run(&[(Key::Win, true), (Key::Other, true), (Key::Win, false), (Key::Win, true), (Key::Win, false)]);
        assert_eq!(v[2], Verdict::Pass);
        assert_eq!(v[4], Verdict::Tap);
    }

    #[test]
    fn windows_con_control_tambien_es_un_atajo() {
        let v = run(&[(Key::Win, true), (Key::Ctrl, true), (Key::Other, true), (Key::Ctrl, false), (Key::Win, false)]);
        assert_eq!(v, [Verdict::Swallow, Verdict::ReleaseWinThenPass, Verdict::Pass, Verdict::Pass, Verdict::Pass]);
    }

    #[test]
    fn soltar_windows_sin_haberla_visto_pulsar_no_hace_nada() {
        assert_eq!(run(&[(Key::Win, false)]), [Verdict::Pass]);
    }

    #[test]
    fn ctrl_esc_se_traga_y_avisa_pero_esc_solo_pasa() {
        let v = run(&[(Key::Ctrl, true), (Key::Esc, true), (Key::Esc, false), (Key::Ctrl, false), (Key::Esc, true), (Key::Esc, false)]);
        assert_eq!(v, [Verdict::Pass, Verdict::SwallowAndFire, Verdict::Swallow, Verdict::Pass, Verdict::Pass, Verdict::Pass]);
    }

    #[test]
    fn el_aviso_de_un_toque_no_espera_a_que_termine_la_accion() {
        // Aunque la acción tarde mucho, el gancho vuelve al instante (y la acción corre en otro hilo).
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let caller = std::thread::current().id();
        ensure_worker();
        *ON_TAP.lock().unwrap() = Some(Arc::new(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            let _ = done_tx.send(std::thread::current().id());
        }));
        let started = std::time::Instant::now();
        fire();
        assert!(started.elapsed() < std::time::Duration::from_millis(100), "el gancho no debe quedarse esperando la acción");
        let ran_on = done_rx.recv_timeout(std::time::Duration::from_secs(3)).expect("la acción debía ejecutarse");
        assert_ne!(ran_on, caller);
    }

    #[test]
    fn las_teclas_se_reconocen() {
        assert_eq!(classify(0x5B), Key::Win);
        assert_eq!(classify(0x5C), Key::Win);
        assert_eq!(classify(0xA2), Key::Ctrl);
        assert_eq!(classify(0x1B), Key::Esc);
        assert_eq!(classify(0x41), Key::Other);
    }
}
