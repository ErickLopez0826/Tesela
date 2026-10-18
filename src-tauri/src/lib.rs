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
    /// No abrir las aplicaciones de arranque (lo usa el reinicio desde Ajustes: ya están abiertas).
    pub no_startup_apps: bool,
    /// Se acepta por compatibilidad (el arranque con Windows lo pasa): la app ya no tiene ventana principal que ocultar.
    pub background: bool,
    /// Prueba de extremo a extremo de un módulo instalado: una ventana de pruebas lo carga, anota lo que
    /// pide a la API del núcleo y escribe el informe en `WM_SELFTEST_OUT` (solo para pruebas).
    pub selftest_module: Option<String>,
    /// Espera antes de arrancar (lo usa el reinicio desde la interfaz para dar tiempo a que la instancia anterior suelte los atajos).
    pub delay_ms: u64,
}

/// Sin ningún argumento (el acceso directo del menú Inicio) la app hace lo que se espera de ella: gestionar las ventanas y
/// mostrar la barra. Con cualquier argumento se respeta lo pedido tal cual.
pub fn with_defaults(mut args: CliArgs) -> CliArgs {
    if args == CliArgs::default() {
        args.manage = Some(Scope::All);
        args.bar = true;
    }
    args
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
            "--background" => out.background = true,
            "--no-startup-apps" => out.no_startup_apps = true,
            "--delay-ms" => {
                out.delay_ms = it.next().and_then(|v| v.parse().ok()).ok_or("--delay-ms necesita un número")?;
            }
            "--selftest-module" => out.selftest_module = Some(it.next().ok_or("--selftest-module necesita el id del módulo")?),
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
    let args = with_defaults(args);

    if args.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(args.delay_ms.min(10_000)));
    }

    #[cfg(windows)]
    if args.recover {
        let n = infrastructure::windows::platform::recover_cloaked_windows();
        eprintln!("[wm] ventanas recuperadas: {n}");
        if infrastructure::windows::winshell::recover_taskbar() {
            eprintln!("[wm] barra de tareas de Windows devuelta");
        }
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
    // El módulo de apps pide enfocar ventanas al gestor; el gestor se conecta a este enlace cuando arranca.
    #[cfg(windows)]
    let focus_sink: infrastructure::windows::system::apps::FocusSink = std::sync::Arc::default();
    #[cfg(windows)]
    let services = std::sync::Arc::new(infrastructure::windows::system::build_services(allow_changes, focus_sink.clone()));

    // Prueba de extremo a extremo de módulos: el portapapeles real nunca se lee ni se copia a disco.
    #[cfg(windows)]
    if args.selftest_module.is_some() {
        *services.clipboard.lock().unwrap() =
            Box::new(application::system::FixedClipboard("texto de prueba del selftest".into()));
    }

    // Módulos instalables (§10): almacén en disco, estado compartido y API del núcleo.
    let modules = std::sync::Arc::new(infrastructure::module_store::ModuleStore::new(
        &infrastructure::config_file::config_dir(),
        domain::modules::Version::parse(domain::modules::CORE_VERSION).expect("versión del núcleo x.y.z"),
    ));
    let last_state = std::sync::Arc::new(interface::commands::LastState::default());
    // El aviso de un módulo necesita el AppHandle, que no existe hasta que arranca la app.
    let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tauri::AppHandle>>> = std::sync::Arc::default();
    let module_api = {
        let slot = handle_slot.clone();
        let slot_for_events = handle_slot.clone();
        std::sync::Arc::new(application::modules::ModuleApi::new(
            modules.clone(),
            last_state.clone(),
            services.clone(),
            std::sync::Arc::new(move |module: &str, text: &str| {
                use tauri::Emitter;
                if let Some(h) = slot.lock().unwrap().as_ref() {
                    let _ = h.emit("wm-notice", format!("{module}: {text}"));
                }
            }),
            // Backend de canales: local y simulado (retardo de 400 ms que imita la red). El remoto (Supabase)
            // espera las credenciales de Erick: ver BLOCKERS.md B-10.
            std::sync::Arc::new(application::channels::LocalChannelBackend::new(std::time::Duration::from_millis(400))),
            {
                let slot = slot_for_events;
                std::sync::Arc::new(move |e: application::modules::ModuleEvent| {
                    use tauri::Emitter;
                    if let Some(h) = slot.lock().unwrap().as_ref() {
                        let _ = h.emit("wm-module-event", serde_json::json!({ "module": e.module, "event": e.event, "payload": e.payload }));
                    }
                })
            },
            domain::modules::CORE_VERSION,
        ))
    };
    let protocol_modules = modules.clone();

    let builder = tauri::Builder::default();
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    let app = builder
        .manage({
            let mut state = interface::commands::AppState::new(
                store.clone(),
                services.clone(),
                allow_changes,
                modules.clone(),
                module_api.clone(),
                last_state.clone(),
            );
            state.wallpaper_transition = config.system.wallpaper_transition;
            state.general = interface::commands::GeneralInfo { managing: args.manage.is_some(), bar: args.bar };
            if let Some(id) = &args.selftest_module {
                state.selftest = Some(interface::commands::SelfTest {
                    module: id.clone(),
                    out: std::env::var("WM_SELFTEST_OUT").unwrap_or_default().into(),
                });
                // Un escritorio de ejemplo para que las consultas `windows.*` tengan qué responder.
                last_state.set(domain::snapshot::sample_state());
            }
            state
        })
        .register_uri_scheme_protocol("wmmodule", move |_ctx, request| {
            interface::modules_protocol::respond(&protocol_modules, request.uri().path())
        })
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
            interface::commands::glass_image,
            interface::commands::tasks_list,
            interface::commands::tasks_save,
            interface::commands::startup_list,
            interface::commands::startup_save,
            interface::commands::app_suggestions,
            interface::commands::quit_app,
            interface::commands::border_enable,
            interface::commands::panel_resize,
            interface::commands::system_mode,
            interface::commands::overview,
            interface::commands::media_now,
            interface::commands::media_command,
            interface::commands::media_thumbnail,
            interface::commands::volume_get,
            interface::commands::volume_set,
            interface::commands::volume_mute,
            interface::commands::radio_states,
            interface::commands::radio_set,
            interface::commands::power_run,
            interface::commands::wallpapers_list,
            interface::commands::wallpaper_set,
            interface::commands::notes_list,
            interface::commands::notes_add,
            interface::commands::notes_update,
            interface::commands::notes_delete,
            interface::commands::notes_export,
            interface::commands::notes_open_folder,
            interface::commands::clipboard_read,
            interface::commands::clipboard_write,
            interface::commands::engine_library,
            interface::commands::engine_apply,
            interface::commands::wifi_networks,
            interface::commands::wifi_connect,
            interface::commands::wifi_disconnect,
            interface::commands::bluetooth_devices,
            interface::commands::hotspot_info,
            interface::commands::hotspot_configure,
            interface::commands::audio_outputs,
            interface::commands::audio_set_output,
            interface::commands::open_windows_settings,
            interface::commands::wallpaper_color,
            interface::commands::shell_get,
            interface::commands::shell_set,
            interface::commands::taskbar_resize,
            interface::commands::taskbar_hide,
            interface::commands::focus_window,
            interface::commands::app_icon,
            interface::commands::modules_list,
            interface::commands::modules_preview,
            interface::commands::modules_install,
            interface::commands::modules_uninstall,
            interface::commands::modules_set_granted,
            interface::commands::modules_set_enabled,
            interface::commands::module_api,
            interface::commands::selftest_config,
            interface::commands::general_status,
            interface::commands::autostart_set,
            interface::commands::restart_app,
            interface::updates::updates_check,
            interface::updates::updates_install,
            interface::commands::selftest_report,
        ])
        .setup(move |app| {
            *handle_slot.lock().unwrap() = Some(app.handle().clone());
            #[cfg(windows)]
            setup_windows(app, &args, &store, config.clone(), warning.clone(), &focus_sink);
            // Las miniaturas del selector de fondos se leen de su carpeta (y solo de ella).
            {
                let dir = app.state::<interface::commands::AppState>().wallpapers_dir.clone();
                let _ = std::fs::create_dir_all(&dir);
                let _ = app.asset_protocol_scope().allow_directory(&dir, false);
            }
            // Y las previsualizaciones de Wallpaper Engine, de sus carpetas de fondos.
            #[cfg(windows)]
            if let Some(engine) = infrastructure::windows::system::wallpaper_engine::WallpaperEngine::detect() {
                for dir in engine.source_dirs() {
                    let _ = app.asset_protocol_scope().allow_directory(dir, true);
                }
            }
            if args.selftest_module.is_none() {
                interface::tray::setup(app);
            } else {
                // Única situación con ventana normal: la prueba de extremo a extremo de un módulo.
                let url = tauri::WebviewUrl::App("index.html".into());
                if let Err(e) = tauri::WebviewWindowBuilder::new(app, "main", url).title("Tesela").inner_size(900.0, 600.0).build() {
                    eprintln!("[wm] no se pudo crear la ventana de pruebas: {e}");
                }
            }
            let _ = (&app, &args);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al construir la aplicación");

    app.run(|_app, event| {
        // Sin ventana principal, que se cierre la última ventana (p. ej. al quitar un monitor) no debe terminar la app:
        // se sale solo desde «Salir» de la bandeja.
        if let tauri::RunEvent::ExitRequested { code: None, api, .. } = &event {
            api.prevent_exit();
        }
        if let tauri::RunEvent::Exit = event {
            #[cfg(windows)]
            {
                interface::bar::undock_all(_app);
                interface::shell::shutdown(_app);
                if let Some(state) = _app.try_state::<interface::commands::AppState>() {
                    if let Some(rt) = state.manager.lock().unwrap().take() {
                        rt.stop();
                    }
                }
                // Con todo ya devuelto a su sitio, termina sin esperar al desmontaje de WebView2: evita que una
                // instancia que se cierra se quede colgada (y compita con la nueva en barras y atajos) y el aviso
                // «Failed to unregister class Chrome_WidgetWin_0».
                std::process::exit(0);
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
    focus_sink: &infrastructure::windows::system::apps::FocusSink,
) {
    use std::sync::Arc;
    use tauri::Emitter;

    use infrastructure::windows::platform::Callbacks;
    use infrastructure::windows::runtime::{Runtime, RuntimeOptions};

    let handle = app.handle().clone();

    if args.bar {
        interface::bar::start_hover_trigger(&handle);
        // Barra de tareas propia y tecla Windows (según sus ajustes; se devuelve la de Windows si quedó oculta).
        interface::shell::init(&handle);
        // Vigila conexiones y desconexiones de monitores para crear y retirar barras.
        let watcher = handle.clone();
        std::thread::spawn(move || {
            let key = |v: &[domain::desktop::Monitor]| {
                v.iter().map(|m| (m.id, m.bounds, m.dpi_scale.to_bits())).collect::<Vec<_>>()
            };
            let mut last = infrastructure::windows::facts::list_monitors();
            // Las barras y la isla se crean desde este hilo, no desde `setup` (hilo principal): crear varias
            // ventanas web seguidas ahí dentro bloquea el arranque («no responde»).
            interface::bar::sync_bars(&watcher, &last);
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

    if args.bar || args.manage.is_some() {
        interface::focus_border::start(&handle);
    }

    // Las aplicaciones de arranque se abren con Tesela (salvo tras un reinicio desde Ajustes, que ya las dejó abiertas).
    let startup_apps = if args.no_startup_apps { Vec::new() } else { infrastructure::startup_store::load(&infrastructure::startup_store::default_path()) };
    let Some(scope) = args.manage.clone() else {
        infrastructure::windows::startup_launch::launch_all(startup_apps, None);
        return;
    };
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
                    let before = s.last_state.set(state.clone());
                    // Los módulos suscritos (y con permiso) reciben los cambios que les interesan.
                    for ev in s.module_api.events_for_state_change(before.as_ref(), state) {
                        let _ = h.emit(
                            "wm-module-event",
                            serde_json::json!({ "module": ev.module, "event": ev.event, "payload": ev.payload }),
                        );
                    }
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
    {
        let tx = runtime.sender();
        *focus_sink.lock().unwrap() =
            Some(Box::new(move |handle| tx.send(application::controller::Message::FocusWindow(handle)).is_ok()));
    }
    infrastructure::windows::startup_launch::launch_all(startup_apps, Some(runtime.sender()));
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
    fn background_se_reconoce() {
        assert!(parse_args(a(&["--background"])).unwrap().background);
        assert!(!parse_args(a(&[])).unwrap().background);
    }

    #[test]
    fn sin_argumentos_gestiona_todo_y_muestra_la_barra_y_con_argumentos_se_respeta_lo_pedido() {
        let d = with_defaults(parse_args(a(&[])).unwrap());
        assert_eq!(d.manage, Some(Scope::All));
        assert!(d.bar);
        // Solo la barra: no se añade la gestión.
        let only_bar = with_defaults(parse_args(a(&["--bar"])).unwrap());
        assert_eq!(only_bar.manage, None);
        // Recuperar ventanas no arranca nada más.
        let recover = with_defaults(parse_args(a(&["--recover"])).unwrap());
        assert!(recover.recover && !recover.bar && recover.manage.is_none());
        // Un reinicio sin gestión ni barra (con espera) tampoco se convierte en otra cosa.
        let restart = with_defaults(parse_args(a(&["--delay-ms", "500"])).unwrap());
        assert!(!restart.bar && restart.manage.is_none());
    }

    #[test]
    fn recover_y_bar_se_reconocen() {
        let r = parse_args(a(&["--recover", "--bar"])).unwrap();
        assert!(r.recover && r.bar);
    }

    #[test]
    fn retraso_de_arranque() {
        assert_eq!(parse_args(a(&["--delay-ms", "1500"])).unwrap().delay_ms, 1500);
        assert!(parse_args(a(&["--delay-ms", "mucho"])).is_err());
        assert_eq!(parse_args(a(&[])).unwrap().delay_ms, 0);
    }

    #[test]
    fn selftest_de_modulo() {
        let r = parse_args(a(&["--selftest-module", "probe"])).unwrap();
        assert_eq!(r.selftest_module.as_deref(), Some("probe"));
        assert!(parse_args(a(&["--selftest-module"])).is_err());
    }
}
