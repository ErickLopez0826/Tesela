//! Fase 7: módulos instalables de extremo a extremo con la app REAL.
//! Instala módulos con el almacén, arranca la app con `--selftest-module <id>` (su ventana principal carga el
//! módulo en un iframe aislado servido por el protocolo `wmmodule://`, anota lo que le pide a la API del núcleo y
//! escribe un informe) y comprueba permisos, aislamiento y desinstalación limpia.
//!
//! Necesita `npm` en el PATH (levanta el servidor de desarrollo de la interfaz en el puerto 1420 si no está).
//! Se ejecuta a mano: `cargo test --test phase7_modules -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use window_manager_lib::domain::modules::{Permission, Version, CORE_VERSION};
use window_manager_lib::infrastructure::module_store::ModuleStore;

fn port_open() -> bool {
    use std::net::ToSocketAddrs;
    // `localhost` puede resolverse a IPv4 o IPv6 según cómo escuche el servidor.
    "localhost:1420"
        .to_socket_addrs()
        .map(|mut a| a.any(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()))
        .unwrap_or(false)
}

/// Servidor de desarrollo de la interfaz (lo necesita el binario de depuración de Tauri).
struct DevServer(Option<Child>);

impl DevServer {
    fn ensure() -> DevServer {
        if port_open() {
            return DevServer(None);
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let child = Command::new("cmd")
            .args(["/C", "npm", "run", "dev"])
            .current_dir(root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("npm run dev");
        let start = Instant::now();
        while !port_open() {
            assert!(start.elapsed() < Duration::from_secs(60), "el servidor de la interfaz no arrancó");
            std::thread::sleep(Duration::from_millis(300));
        }
        DevServer(Some(child))
    }
}

impl Drop for DevServer {
    fn drop(&mut self) {
        if let Some(c) = &mut self.0 {
            // `npm` lanza procesos hijos: se cierra todo el árbol.
            let _ = Command::new("taskkill").args(["/PID", &c.id().to_string(), "/T", "/F"]).output();
        }
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wm-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn store(base: &Path) -> ModuleStore {
    ModuleStore::new(base, Version::parse(CORE_VERSION).unwrap())
}

/// Arranca la app probando un módulo y devuelve el informe que escribió su ventana principal.
fn selftest(base: &Path, module: &str) -> Value {
    let out = base.join(format!("informe-{module}.json"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_window-manager"))
        .args(["--selftest-module", module])
        .env("WM_CONFIG_DIR", base)
        .env("WM_STATE_DIR", base.join("state"))
        .env("WM_SELFTEST_OUT", &out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("arrancar la app");
    let start = Instant::now();
    while !out.exists() {
        if start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            panic!("la app no escribió el informe de la prueba del módulo {module}");
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    // La app se cierra sola tras escribir el informe.
    let end = Instant::now();
    while child.try_wait().unwrap().is_none() && end.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = child.kill();
    serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).expect("informe JSON")
}

/// (método, resultado) de cada llamada que el módulo hizo al núcleo; el resultado es "ok" o el código del error.
fn outcomes(report: &Value) -> Vec<(String, String)> {
    report["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|r| {
            let result = if r["ok"] == true {
                "ok".to_string()
            } else {
                r["error"].as_str().unwrap_or("").split(':').next().unwrap_or("").to_string()
            };
            (r["method"].as_str().unwrap().to_string(), result)
        })
        .collect()
}

fn result_of(o: &[(String, String)], method: &str, nth: usize) -> String {
    o.iter().filter(|(m, _)| m == method).nth(nth).map(|x| x.1.clone()).unwrap_or_else(|| format!("(sin llamada {method} #{nth})"))
}

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
