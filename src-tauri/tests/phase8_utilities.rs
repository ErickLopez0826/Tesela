//! Fase 8: Notas, Apps abiertas/fijadas, Portapapeles y Prueba de velocidad funcionan como módulos INSTALABLES.
//! Cada uno se instala con el almacén (con los permisos que pide), se ejecuta en la app real dentro de su iframe
//! aislado, se le disparan las acciones que declara, y se comprueba lo que hizo en la API del núcleo y en disco.
//!
//! Privacidad: el portapapeles de la prueba es un texto fijo (nunca se lee el real); de las apps abiertas solo se
//! comprueba que la consulta funcionó, sin registrar nombres de ventanas. La prueba de velocidad usa el adaptador
//! simulado mientras el CLI de Ookla no esté instalado.
//!
//! Se ejecuta a mano: `cargo test --test phase8_utilities -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};

use common::e2e::*;
use window_manager_lib::domain::modules::Permission;

fn module_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../modules").join(name)
}

fn install_all_requested(base: &Path, name: &str) -> Vec<Permission> {
    let s = store(base);
    let preview = s.preview(&module_dir(name)).unwrap_or_else(|e| panic!("{name} no es válido: {e:?}"));
    let perms: Vec<Permission> = preview.permissions.iter().map(|p| p.permission).collect();
    s.install(&module_dir(name), &perms).unwrap_or_else(|e| panic!("instalar {name}: {e:?}"));
    perms
}

fn assert_clean_uninstall(base: &Path, id: &str) {
    store(base).uninstall(id).unwrap();
    assert!(!base.join("modules").join(id).exists(), "{id}: quedaron archivos");
    assert!(!base.join("module-data").join(id).exists(), "{id}: quedaron datos");
    assert!(store(base).list().iter().all(|m| m.manifest.id != id), "{id}: quedó en el registro");
}

#[test]
#[ignore]
fn los_cuatro_modulos_de_utilidad_se_instalan_funcionan_y_se_desinstalan() {
    let _dev = DevServer::ensure();
    let base = tmp("utilities");

    // --- Notas: lista, crea y guarda notas como archivos de una carpeta local ---------------------------------
    let perms = install_all_requested(&base, "notes");
    assert_eq!(perms, vec![Permission::Files]);
    let r = selftest(&base, "notes");
    let o = outcomes(&r);
    println!("notas ({}): {o:?}", r["reason"]);
    assert_eq!(result_of(&o, "files.dir", 0), "ok");
    assert_eq!(result_of(&o, "files.list", 0), "ok");
    assert_eq!(result_of(&o, "files.write", 0), "ok", "nota de bienvenida");
    assert!(calls(&o, "files.write") >= 2, "la acción «Nueva nota» también escribió: {o:?}");
    assert!(o.iter().all(|(_, res)| res == "ok"), "todo lo que pidió estaba permitido: {o:?}");
    let notes = base.join("module-data/notes/notas");
    assert!(notes.join("Bienvenida.md").exists() && notes.join("Nueva nota.md").exists());
    assert!(std::fs::read_to_string(notes.join("Bienvenida.md")).unwrap().contains("# Bienvenida"));
    assert_clean_uninstall(&base, "notes");

    // --- Apps abiertas y fijadas: lista las ventanas reales y lee las fijadas ----------------------------------
    let perms = install_all_requested(&base, "apps");
    assert_eq!(perms.len(), 3);
    let r = selftest(&base, "apps");
    let o = outcomes(&r);
    println!("apps ({}): {} llamadas, todas ok: {}", r["reason"], o.len(), o.iter().all(|x| x.1 == "ok"));
    assert_eq!(result_of(&o, "files.read", 0), "ok", "fijadas");
    assert!(calls(&o, "apps.list_open") >= 1);
    assert_eq!(result_of(&o, "apps.list_open", 0), "ok", "lista las apps abiertas del sistema");
    assert!(o.iter().all(|(_, res)| res == "ok"), "{o:?}");
    assert_clean_uninstall(&base, "apps");

    // --- Portapapeles: registra lo copiado (texto fijo de la prueba) y «Limpiar» lo borra ------------------------
    let perms = install_all_requested(&base, "clipboard");
    assert_eq!(perms.len(), 2);
    let r = selftest(&base, "clipboard");
    let o = outcomes(&r);
    println!("portapapeles ({}): {o:?}", r["reason"]);
    assert_eq!(result_of(&o, "clipboard.read", 0), "ok");
    assert!(calls(&o, "files.write") >= 2, "guardó el historial y luego lo limpió");
    assert!(o.iter().all(|(_, res)| res == "ok"), "{o:?}");
    // La acción «Limpiar historial» fue lo último que hizo: el historial quedó vacío.
    assert_eq!(std::fs::read_to_string(base.join("module-data/clipboard/historial.json")).unwrap(), "[]");
    assert_clean_uninstall(&base, "clipboard");

    // --- Prueba de velocidad: se inicia, se consulta y se guarda el resultado ------------------------------------
    let perms = install_all_requested(&base, "speedtest");
    assert_eq!(perms.len(), 2);
    let r = selftest(&base, "speedtest");
    let o = outcomes(&r);
    println!("velocidad ({}): {o:?}", r["reason"]);
    assert_eq!(result_of(&o, "speedtest.start", 0), "ok");
    assert!(calls(&o, "speedtest.status") >= 2);
    assert!(o.iter().all(|(_, res)| res == "ok"), "{o:?}");
    let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(base.join("module-data/speedtest/historial.json")).unwrap()).unwrap();
    assert_eq!(saved.as_array().unwrap().len(), 1, "guardó la medición");
    assert!(saved[0]["download_mbps"].as_f64().unwrap() > 0.0);
    // Con el CLI de Ookla sin instalar la medición es simulada y así queda marcada.
    println!("medición simulada: {}", saved[0]["simulated"]);
    assert_clean_uninstall(&base, "speedtest");

    // Todo se desinstaló sin dejar nada.
    assert!(store(&base).list().is_empty());
}

