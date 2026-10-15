//! Adaptadores de Windows para los módulos de sistema y de acción (Fase 6).

pub mod media;
pub mod overview;
pub mod power;
pub mod radios;
pub mod volume;
pub mod wallpaper;

use std::sync::Mutex;

use crate::application::system::{Guarded, SystemServices};

/// Arma los servicios de sistema. Con `allow_changes = false` (el valor por defecto) la energía y la
/// conectividad se simulan: se pueden leer estados, pero nada se enciende, apaga ni reinicia.
pub fn build_services(allow_changes: bool) -> SystemServices {
    SystemServices {
        media: Mutex::new(Box::new(media::SmtcMedia::new())),
        power: Mutex::new(Box::new(Guarded::new(power::LivePower, allow_changes))),
        radios: Mutex::new(Box::new(Guarded::new(radios::WinRtRadios::new(), allow_changes))),
        wallpaper: Mutex::new(Box::new(wallpaper::ShellWallpaper::new())),
        volume: Mutex::new(Box::new(volume::CoreAudioVolume::new())),
        overview: Mutex::new(Box::new(overview::LazyOverview::new())),
    }
}
