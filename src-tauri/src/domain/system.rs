//! Tipos de los módulos de sistema y de acción (§9.2, §12 Fase 6). Datos puros.

use serde::{Deserialize, Serialize};

/// Resultado de una acción que cambia el sistema. Con el modo seguro activo (por defecto) se simula
/// y `simulated` es `true`: no se tocó nada de verdad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionOutcome {
    pub simulated: bool,
    /// Qué se hizo, o qué se habría hecho.
    pub description: String,
}

impl ActionOutcome {
    pub fn done(description: impl Into<String>) -> Self {
        ActionOutcome { simulated: false, description: description.into() }
    }

    pub fn simulated(description: impl Into<String>) -> Self {
        ActionOutcome { simulated: true, description: description.into() }
    }
}

// ----- Energía -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerAction {
    Shutdown,
    Suspend,
    Restart,
    Hibernate,
}

impl PowerAction {
    pub fn from_name(name: &str) -> Option<PowerAction> {
        Some(match name {
            "shutdown" => PowerAction::Shutdown,
            "suspend" => PowerAction::Suspend,
            "restart" => PowerAction::Restart,
            "hibernate" => PowerAction::Hibernate,
            _ => return None,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            PowerAction::Shutdown => "apagar el equipo",
            PowerAction::Suspend => "suspender el equipo",
            PowerAction::Restart => "reiniciar el equipo",
            PowerAction::Hibernate => "hibernar el equipo",
        }
    }
}

// ----- Conectividad -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadioKind {
    Wifi,
    Bluetooth,
    /// Compartir la conexión (zona Wi-Fi móvil).
    Hotspot,
}

impl RadioKind {
    pub fn from_name(name: &str) -> Option<RadioKind> {
        Some(match name {
            "wifi" => RadioKind::Wifi,
            "bluetooth" => RadioKind::Bluetooth,
            "hotspot" => RadioKind::Hotspot,
            _ => return None,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            RadioKind::Wifi => "Wi-Fi",
            RadioKind::Bluetooth => "Bluetooth",
            RadioKind::Hotspot => "compartir red",
        }
    }
}

/// Estado de un interruptor del sistema: `None` si el equipo no lo tiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RadioState {
    pub kind: RadioKind,
    pub available: bool,
    pub on: bool,
}

/// Red Wi-Fi a la vista (una por nombre, con la mejor señal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WifiNetwork {
    pub ssid: String,
    /// Señal de 0 a 100.
    pub signal: u8,
    pub secured: bool,
    pub connected: bool,
}

/// Dispositivo Bluetooth emparejado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BluetoothDevice {
    pub id: String,
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HotspotClient {
    pub name: String,
    pub mac: String,
}

/// Configuración de compartir red y quién está conectado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HotspotInfo {
    pub ssid: String,
    pub passphrase: String,
    pub clients: Vec<HotspotClient>,
    pub max_clients: u32,
}

/// Dispositivo de salida de audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

/// Fondo de la biblioteca de Wallpaper Engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineWallpaper {
    /// Carpeta del fondo (con «/»): identifica al fondo y es lo único que se acepta para aplicarlo.
    pub id: String,
    pub title: String,
    /// «scene», «video», «web», «application»… (en minúsculas; vacío si no se sabe).
    pub kind: String,
    /// Ruta absoluta de la imagen de previsualización, si tiene.
    pub preview: Option<String>,
}

/// Qué fondo tiene Wallpaper Engine puesto en un monitor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineCurrent {
    pub monitor: u32,
    pub id: String,
}

/// Biblioteca de Wallpaper Engine tal como la ve la interfaz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct EngineLibrary {
    /// `false` si Wallpaper Engine no está instalado.
    pub available: bool,
    pub items: Vec<EngineWallpaper>,
    pub current: Vec<EngineCurrent>,
}

// ----- Medios -----

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub app: String,
    pub playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
}

impl MediaCommand {
    pub fn from_name(name: &str) -> Option<MediaCommand> {
        Some(match name {
            "play_pause" => MediaCommand::PlayPause,
            "next" => MediaCommand::Next,
            "previous" => MediaCommand::Previous,
            _ => return None,
        })
    }
}

// ----- Volumen -----

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct VolumeState {
    /// 0 a 100.
    pub percent: f32,
    pub muted: bool,
}

// ----- Estadísticas detalladas (§9.2) -----

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessUsage {
    pub name: String,
    pub cpu_percent: f32,
    pub ram_bytes: u64,
    /// Uso de la gráfica (0-100); 0 si el equipo no expone el dato.
    pub gpu_percent: f32,
    /// Lectura + escritura en disco, en bytes por segundo.
    pub disk_bytes_per_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiskUsage {
    pub name: String,
    pub used: u64,
    pub total: u64,
}

/// Especificaciones que no cambian mientras la app corre: procesador y tarjeta gráfica.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct HardwareSpecs {
    /// Nombre comercial del procesador (p. ej. «AMD Ryzen 7 7840HS with Radeon 780M Graphics»).
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    /// Frecuencia base/actual informada por Windows, en MHz (0 si no se conoce).
    pub cpu_mhz: u64,
    /// Nombre de la tarjeta gráfica dedicada más potente (o la integrada si no hay otra).
    pub gpu_name: Option<String>,
    /// Memoria de vídeo dedicada en bytes (0 en gráficas integradas que comparten RAM).
    pub gpu_vram: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemOverview {
    pub hardware: HardwareSpecs,
    pub cpu_percent: f32,
    /// `None` si no se pudo leer (el equipo no lo expone).
    pub gpu_percent: Option<f32>,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disks: Vec<DiskUsage>,
    /// Bytes por segundo desde la lectura anterior.
    pub net_down: u64,
    pub net_up: u64,
    pub uptime_secs: u64,
    /// Temperatura del procesador en °C si el sistema la expone sin permisos especiales.
    pub temperature_c: Option<f32>,
    pub top_processes: Vec<ProcessUsage>,
}

// ----- Fondos de pantalla -----

pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "webp"];

pub fn is_image_file(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acciones_de_energia_por_nombre() {
        assert_eq!(PowerAction::from_name("hibernate"), Some(PowerAction::Hibernate));
        assert_eq!(PowerAction::from_name("rm -rf"), None);
    }

    #[test]
    fn radios_y_medios_por_nombre() {
        assert_eq!(RadioKind::from_name("hotspot"), Some(RadioKind::Hotspot));
        assert_eq!(RadioKind::from_name("nfc"), None);
        assert_eq!(MediaCommand::from_name("play_pause"), Some(MediaCommand::PlayPause));
        assert_eq!(MediaCommand::from_name("stop"), None);
    }

    #[test]
    fn solo_extensiones_de_imagen() {
        assert!(is_image_file("Playa.JPG"));
        assert!(is_image_file("a.b.png"));
        assert!(!is_image_file("virus.exe"));
        assert!(!is_image_file("sin_extension"));
    }
}