#[test]
#[ignore]
fn sin_los_permisos_concedidos_ningun_modulo_de_utilidad_accede_al_sistema() {
    let _dev = DevServer::ensure();
    let base = tmp("utilities-denied");
    // Se instalan sin conceder NADA: el módulo corre pero cada función real devuelve permission_denied.
    for name in ["apps", "clipboard"] {
        store(&base).install(&module_dir(name), &[]).unwrap();
        let r = selftest(&base, name);
        let o = outcomes(&r);
        println!("{name} sin permisos ({}): {o:?}", r["reason"]);
        assert!(!o.is_empty());
        // `core.version` es libre (no da acceso a nada); todo lo demás debe ser rechazado.
        let real: Vec<_> = o.iter().filter(|(m, _)| m != "core.version").collect();
        assert!(!real.is_empty() && real.iter().all(|(_, res)| res == "permission_denied"), "{name} no debió conseguir nada: {o:?}");
        assert!(!base.join("module-data").join(name).exists(), "{name} no pudo escribir nada");
    }
}

/// La lista de "apps abiertas" incluye las ventanas que la app oculta por estar en otro workspace, y enfocar una
/// de ellas cambia de workspace y la vuelve a mostrar. Usa ventanas WMTEST y el gestor real (alcance `prefix:WMTEST`).
#[test]
#[ignore]
fn enfocar_una_app_de_otro_workspace_la_trae_de_vuelta() {
    use std::sync::{Arc, Mutex};
    use window_manager_lib::application::controller::Message;
    use window_manager_lib::application::system::AppsControl;
    use window_manager_lib::domain::actions::Action;
    use window_manager_lib::domain::manageable::Scope;
    use window_manager_lib::infrastructure::windows::runtime::Runtime;
    use window_manager_lib::infrastructure::windows::system::apps::{FocusSink, Win32Apps};

    std::env::set_var("WM_STATE_DIR", std::env::temp_dir().join("wm-test-state"));
    let windows = common::spawn_windows(2, "WMTEST app");
    let rt = Runtime::start(Scope::TitlePrefix("WMTEST".into()));
    let sink: FocusSink = Arc::new(Mutex::new(None));
    let tx = rt.sender();
    *sink.lock().unwrap() = Some(Box::new(move |h| tx.send(Message::FocusWindow(h)).is_ok()));
    let mut apps = Win32Apps::new(sink);

    common::wait_until("las ventanas ya están en el layout", || {
        let f = common::frame(windows.ids[0]);
        f.w > 100 && f.w < 1500 && !common::hidden(windows.ids[0])
    });
    // Se mandan a un workspace oculto.
    rt.send(Message::Action(Action::FocusWorkspace(5)));
    common::wait_until("quedan ocultas", || windows.ids.iter().all(|&id| common::hidden(id)));

    // La app anota las ventanas que oculta un instante después de ocultarlas.
    common::wait_until("la app anota las ocultas", || {
        window_manager_lib::infrastructure::windows::platform::hidden_by_manager().len() == 2
    });
    // Siguen apareciendo como apps abiertas aunque estén ocultas.
    let listed: Vec<_> = apps.open_apps().unwrap().into_iter().filter(|a| a.title.starts_with("WMTEST")).collect();
    assert_eq!(listed.len(), 2, "las dos ventanas ocultas se listan: {listed:?}");
    assert!(listed.iter().all(|a| !a.path.is_empty()), "con la ruta del ejecutable para poder fijarlas");

    // Enfocar una trae su workspace de vuelta.
    apps.focus(windows.ids[0]).unwrap();
    common::wait_until("la ventana vuelve a verse", || !common::hidden(windows.ids[0]));
    rt.stop();
}
