//! Icono en la bandeja del sistema: permite mostrar la ventana, pausar el gestor (devuelve las ventanas a su
//! sitio), cambiar el arranque con Windows y salir, sin depender de la ventana principal.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, WindowEvent};

const MAIN: &str = "main";

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(windows)]
fn autostart_active() -> bool {
    use crate::infrastructure::windows::autostart::{status, RUN_KEY, VALUE_NAME};
    status(RUN_KEY, VALUE_NAME).is_some()
}

#[cfg(windows)]
fn set_autostart(enabled: bool) -> Result<(), String> {
    use crate::infrastructure::windows::autostart::{command_line, disable, enable, RUN_KEY, VALUE_NAME};
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        enable(RUN_KEY, VALUE_NAME, &command_line(&exe, true, true, true))
    } else {
        disable(RUN_KEY, VALUE_NAME)
    }
}

#[cfg(not(windows))]
fn autostart_active() -> bool {
    false
}

/// Crea el icono de bandeja. Cerrar la ventana principal solo la oculta; «Salir» del menú cierra la app.
/// Con `background` la ventana principal no se muestra al arrancar.
pub fn setup(app: &mut App, background: bool) {
    let built = (|| -> tauri::Result<()> {
        let show = MenuItem::with_id(app, "show", "Mostrar ventana", true, None::<&str>)?;
        let pause = MenuItem::with_id(app, "pause", "Pausar / reanudar gestor de ventanas", true, None::<&str>)?;
        let autostart = CheckMenuItem::with_id(app, "autostart", "Iniciar con Windows", true, autostart_active(), None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
        let sep = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(app, &[&show, &pause, &autostart, &sep, &quit])?;

        let mut builder = TrayIconBuilder::with_id("main")
            .tooltip("Window Manager")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "show" => show_main(app),
                "pause" => {
                    // Misma acción que la tecla de emergencia: pausa el gestor y devuelve las ventanas a su sitio.
                    #[cfg(windows)]
                    if let Some(state) = app.try_state::<crate::interface::commands::AppState>() {
                        if let Some(rt) = state.manager.lock().unwrap().as_ref() {
                            rt.send(crate::application::controller::Message::Emergency);
                        }
                    }
                }
                "autostart" => {
                    #[cfg(windows)]
                    {
                        let want = !autostart_active();
                        if let Err(e) = set_autostart(want) {
                            eprintln!("[wm] arranque con Windows: {e}");
                        }
                        let _ = autostart.set_checked(autostart_active());
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                    show_main(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        builder.build(app)?;
        Ok(())
    })();
    if let Err(e) = built {
        // Sin bandeja la ventana debe seguir cerrándose normalmente.
        eprintln!("[wm] no se pudo crear el icono de la bandeja: {e}");
        return;
    }

    if let Some(w) = app.get_webview_window(MAIN) {
        let hide_target = w.clone();
        w.on_window_event(move |ev| {
            if let WindowEvent::CloseRequested { api, .. } = ev {
                api.prevent_close();
                let _ = hide_target.hide();
            }
        });
        if background {
            let _ = w.hide();
        }
    }
}
