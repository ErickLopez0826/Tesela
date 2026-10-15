//! Raíz de composición: cablea las capas (dominio ← aplicación ← infraestructura/interfaz).

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;

use domain::manageable::Scope;
#[allow(unused_imports)]
use tauri::Manager;

/// Argumentos de línea de comandos (ver DECISIONS.md D-1 y D-3).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CliArgs {
    pub manage: Option<Scope>,
    pub recover: bool,
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

    let app = tauri::Builder::default()
        .manage(interface::commands::AppState::new(store.clone()))
        .invoke_handler(tauri::generate_handler![
            interface::commands::list_windows,
            interface::commands::list_exclusions,
            interface::commands::add_exclusion,
            interface::commands::remove_exclusion,
        ])
        .setup(move |app| {
            #[cfg(windows)]
            if let Some(scope) = args.manage.clone() {
                use std::sync::Arc;
                use tauri::Emitter;

                let handle = app.handle().clone();
                let notice: Arc<dyn Fn(&str) + Send + Sync> = {
                    let h = handle.clone();
                    Arc::new(move |text: &str| {
                        let _ = h.emit("wm-notice", text.to_string());
                    })
                };
                let bar: Arc<dyn Fn(u32, bool) + Send + Sync> = {
                    let h = handle.clone();
                    Arc::new(move |monitor: u32, visible: bool| {
                        let _ = h.emit("wm-bar-visibility", (monitor, visible));
                    })
                };
                let (config, warning) = infrastructure::config_file::load_or_create();
                if let Some(w) = warning {
                    eprintln!("[wm] {w}");
                    notice(&w);
                }
                let options = infrastructure::windows::runtime::RuntimeOptions {
                    loader: Some(Box::new(infrastructure::config_file::load_file)),
                    notice,
                    bar,
                    exclusions: store.list(),
                    on_exit: Box::new(move || handle.exit(0)),
                    ..infrastructure::windows::runtime::RuntimeOptions::from_config(scope, config)
                };
                let runtime = infrastructure::windows::runtime::Runtime::start_with(options);
                let state = app.state::<interface::commands::AppState>();
                *state.manager.lock().unwrap() = Some(runtime);
            }
            let _ = &app;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al construir la aplicación");

    app.run(|_app, event| {
        if let tauri::RunEvent::Exit = event {
            #[cfg(windows)]
            if let Some(state) = _app.try_state::<interface::commands::AppState>() {
                if let Some(rt) = state.manager.lock().unwrap().take() {
                    rt.stop();
                }
            }
        }
    });
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
    fn recover_se_reconoce() {
        assert!(parse_args(a(&["--recover"])).unwrap().recover);
    }
}
