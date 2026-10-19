//! Adaptadores de Windows para los módulos de sistema y de acción (Fase 6).

pub mod app_actions;
pub mod app_audio;
pub mod apps;
pub mod background;
pub mod clipboard;
pub mod installed_apps;
pub mod media;
pub mod overview;
pub mod network;
pub mod power;
pub mod radios;
pub mod search_host;
pub mod speedtest;
pub mod volume;
pub mod wallpaper;
pub mod wallpaper_color;
pub mod wallpaper_engine;

use std::sync::Mutex;

use crate::application::system::{Guarded, SystemServices};

/// Arma los servicios de sistema. Con `allow_changes = false` la energía y la
/// conectividad se simulan: se pueden leer estados, pero nada se enciende, apaga ni reinicia.
pub fn build_services(allow_changes: bool, focus_sink: apps::FocusSink) -> SystemServices {
    SystemServices {
        media: Mutex::new(Box::new(media::SmtcMedia::new())),
        power: Mutex::new(Box::new(Guarded::new(power::LivePower, allow_changes))),
        radios: Mutex::new(Box::new(Guarded::new(radios::WinRtRadios::new(), allow_changes))),
        network: Mutex::new(Box::new(Guarded::new(network::WinRtNetwork::new(), allow_changes))),
        wallpaper: Mutex::new(Box::new(wallpaper::ShellWallpaper::new())),
        engine: Mutex::new(match wallpaper_engine::WallpaperEngine::detect() {
            Some(e) => Box::new(e),
            None => Box::new(wallpaper_engine::NoEngine),
        }),
        volume: Mutex::new(Box::new(volume::CoreAudioVolume::new())),
        overview: Mutex::new(Box::new(overview::LazyOverview::new())),
        clipboard: Mutex::new(Box::new(clipboard::Win32Clipboard)),
        apps: Mutex::new(Box::new(apps::Win32Apps::new(focus_sink))),
        // Con el CLI de Ookla instalado se mide de verdad; si no, datos de ejemplo marcados como simulados.
        speedtest: Mutex::new(Box::new(match speedtest::find_ookla() {
            Some(exe) => speedtest::ThreadedSpeedTest::ookla(exe),
            None => speedtest::ThreadedSpeedTest::simulated(),
        })),
    }
}
