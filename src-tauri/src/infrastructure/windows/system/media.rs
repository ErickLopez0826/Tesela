//! Controles de medios (§9.2) con las sesiones de transporte multimedia de Windows (SMTC):
//! sirve para Spotify, navegadores, reproductores... lo que muestre el control de volumen de Windows.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::application::system::MediaControl;
use crate::domain::system::{ActionOutcome, MediaCommand, MediaInfo};

/// Cuánto se espera a la API de medios antes de rendirse. Windows a veces deja sin responder a `RequestAsync`
/// (servicio atascado): sin este límite una llamada colgada bloquearía a todas las siguientes.
const TIMEOUT: Duration = Duration::from_millis(2500);

enum Request {
    Now,
    Thumb,
    Send(MediaCommand),
}

enum Reply {
    Now(Result<Option<MediaInfo>, String>),
    Thumb(Result<Option<String>, String>),
    Sent(Result<ActionOutcome, String>),
}

/// Las llamadas WinRT se hacen en un hilo propio: si Windows no responde, solo se atasca ese hilo, y mientras
/// siga atascado las peticiones nuevas fallan al instante en vez de acumularse.
pub struct SmtcMedia {
    tx: Sender<(Request, Sender<Reply>)>,
    busy: Arc<AtomicBool>,
}

impl SmtcMedia {
    pub fn new() -> Self {
        let (tx, rx) = channel::<(Request, Sender<Reply>)>();
        let busy = Arc::new(AtomicBool::new(false));
        let flag = busy.clone();
        let _ = thread::Builder::new().name("wm-media".into()).spawn(move || {
            for (req, reply) in rx {
                flag.store(true, Ordering::SeqCst);
                let out = match req {
                    Request::Now => Reply::Now(now_playing_blocking()),
                    Request::Thumb => Reply::Thumb(thumbnail_blocking()),
                    Request::Send(c) => Reply::Sent(send_blocking(c)),
                };
                flag.store(false, Ordering::SeqCst);
                let _ = reply.send(out);
            }
        });
        SmtcMedia { tx, busy }
    }

    fn ask(&self, req: Request) -> Result<Reply, String> {
        if self.busy.load(Ordering::SeqCst) {
            return Err("el servicio de medios de Windows no responde".into());
        }
        let (rtx, rrx) = channel();
        self.tx.send((req, rtx)).map_err(|_| "el hilo de medios terminó".to_string())?;
        rrx.recv_timeout(TIMEOUT).map_err(|_| "el servicio de medios de Windows no respondió a tiempo".to_string())
    }
}

fn session() -> Result<Option<GlobalSystemMediaTransportControlsSession>, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .and_then(|op| op.join())
        .map_err(|e| e.to_string())?;
    // Sin sesión activa `GetCurrentSession` devuelve un error: no es un fallo, simplemente no suena nada.
    Ok(manager.GetCurrentSession().ok())
}

impl Default for SmtcMedia {
    fn default() -> Self {
        Self::new()
    }
}

/// "Spotify.exe" o "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" -> nombre corto para mostrar.
fn short_app_name(id: &str) -> String {
    let base = id.rsplit(['!', '\\']).next().unwrap_or(id);
    base.trim_end_matches(".exe").to_string()
}

fn now_playing_blocking() -> Result<Option<MediaInfo>, String> {
    let Some(session) = session()? else { return Ok(None) };
    let props = session.TryGetMediaPropertiesAsync().and_then(|op| op.join()).map_err(|e| e.to_string())?;
    let playing = session
        .GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .map(|s| s == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
        .unwrap_or(false);
    let app = session.SourceAppUserModelId().map(|s| short_app_name(&s.to_string())).unwrap_or_default();
    let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
    if title.is_empty() && artist.is_empty() {
        return Ok(None);
    }
    Ok(Some(MediaInfo { title, artist, app, playing }))
}

/// Codifica en base64 estándar (sin dependencias extra: solo se usa para la portada).
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Más grande que esto no se manda a la interfaz (una portada normal pesa decenas de KB).
const MAX_THUMBNAIL: u32 = 3 * 1024 * 1024;

fn thumbnail_blocking() -> Result<Option<String>, String> {
    use windows::Storage::Streams::DataReader;
    let Some(session) = session()? else { return Ok(None) };
    let props = session.TryGetMediaPropertiesAsync().and_then(|op| op.join()).map_err(|e| e.to_string())?;
    let Ok(reference) = props.Thumbnail() else { return Ok(None) };
    let stream = reference.OpenReadAsync().and_then(|op| op.join()).map_err(|e| e.to_string())?;
    let size = stream.Size().map_err(|e| e.to_string())? as u32;
    if size == 0 || size > MAX_THUMBNAIL {
        return Ok(None);
    }
    let mime = stream.ContentType().map(|s| s.to_string()).unwrap_or_default();
    let mime = if mime.starts_with("image/") { mime } else { "image/jpeg".to_string() };
    let input = stream.GetInputStreamAt(0).map_err(|e| e.to_string())?;
    let reader = DataReader::CreateDataReader(&input).map_err(|e| e.to_string())?;
    reader.LoadAsync(size).and_then(|op| op.join()).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).map_err(|e| e.to_string())?;
    Ok(Some(format!("data:{mime};base64,{}", base64(&buf))))
}

fn send_blocking(command: MediaCommand) -> Result<ActionOutcome, String> {
    let Some(session) = session()? else {
        return Err("no hay nada reproduciéndose".into());
    };
    let (op, label) = match command {
        MediaCommand::PlayPause => (session.TryTogglePlayPauseAsync(), "Reproducir/pausar"),
        MediaCommand::Next => (session.TrySkipNextAsync(), "Siguiente"),
        MediaCommand::Previous => (session.TrySkipPreviousAsync(), "Anterior"),
    };
    let accepted = op.and_then(|o| o.join()).map_err(|e| e.to_string())?;
    if accepted {
        Ok(ActionOutcome::done(label))
    } else {
        Err("el reproductor no aceptó la orden".into())
    }
}

impl MediaControl for SmtcMedia {
    fn now_playing(&mut self) -> Result<Option<MediaInfo>, String> {
        match self.ask(Request::Now)? {
            Reply::Now(r) => r,
            _ => Err("respuesta inesperada".into()),
        }
    }

    fn thumbnail(&mut self) -> Result<Option<String>, String> {
        match self.ask(Request::Thumb)? {
            Reply::Thumb(r) => r,
            _ => Err("respuesta inesperada".into()),
        }
    }

    fn send(&mut self, command: MediaCommand) -> Result<ActionOutcome, String> {
        match self.ask(Request::Send(command))? {
            Reply::Sent(r) => r,
            _ => Err("respuesta inesperada".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_coincide_con_los_vectores_estandar() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn nombre_corto_de_la_app() {
        assert_eq!(short_app_name("Spotify.exe"), "Spotify");
        assert_eq!(short_app_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify");
        assert_eq!(short_app_name("chrome.exe"), "chrome");
    }

    /// Solo lectura: no se envía ninguna orden a lo que esté sonando.
    #[test]
    fn lee_la_sesion_de_medios_real_si_la_hay() {
        let mut m = SmtcMedia::new();
        // Si Windows no responde, la llamada falla a tiempo en vez de colgarse (eso es lo que se comprueba).
        let Ok(info) = m.now_playing() else {
            println!("la API de medios no respondió a tiempo (se acepta)");
            return;
        };
        println!("ahora suena: {info:?}");
        if let Some(i) = info {
            assert!(!i.title.is_empty() || !i.artist.is_empty());
        }
    }
}
