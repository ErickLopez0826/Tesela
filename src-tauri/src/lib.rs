//! Raíz de composición: cablea las capas (dominio ← aplicación ← infraestructura/interfaz).

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;

use domain::manageable::Scope;
use tauri::Manager;

/// Argumentos de línea de comandos (ver DECISIONS.md D-1, D-3 y D-31).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CliArgs {
    pub manage: Option<Scope>,
    pub recover: bool,
    /// Muestra la barra de cada monitor (reserva espacio en pantalla).
    pub bar: bool,
}

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<CliArgs, String> {
    let mut out = CliArgs::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--manage" => {
                let v = it.next().ok_or("--manage necesita un alcance: all | prefix:TEXTO")?;
                out.manage = Some(Scope::parse(&v).ok_or_else(|| format!("alcance inválido: {v}"))?);
            }
            "--recover" => out.recover = true,
            "--bar" => out.bar = true,
            _ => {}
        }
    }
    Ok(out)
}

pub fn run() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[wm] {e}");
            std::process::exit(2);
        }
    };

    #[cfg(windows)]
    if args.recover {
        let n = infrastructure::windows::platform::recover_cloaked_windows();
        eprintln!("[wm] ventanas recuperadas: {n}");
        return;
    }

    let store = std::sync::Arc::new(infrastructure::exclusion_store::ExclusionStore::load_or_seed(
        infrastructure::exclusion_store::ExclusionStore::default_path(),
    ));
    // La configuración se lee una sola vez al arrancar: la usan el gestor, los atajos y los módulos de sistema.
    let (config, warning) = infrastructure::config_file::load_or_create();
    if let Some(w) = &warning {
        eprintln!("[wm] {w}");
    }
    let allow_changes = config.system.allow_changes;
    #[cfg(windows)]
    let services = std::sync::Arc::new(infrastructure::windows::system::build_services(allow_changes));

    let app = tauri::Builder::default()
        .manage(interface::commands::AppState::new(store.clone(), services.clone(), allow_changes))
        .invoke_handler(tauri::generate_handler![
            interface::commands::list_windows,
            interface::commands::list_exclusions,
            interface::commands::add_exclusion,
            interface::commands::remove_exclusion,
            interface::commands::get_state,
            interface::commands::system_stats,
            interface::commands::run_action,
            interface::commands::open_panel,
            interface::commands::close_panel,
            interface::commands::system_mode,
            interface::commands::overview,
            interface::commands::media_now,
            interface::commands::media_command,
            interface::commands::volume_get,
            interface::commands::volume_set,
            interface::commands::volume_mute,
            interface::commands::radio_states,
            interface::commands::radio_set,
            interface::commands::power_run,
            interface::commands::wallpapers_list,
            interface::commands::wallpaper_set,
        ])
        .setup(move |app| {
            #[cfg(windows)]
            setup_windows(app, &args, &store, config.clone(), warning.clone());
            let _ = (&app, &args);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al construir la aplicación");

    app.run(|_app, event| {
        if let tauri::RunEvent::Exit = event {
            #[cfg(windows)]
            {
                interface::bar::undock_all(_app);
                if let Some(state) = _app.try_state::<interface::commands::AppState>() {
                    if let Some(rt) = state.manager.lock().unwrap().take() {
                        rt.stop();
                    }
                }
            }
        }
    });
}

/// Arranque específico de Windows: barras y gestor de ventanas, según los argumentos.
#[cfg(windows)]
fn setup_windows(
    app: &mut tauri::App,
    args: &CliArgs,
    store: &std::sync::Arc<infrastructure::exclusion_store::ExclusionStore>,
    config: domain::config::ValidConfig,
    warning: Option<String>,
) {
    use std::sync::Arc;
    use tauri::Emitter;

    use infrastructure::windows::platform::Callbacks;
    use infrastructure::windows::runtime::{Runtime, RuntimeOptions};

    let handle = app.handle().clone();

    if args.bar {
        interface::bar::sync_bars(&handle, &infrastructure::windows::facts::list_monitors());
        // Vigila conexiones y desconexiones de monitores para crear y retirar barras.
        let watcher = handle.clone();
        std::thread::spawn(move || {
            let key = |v: &[domain::desktop::Monitor]| {
                v.iter().map(|m| (m.id, m.bounds, m.dpi_scale.to_bits())).collect::<Vec<_>>()
            };
            let mut last = infrastructure::windows::facts::list_monitors();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                // Solo cuenta la geometría real del monitor; el área útil cambia al acoplar nuestras barras.
                let now = infrastructure::windows::facts::list_monitors();
                if key(&now) != key(&last) {
                    // Desde este hilo (no el principal): crear ventanas en el principal bloquearía el bucle.
                    interface::bar::sync_bars(&watcher, &now);
                    last = now;
                }
            }
        });
    }

    let Some(scope) = args.manage.clone() else { return };
    let callbacks = Callbacks {
        notice: {
            let h = handle.clone();
            Arc::new(move |text: &str| {
                let _ = h.emit("wm-notice", text.to_string());
            })
        },
        bar: {
            let h = handle.clone();
            Arc::new(move |monitor: u32, visible: bool| {
                let _ = h.emit("wm-bar-visibility", (monitor, visible));
                interface::bar::set_bar_visible(&h, monitor, visible);
            })
        },
        state: {
            let h = handle.clone();
            Arc::new(move |state: &domain::snapshot::StateSnapshot| {
                if let Some(s) = h.try_state::<interface::commands::AppState>() {
                    *s.last_state.lock().unwrap() = Some(state.clone());
                }
                let _ = h.emit("wm-state", state.clone());
            })
        },
        panel: {
            let h = handle.clone();
            Arc::new(move |monitor: u32| interface::bar::toggle_panel(&h, monitor))
        },
    };
    if let Some(w) = warning {
        (callbacks.notice)(&w);
    }
    let exit_handle = handle.clone();
    let options = RuntimeOptions {
        loader: Some(Box::new(infrastructure::config_file::load_file)),
        callbacks,
        exclusions: store.list(),
        on_exit: Box::new(move || exit_handle.exit(0)),
        ..RuntimeOptions::from_config(scope, config)
    };
    let runtime = Runtime::start_with(options);
    let state = app.state::<interface::commands::AppState>();
    *state.manager.lock().unwrap() = Some(runtime);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sin_argumentos_no_gestiona() {
        assert_eq!(parse_args(a(&[])).unwrap(), CliArgs::default());
    }

    #[test]
    fn manage_con_prefijo() {
        let r = parse_args(a(&["--manage", "prefix:WMTEST"])).unwrap();
        assert_eq!(r.manage, Some(Scope::TitlePrefix("WMTEST".into())));
    }

    #[test]
    fn manage_invalido_falla() {
        assert!(parse_args(a(&["--manage", "xx"])).is_err());
        assert!(parse_args(a(&["--manage"])).is_err());
    }

    #[test]
    fn recover_y_bar_se_reconocen() {
        let r = parse_args(a(&["--recover", "--bar"])).unwrap();
        assert!(r.recover && r.bar);
    }
}
