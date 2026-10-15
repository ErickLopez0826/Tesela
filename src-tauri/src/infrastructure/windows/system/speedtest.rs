//! Prueba de velocidad con el CLI oficial de Ookla (§14), ejecutado en segundo plano.
//!
//! El CLI NO está instalado en este equipo: mientras no lo esté se usa el adaptador simulado, que devuelve
//! datos de ejemplo marcados como `simulated`. Para instalarlo: `winget install Ookla.Speedtest.CLI`
//! (ver BLOCKERS.md B-9).

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::application::system::SpeedTestControl;
use crate::domain::utility::{SpeedResult, SpeedStatus};

const NO_WINDOW: u32 = 0x0800_0000;

/// Interpreta la salida `--format=json` del CLI de Ookla. El ancho de banda viene en bytes por segundo.
pub fn parse_ookla_json(json: &str) -> Result<SpeedResult, String> {
    let v: Value = serde_json::from_str(json.trim()).map_err(|e| format!("salida de speedtest no válida: {e}"))?;
    if v["type"] == "log" || v.get("error").is_some() {
        return Err(v["message"].as_str().or(v["error"].as_str()).unwrap_or("speedtest falló").to_string());
    }
    let mbps = |path: &str| -> Result<f64, String> {
        v[path]["bandwidth"].as_f64().map(|b| b * 8.0 / 1_000_000.0).ok_or_else(|| format!("falta {path}.bandwidth"))
    };
    let server = {
        let name = v["server"]["name"].as_str().unwrap_or("");
        let loc = v["server"]["location"].as_str().unwrap_or("");
        match (name.is_empty(), loc.is_empty()) {
            (true, true) => String::new(),
            (false, true) => name.to_string(),
            (true, false) => loc.to_string(),
            _ => format!("{name} ({loc})"),
        }
    };
    Ok(SpeedResult {
        download_mbps: mbps("download")?,
        upload_mbps: mbps("upload")?,
        ping_ms: v["ping"]["latency"].as_f64().ok_or("falta ping.latency")?,
        server,
        isp: v["isp"].as_str().unwrap_or("").to_string(),
        simulated: false,
    })
}

/// Busca `speedtest.exe` en el PATH.
pub fn find_ookla() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("speedtest.exe")).find(|p| p.is_file())
}

/// Ejecuta una medición en un hilo y guarda el estado; sirve para el CLI real y para el simulado.
pub struct ThreadedSpeedTest {
    status: Arc<Mutex<SpeedStatus>>,
    run: Arc<dyn Fn() -> Result<SpeedResult, String> + Send + Sync>,
}

impl ThreadedSpeedTest {
    pub fn new(run: impl Fn() -> Result<SpeedResult, String> + Send + Sync + 'static) -> Self {
        ThreadedSpeedTest { status: Arc::new(Mutex::new(SpeedStatus::Idle)), run: Arc::new(run) }
    }

    /// Con el CLI de Ookla real.
    pub fn ookla(exe: PathBuf) -> Self {
        Self::new(move || {
            let out = std::process::Command::new(&exe)
                .args(["--accept-license", "--accept-gdpr", "--format=json"])
                .creation_flags(NO_WINDOW)
                .output()
                .map_err(|e| format!("no se pudo ejecutar speedtest: {e}"))?;
            parse_ookla_json(&String::from_utf8_lossy(&out.stdout)).or_else(|e| {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(if stderr.trim().is_empty() { e } else { format!("{e} ({})", stderr.trim()) })
            })
        })
    }

    /// Datos de ejemplo mientras el CLI no esté instalado (marcados `simulated`).
    pub fn simulated() -> Self {
        Self::new(|| {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            Ok(SpeedResult {
                download_mbps: 250.0,
                upload_mbps: 40.0,
                ping_ms: 12.0,
                server: "Servidor de ejemplo".into(),
                isp: "Simulado".into(),
                simulated: true,
            })
        })
    }
}

