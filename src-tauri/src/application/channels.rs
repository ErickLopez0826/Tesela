//! Canales de mensajes entre módulos y entre equipos (§10.4: "enviar mensaje a un canal", §10.6 módulo de pareja).
//!
//! El núcleo ofrece canales con nombre; detrás hay un `ChannelBackend`. Hoy solo existe el local (dentro de esta
//! app, con un retardo configurable que imita la red). El backend remoto (Supabase Realtime) queda pendiente de
//! las credenciales de Erick: ver BLOCKERS.md B-10 y `docs/modulo-pareja.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

pub const MAX_CHANNEL_NAME: usize = 64;
pub const MAX_MESSAGE_CHARS: usize = 2000;

pub fn is_valid_channel(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_CHANNEL_NAME && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChannelMessage {
    pub channel: String,
    /// Módulo que lo envió.
    pub from_module: String,
    pub text: String,
    /// Milisegundos desde 1970 en que se envió.
    pub sent_at_ms: u64,
}

pub type Sink = Arc<dyn Fn(ChannelMessage) + Send + Sync>;

pub trait ChannelBackend: Send + Sync {
    /// `"local"` (simulado, no sale de este equipo) o `"remote"`.
    fn kind(&self) -> &'static str;
    /// Retardo típico de entrega en milisegundos (informativo para la interfaz).
    fn latency_ms(&self) -> u64;
    fn send(&self, message: ChannelMessage) -> Result<(), String>;
    fn subscribe(&self, module: &str, channel: &str, sink: Sink) -> Result<(), String>;
    fn unsubscribe(&self, module: &str, channel: &str);
    /// Quita todas las suscripciones de un módulo (al desinstalarlo o desactivarlo).
    fn unsubscribe_module(&self, module: &str);
}

/// Hub local: entrega a todos los suscriptores del canal (incluido quien envía) tras `latency`.
pub struct LocalChannelBackend {
    latency: Duration,
    subs: Mutex<HashMap<(String, String), Sink>>,
}

impl LocalChannelBackend {
    pub fn new(latency: Duration) -> Arc<LocalChannelBackend> {
        Arc::new(LocalChannelBackend { latency, subs: Mutex::new(HashMap::new()) })
    }

    pub fn subscriber_count(&self) -> usize {
        self.subs.lock().unwrap().len()
    }
}

impl ChannelBackend for Arc<LocalChannelBackend> {
    fn kind(&self) -> &'static str {
        "local"
    }

    fn latency_ms(&self) -> u64 {
        self.latency.as_millis() as u64
    }

    fn send(&self, message: ChannelMessage) -> Result<(), String> {
        let targets: Vec<Sink> = self
            .subs
            .lock()
            .unwrap()
            .iter()
            .filter(|((_, channel), _)| *channel == message.channel)
            .map(|(_, sink)| sink.clone())
            .collect();
        if targets.is_empty() {
            return Ok(()); // sin suscriptores: el mensaje se pierde, como en un canal de difusión
        }
        let latency = self.latency;
        std::thread::Builder::new()
            .name("wm-channel".into())
            .spawn(move || {
                std::thread::sleep(latency);
                for sink in targets {
                    // Un suscriptor que falla no impide que los demás reciban el mensaje.
                    let m = message.clone();
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink(m)));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn subscribe(&self, module: &str, channel: &str, sink: Sink) -> Result<(), String> {
        self.subs.lock().unwrap().insert((module.to_string(), channel.to_string()), sink);
        Ok(())
    }

    fn unsubscribe(&self, module: &str, channel: &str) {
        self.subs.lock().unwrap().remove(&(module.to_string(), channel.to_string()));
    }

    fn unsubscribe_module(&self, module: &str) {
        self.subs.lock().unwrap().retain(|(m, _), _| m != module);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn hub(ms: u64) -> Arc<LocalChannelBackend> {
        LocalChannelBackend::new(Duration::from_millis(ms))
    }

    fn msg(channel: &str, from: &str, text: &str) -> ChannelMessage {
        ChannelMessage { channel: channel.into(), from_module: from.into(), text: text.into(), sent_at_ms: 0 }
    }

    fn collector() -> (Sink, Arc<Mutex<Vec<(ChannelMessage, Instant)>>>) {
        let got: Arc<Mutex<Vec<(ChannelMessage, Instant)>>> = Arc::default();
        let g = got.clone();
        (Arc::new(move |m| g.lock().unwrap().push((m, Instant::now()))), got)
    }

    fn wait_for(got: &Arc<Mutex<Vec<(ChannelMessage, Instant)>>>, n: usize) {
        for _ in 0..200 {
            if got.lock().unwrap().len() >= n {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("llegaron {} de {n} mensajes", got.lock().unwrap().len());
    }

    #[test]
    fn nombres_de_canal() {
        for ok in ["pareja", "a-b_c1", &"x".repeat(64)] {
            assert!(is_valid_channel(ok), "{ok}");
        }
        for bad in ["", "Mayús", "con espacio", "a/b", &"x".repeat(65), "ñ"] {
            assert!(!is_valid_channel(bad), "{bad}");
        }
    }

    #[test]
    fn dos_instancias_se_envian_mensajes_con_el_retardo_configurado() {
        let b = hub(80);
        let (sink_a, got_a) = collector();
        let (sink_b, got_b) = collector();
        b.subscribe("a", "pareja", sink_a).unwrap();
        b.subscribe("b", "pareja", sink_b).unwrap();
        let t0 = Instant::now();
        b.send(msg("pareja", "a", "hola")).unwrap();
        wait_for(&got_b, 1);
        let delay = got_b.lock().unwrap()[0].1.duration_since(t0);
        assert!(delay >= Duration::from_millis(80), "respeta el retardo: {delay:?}");
        assert!(delay < Duration::from_millis(500), "pero es razonable: {delay:?}");
        assert_eq!(got_b.lock().unwrap()[0].0.text, "hola");
        // Quien envía también lo recibe (el módulo filtra el suyo por su identidad de instancia).
        wait_for(&got_a, 1);
        // Y en sentido contrario.
        b.send(msg("pareja", "b", "hola a ti")).unwrap();
        wait_for(&got_a, 2);
        assert_eq!(got_a.lock().unwrap()[1].0.from_module, "b");
    }

    #[test]
    fn solo_reciben_los_suscritos_a_ese_canal() {
        let b = hub(1);
        let (s1, g1) = collector();
        let (s2, g2) = collector();
        b.subscribe("a", "uno", s1).unwrap();
        b.subscribe("b", "dos", s2).unwrap();
        b.send(msg("uno", "x", "para uno")).unwrap();
        wait_for(&g1, 1);
        std::thread::sleep(Duration::from_millis(50));
        assert!(g2.lock().unwrap().is_empty());
        b.send(msg("nadie", "x", "al vacío")).unwrap();
    }

    #[test]
    fn quitar_suscripciones_corta_la_entrega() {
        let b = hub(1);
        let (s, g) = collector();
        b.subscribe("a", "c", s.clone()).unwrap();
        b.subscribe("a", "d", s).unwrap();
        assert_eq!(b.subscriber_count(), 2);
        b.unsubscribe("a", "c");
        b.send(msg("c", "x", "no debe llegar")).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        assert!(g.lock().unwrap().is_empty());
        b.unsubscribe_module("a");
        assert_eq!(b.subscriber_count(), 0);
    }

    #[test]
    fn un_suscriptor_que_falla_no_rompe_a_los_demas() {
        let b = hub(1);
        let (ok, got) = collector();
        b.subscribe("malo", "c", Arc::new(|_| panic!("explotó"))).unwrap();
        b.subscribe("bueno", "c", ok).unwrap();
        b.send(msg("c", "x", "hola")).unwrap();
        wait_for(&got, 1);
    }
}
