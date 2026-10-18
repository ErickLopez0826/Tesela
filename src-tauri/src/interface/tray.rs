//! Icono en la bandeja del sistema: abre el panel, pausa el gestor (devuelve las ventanas a su sitio), cambia el
//! arranque con Windows y sale. La app no tiene ventana principal: todo se maneja desde la barra y el panel.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager};

/// Abre (o cierra) el panel en el monitor principal.
fn toggle_panel(app: &AppHandle) {
    #[cfg(windows)]
    {
        let monitors = crate::infrastructure::windows::facts::list_monitors();
        if let Some(m) = monitors.iter().find(|m| m.primary).or(monitors.first()) {
            crate::interface::bar::toggle_panel(app, m.id);
        }
    }
    #[cfg(not(windows))]
    let _ = app;
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

/// Crea el icono de bandeja; «Salir» del menú cierra la app.
pub fn setup(app: &mut App) {
    let built = (|| -> tauri::Result<()> {
        let show = MenuItem::with_id(app, "show", "Abrir panel", true, None::<&str>)?;
        let pause = MenuItem::with_id(app, "pause", "Pausar / reanudar gestor de ventanas", true, None::<&str>)?;
        let autostart = CheckMenuItem::with_id(app, "autostart", "Iniciar con Windows", true, autostart_active(), None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
        let sep = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(app, &[&show, &pause, &autostart, &sep, &quit])?;

        let mut builder = TrayIconBuilder::with_id("main")
            .tooltip("Tesela")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(move |app, event| match event.id.as_ref() {
                "show" => toggle_panel(app),
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
                    toggle_panel(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        builder.build(app)?;
        Ok(())
    })();
    if let Err(e) = built {
        eprintln!("[wm] no se pudo crear el icono de la bandeja: {e}");
    }
}