impl SpeedTestControl for ThreadedSpeedTest {
    fn start(&mut self) -> Result<(), String> {
        {
            let mut s = self.status.lock().unwrap();
            if matches!(*s, SpeedStatus::Running) {
                return Err("ya hay una prueba en curso".into());
            }
            *s = SpeedStatus::Running;
        }
        let status = self.status.clone();
        let run = self.run.clone();
        std::thread::Builder::new()
            .name("wm-speedtest".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run()))
                    .unwrap_or_else(|_| Err("la prueba falló internamente".into()));
                *status.lock().unwrap() = match outcome {
                    Ok(result) => SpeedStatus::Done { result },
                    Err(error) => SpeedStatus::Failed { error },
                };
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn status(&mut self) -> SpeedStatus {
        self.status.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Salida real de `speedtest --format=json` (estructura documentada por Ookla; valores de ejemplo).
    const SAMPLE: &str = r#"{"type":"result","timestamp":"2026-10-14T22:30:00Z","ping":{"jitter":0.8,"latency":11.5,"low":10.9,"high":13.2},
      "download":{"bandwidth":31250000,"bytes":400000000,"elapsed":15001},
      "upload":{"bandwidth":5000000,"bytes":60000000,"elapsed":12000},
      "packetLoss":0,"isp":"Mi Proveedor","interface":{"internalIp":"192.168.1.5","name":"Ethernet","isVpn":false},
      "server":{"id":1234,"host":"speed.example.net","port":8080,"name":"Ejemplo Net","location":"Ciudad de México","country":"Mexico"},
      "result":{"id":"abc","url":"https://www.speedtest.net/result/c/abc"}}"#;

    #[test]
    fn interpreta_la_salida_del_cli() {
        let r = parse_ookla_json(SAMPLE).unwrap();
        assert!((r.download_mbps - 250.0).abs() < 1e-9, "31 250 000 B/s = 250 Mbps");
        assert!((r.upload_mbps - 40.0).abs() < 1e-9);
        assert!((r.ping_ms - 11.5).abs() < 1e-9);
        assert_eq!(r.server, "Ejemplo Net (Ciudad de México)");
        assert_eq!(r.isp, "Mi Proveedor");
        assert!(!r.simulated);
    }

    #[test]
    fn salidas_con_error_o_incompletas() {
        assert!(parse_ookla_json("").is_err());
        assert!(parse_ookla_json("no es json").is_err());
        let log = r#"{"type":"log","timestamp":"x","message":"Configuration - Could not connect","level":"error"}"#;
        assert!(parse_ookla_json(log).unwrap_err().contains("Could not connect"));
        assert!(parse_ookla_json(r#"{"type":"result","ping":{"latency":1.0}}"#).unwrap_err().contains("download"));
    }

    fn wait_done(t: &mut ThreadedSpeedTest) -> SpeedStatus {
        for _ in 0..100 {
            let s = t.status();
            if !matches!(s, SpeedStatus::Running) {
                return s;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        panic!("la prueba no terminó");
    }

    #[test]
    fn flujo_idle_running_done_y_no_se_solapan_pruebas() {
        let mut t = ThreadedSpeedTest::new(|| {
            std::thread::sleep(std::time::Duration::from_millis(150));
            parse_ookla_json(SAMPLE)
        });
        assert_eq!(t.status(), SpeedStatus::Idle);
        t.start().unwrap();
        assert_eq!(t.status(), SpeedStatus::Running);
        assert!(t.start().unwrap_err().contains("en curso"), "no se lanzan dos a la vez");
        match wait_done(&mut t) {
            SpeedStatus::Done { result } => assert!((result.download_mbps - 250.0).abs() < 1e-9),
            other => panic!("{other:?}"),
        }
        t.start().unwrap(); // se puede repetir
        wait_done(&mut t);
    }

    #[test]
    fn un_fallo_o_un_panico_se_informa_sin_tumbar_nada() {
        let mut t = ThreadedSpeedTest::new(|| Err("sin conexión".into()));
        t.start().unwrap();
        assert_eq!(wait_done(&mut t), SpeedStatus::Failed { error: "sin conexión".into() });
        let mut p = ThreadedSpeedTest::new(|| panic!("explotó"));
        p.start().unwrap();
        assert!(matches!(wait_done(&mut p), SpeedStatus::Failed { .. }));
    }

    #[test]
    fn el_adaptador_simulado_marca_sus_datos_como_simulados() {
        let mut t = ThreadedSpeedTest::simulated();
        t.start().unwrap();
        match wait_done(&mut t) {
            SpeedStatus::Done { result } => assert!(result.simulated),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn busca_el_cli_sin_fallar_si_no_esta() {
        // En este equipo no está instalado; en otros puede estarlo. Solo se comprueba que no entra en pánico.
        let found = find_ookla();
        println!("CLI de Ookla: {found:?}");
    }
}
