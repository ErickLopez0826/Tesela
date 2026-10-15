//! Controles de medios (§9.2) con las sesiones de transporte multimedia de Windows (SMTC):
//! sirve para Spotify, navegadores, reproductores... lo que muestre el control de volumen de Windows.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::application::system::MediaControl;
use crate::domain::system::{ActionOutcome, MediaCommand, MediaInfo};

pub struct SmtcMedia;

impl SmtcMedia {
    pub fn new() -> Self {
        SmtcMedia
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

impl MediaControl for SmtcMedia {
    fn now_playing(&mut self) -> Result<Option<MediaInfo>, String> {
        let Some(session) = Self::session()? else { return Ok(None) };
        let props = session
            .TryGetMediaPropertiesAsync()
            .and_then(|op| op.join())
            .map_err(|e| e.to_string())?;
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

    fn send(&mut self, command: MediaCommand) -> Result<ActionOutcome, String> {
        let Some(session) = Self::session()? else {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let info = m.now_playing().expect("la API de medios responde");
        println!("ahora suena: {info:?}");
        if let Some(i) = info {
            assert!(!i.title.is_empty() || !i.artist.is_empty());
        }
    }
}
