//! Fase 8: la Prueba de velocidad funciona como módulo INSTALABLE (Notas, Apps y Portapapeles dejaron de ser módulos:
//! las notas son ahora una pestaña propia del panel). Se se instala con el almacén (con los permisos que pide), se ejecuta en la app real dentro de su iframe
//! aislado, se le disparan las acciones que declara, y se comprueba lo que hizo en la API del núcleo y en disco.
//!
//! La prueba de velocidad usa el adaptador simulado mientras el CLI de Ookla no esté instalado.
//!
//! Se ejecuta a mano: `cargo test --test phase8_utilities -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};

use common::e2e::*;
use tesela_lib::domain::modules::Permission;

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
fn el_modulo_de_utilidad_se_instala_funciona_y_se_desinstala() {
    let _dev = DevServer::ensure();
    let base = tmp("utilities");

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
    for name in ["speedtest"] {
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
    use tesela_lib::application::controller::Message;
    use tesela_lib::application::system::AppsControl;
    use tesela_lib::domain::actions::Action;
    use tesela_lib::domain::manageable::Scope;
    use tesela_lib::infrastructure::windows::runtime::Runtime;
    use tesela_lib::infrastructure::windows::system::apps::{FocusSink, Win32Apps};

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
        tesela_lib::infrastructure::windows::platform::hidden_by_manager().len() == 2
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
