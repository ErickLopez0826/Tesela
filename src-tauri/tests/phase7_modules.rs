//! Fase 7: módulos instalables de extremo a extremo con la app REAL.
//! Instala módulos con el almacén, arranca la app con `--selftest-module <id>` (su ventana principal carga el
//! módulo en un iframe aislado servido por el protocolo `wmmodule://`, anota lo que le pide a la API del núcleo y
//! escribe un informe) y comprueba permisos, aislamiento y desinstalación limpia.
//!
//! Necesita `npm` en el PATH (levanta el servidor de desarrollo de la interfaz en el puerto 1420 si no está).
//! Se ejecuta a mano: `cargo test --test phase7_modules -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::path::Path;

use common::e2e::*;
use window_manager_lib::domain::modules::Permission;

#[test]
#[ignore]
fn los_permisos_se_cumplen_en_la_app_real_y_la_desinstalacion_no_deja_nada() {
    let _dev = DevServer::ensure();
    let base = tmp("probe");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/modules/probe");
    let s = store(&base);

    // La vista previa muestra lo que pide; el usuario concede SOLO windows y files.
    let preview = s.preview(&fixture).expect("la sonda es válida");
    assert_eq!(preview.permissions.len(), 5);
    s.install(&fixture, &[Permission::Windows, Permission::Files]).expect("instalar la sonda");

    let report = selftest(&base, "probe");
    let o = outcomes(&report);
    println!("sonda ({}): src={} page={} {o:#?}", report["reason"], report["loadedSrc"], report["page"]);
    assert_eq!(report["reason"], "done", "la sonda terminó");

    // Lo concedido funciona…
    assert_eq!(result_of(&o, "core.version", 0), "ok");
    assert_eq!(result_of(&o, "windows.active_workspace", 0), "ok");
    assert_eq!(result_of(&o, "windows.focused", 0), "ok");
    assert_eq!(result_of(&o, "files.write", 0), "ok");
    assert_eq!(result_of(&o, "files.read", 0), "ok");
    assert_eq!(result_of(&o, "events.subscribe", 0), "ok");
    // …y lo no concedido, no: "Instalar un módulo que pide un permiso no concedido: no accede a esa función".
    assert_eq!(result_of(&o, "system.stats", 0), "permission_denied");
    assert_eq!(result_of(&o, "power.run", 0), "permission_denied");
    assert_eq!(result_of(&o, "notify.show", 0), "permission_denied");
    // Errores de uso normales no se confunden con permisos.
    assert_eq!(result_of(&o, "files.write", 1), "bad_params", "escapar de su carpeta se rechaza");
    assert_eq!(result_of(&o, "events.subscribe", 1), "bad_params");
    assert_eq!(result_of(&o, "metodo.inventado", 0), "unknown_method");
    // El iframe aislado no puede saltarse la API del núcleo hablando directamente con Tauri.
    let direct = report["records"].as_array().unwrap().iter().find(|r| r["method"] == "probe.tauri_directo").expect("llamada de aislamiento");
    println!("acceso directo a Tauri desde el módulo: {}", direct["params"]);
    assert_ne!(direct["params"]["result"], "PERMITIDO", "el módulo escapó de su aislamiento: {}", direct["params"]);

    // En disco: escribió dentro de su carpeta y nada fuera de ella.
    assert_eq!(std::fs::read_to_string(base.join("module-data/probe/sonda.txt")).unwrap(), "dentro");
    assert!(!base.join("module-data/fuera.txt").exists() && !base.join("fuera.txt").exists());

    // Desinstalar: sin rastro (archivos, datos propios, registro).
    s.uninstall("probe").expect("desinstalar");
    assert!(!base.join("modules/probe").exists());
    assert!(!base.join("module-data/probe").exists());
    assert!(store(&base).list().is_empty());
    let after = selftest(&base, "probe");
    assert_eq!(after["reason"], "module_not_found", "la app ya no encuentra el módulo");
}

#[test]
#[ignore]
fn el_modulo_de_ejemplo_se_instala_funciona_y_se_desinstala_sin_tocar_el_nucleo() {
    let _dev = DevServer::ensure();
    let base = tmp("hello");
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../modules/examples/hello");
    let s = store(&base);

    // 1. Instalar: se muestran sus permisos y el usuario los acepta.
    let preview = s.preview(&example).expect("el ejemplo es válido");
    println!("permisos que pide: {:?}", preview.permissions.iter().map(|p| p.permission).collect::<Vec<_>>());
    assert_eq!(preview.permissions.len(), 3);
    s.install(&example, &[Permission::Windows, Permission::Files, Permission::Notifications]).expect("instalar");
    assert_eq!(s.list().len(), 1);

    // 2. Funciona: lee/escribe su contador, consulta el escritorio y se suscribe a eventos, todo por la API del núcleo.
    let report = selftest(&base, "hello");
    let o = outcomes(&report);
    println!("hello ({}): src={} page={} {o:#?}", report["reason"], report["loadedSrc"], report["page"]);
    assert_eq!(result_of(&o, "files.read", 0), "ok");
    assert_eq!(result_of(&o, "files.write", 0), "ok");
    assert_eq!(result_of(&o, "windows.active_workspace", 0), "ok");
    assert_eq!(result_of(&o, "windows.focused", 0), "ok");
    assert_eq!(result_of(&o, "events.subscribe", 0), "ok");
    assert_eq!(result_of(&o, "events.subscribe", 1), "ok");
    assert_eq!(std::fs::read_to_string(base.join("module-data/hello/contador.txt")).unwrap(), "1");
    // Una segunda apertura incrementa el contador (el estado persiste en su carpeta).
    selftest(&base, "hello");
    assert_eq!(std::fs::read_to_string(base.join("module-data/hello/contador.txt")).unwrap(), "2");

    // 3. Desinstalar limpiamente.
    s.uninstall("hello").unwrap();
    assert!(!base.join("modules/hello").exists() && !base.join("module-data/hello").exists());
    assert!(store(&base).list().is_empty());
}
