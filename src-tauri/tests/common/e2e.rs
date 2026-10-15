//! Utilidades de las pruebas de extremo a extremo de módulos (`--selftest-module`).
#![allow(dead_code)]

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use window_manager_lib::domain::modules::{Version, CORE_VERSION};
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
pub struct DevServer(Option<Child>);

impl DevServer {
    pub fn ensure() -> DevServer {
        // Con un ejecutable de producción (`WM_TEST_EXE`) la interfaz va incrustada: no hace falta servidor.
        if std::env::var_os("WM_TEST_EXE").is_some() || port_open() {
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

pub fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wm-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn store(base: &Path) -> ModuleStore {
    ModuleStore::new(base, Version::parse(CORE_VERSION).unwrap())
}

/// Arranca la app probando un módulo y devuelve el informe que escribió su ventana principal.
pub fn selftest(base: &Path, module: &str) -> Value {
    let out = base.join(format!("informe-{module}-{}.json", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    let exe = std::env::var_os("WM_TEST_EXE").unwrap_or_else(|| env!("CARGO_BIN_EXE_window-manager").into());
    let mut child = Command::new(exe)
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
pub fn outcomes(report: &Value) -> Vec<(String, String)> {
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

pub fn result_of(o: &[(String, String)], method: &str, nth: usize) -> String {
    o.iter().filter(|(m, _)| m == method).nth(nth).map(|x| x.1.clone()).unwrap_or_else(|| format!("(sin llamada {method} #{nth})"))
}

pub fn calls(o: &[(String, String)], method: &str) -> usize {
    o.iter().filter(|(m, _)| m == method).count()
}
