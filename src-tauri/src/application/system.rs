//! Puertos de los módulos de sistema y de acción (Fase 6) y el modo seguro que protege lo disruptivo.
//!
//! Regla de seguridad (DECISIONS.md D-4 y D-39): apagar, suspender, reiniciar, hibernar y cambiar
//! Wi-Fi/Bluetooth/compartir red SOLO se ejecutan de verdad si la configuración lo permite
//! (`[system] allow_changes = true`, el valor por defecto). Con `false` se simulan, sin llamar al sistema.

use std::sync::Mutex;

use crate::domain::system::{
    ActionOutcome, AudioDevice, BluetoothDevice, EngineLibrary, HotspotInfo, MediaCommand, MediaInfo, PowerAction, RadioKind, RadioState,
    SystemOverview, VolumeState, WifiNetwork,
};
use crate::domain::utility::{OpenApp, SpeedStatus};

pub trait MediaControl: Send {
    fn now_playing(&mut self) -> Result<Option<MediaInfo>, String>;
    fn send(&mut self, command: MediaCommand) -> Result<ActionOutcome, String>;
    /// Portada o miniatura de lo que suena, como «data:image/...;base64,...» (`None` si no la hay).
    fn thumbnail(&mut self) -> Result<Option<String>, String> {
        Ok(None)
    }
}

pub trait PowerControl: Send {
    fn run(&mut self, action: PowerAction) -> Result<ActionOutcome, String>;
}

pub trait RadioControl: Send {
    fn states(&mut self) -> Vec<RadioState>;
    fn set(&mut self, kind: RadioKind, on: bool) -> Result<ActionOutcome, String>;
}

pub trait WallpaperControl: Send {
    /// Ruta de la imagen actual del primer monitor, si se conoce.
    fn current(&mut self) -> Result<Option<String>, String>;
    /// Cambia el fondo con la transición del sistema.
    fn set(&mut self, path: &str) -> Result<ActionOutcome, String>;
}

/// Fondos de Wallpaper Engine: ver la biblioteca y pedirle que cambie de fondo.
pub trait EngineControl: Send {
    fn library(&mut self) -> EngineLibrary;
    /// Aplica un fondo de la biblioteca (por su `id`), en un monitor o en todos.
    fn apply(&mut self, id: &str, monitor: Option<u32>) -> Result<ActionOutcome, String>;
    /// Previsualización (ruta de imagen) del fondo puesto en el monitor principal; de ahí sale el color de la interfaz.
    fn current_preview(&mut self) -> Option<String> {
        None
    }
    /// Previsualización del fondo puesto en un monitor concreto (si no la hay, la del principal).
    fn current_preview_for(&mut self, _monitor: u32) -> Option<String> {
        self.current_preview()
    }
    /// Qué fondo tiene puesto cada monitor ahora.
    fn current(&mut self) -> Vec<crate::domain::system::EngineCurrent> {
        vec![]
    }
}

pub trait VolumeControl: Send {
    fn get(&mut self) -> Result<VolumeState, String>;
    fn set_percent(&mut self, percent: f32) -> Result<ActionOutcome, String>;
    fn set_muted(&mut self, muted: bool) -> Result<ActionOutcome, String>;
    /// Dispositivos de salida de audio activos, con el predeterminado marcado.
    fn outputs(&mut self) -> Result<Vec<AudioDevice>, String> {
        Ok(vec![])
    }
    /// Cambia el dispositivo de salida predeterminado.
    fn set_output(&mut self, _id: &str) -> Result<ActionOutcome, String> {
        Err("este equipo no permite cambiar la salida de audio".into())
    }
}

/// Redes Wi-Fi, dispositivos Bluetooth y zona compartida: ver, conectar y configurar.
pub trait NetworkControl: Send {
    /// Busca las redes a la vista (tarda unos segundos).
    fn wifi_networks(&mut self) -> Result<Vec<WifiNetwork>, String>;
    /// Conecta a una red; con contraseña si es nueva y está protegida.
    fn wifi_connect(&mut self, ssid: &str, password: Option<&str>) -> Result<ActionOutcome, String>;
    fn wifi_disconnect(&mut self) -> Result<ActionOutcome, String>;
    fn bluetooth_devices(&mut self) -> Result<Vec<BluetoothDevice>, String>;
    fn hotspot(&mut self) -> Result<Option<HotspotInfo>, String>;
    fn hotspot_configure(&mut self, ssid: &str, passphrase: &str) -> Result<ActionOutcome, String>;
}

/// Sin red que gestionar (pruebas y equipos sin adaptador).
pub struct NoNetwork;

impl NetworkControl for NoNetwork {
    fn wifi_networks(&mut self) -> Result<Vec<WifiNetwork>, String> {
        Ok(vec![])
    }
    fn wifi_connect(&mut self, _: &str, _: Option<&str>) -> Result<ActionOutcome, String> {
        Err("sin adaptador Wi-Fi".into())
    }
    fn wifi_disconnect(&mut self) -> Result<ActionOutcome, String> {
        Err("sin adaptador Wi-Fi".into())
    }
    fn bluetooth_devices(&mut self) -> Result<Vec<BluetoothDevice>, String> {
        Ok(vec![])
    }
    fn hotspot(&mut self) -> Result<Option<HotspotInfo>, String> {
        Ok(None)
    }
    fn hotspot_configure(&mut self, _: &str, _: &str) -> Result<ActionOutcome, String> {
        Err("sin zona compartida".into())
    }
}

pub trait OverviewProvider: Send {
    fn read(&mut self) -> SystemOverview;
}

/// Portapapeles de texto. `read_text` devuelve `None` si está vacío, no es texto o la aplicación que copió
/// pidió que no se guarde en historiales (gestores de contraseñas).
pub trait ClipboardControl: Send {
    fn read_text(&mut self) -> Result<Option<String>, String>;
    fn write_text(&mut self, text: &str) -> Result<(), String>;
}

/// Portapapeles de mentira con un texto fijo: lo usa la prueba de extremo a extremo (`--selftest-module`) para no
/// leer ni guardar nunca el portapapeles real de la persona que ejecuta las pruebas.
pub struct FixedClipboard(pub String);

impl ClipboardControl for FixedClipboard {
    fn read_text(&mut self) -> Result<Option<String>, String> {
        Ok(Some(self.0.clone()))
    }

    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.0 = text.to_string();
        Ok(())
    }
}

/// Apps abiertas, traer una al frente y abrir programas.
pub trait AppsControl: Send {
    fn open_apps(&mut self) -> Result<Vec<OpenApp>, String>;
    /// Cambia al workspace de la ventana si hace falta y le da el foco.
    fn focus(&mut self, handle: isize) -> Result<(), String>;
    /// Abre un programa o acceso directo (la ruta ya está validada por el núcleo).
    fn launch(&mut self, target: &str) -> Result<(), String>;
}

/// Prueba de velocidad: se inicia y se consulta (tarda decenas de segundos).
pub trait SpeedTestControl: Send {
    fn start(&mut self) -> Result<(), String>;
    fn status(&mut self) -> SpeedStatus;
}

/// Envuelve un adaptador: las lecturas pasan, las escrituras disruptivas solo se hacen si `live`.
pub struct Guarded<T> {
    pub inner: T,
    pub live: bool,
}

impl<T> Guarded<T> {
    pub fn new(inner: T, live: bool) -> Self {
        Guarded { inner, live }
    }
}

impl<T: PowerControl> PowerControl for Guarded<T> {
    fn run(&mut self, action: PowerAction) -> Result<ActionOutcome, String> {
        if self.live {
            self.inner.run(action)
        } else {
            Ok(ActionOutcome::simulated(format!("Simulado: se habría pedido {}", action.label())))
        }
    }
}

impl<T: RadioControl> RadioControl for Guarded<T> {
    fn states(&mut self) -> Vec<RadioState> {
        self.inner.states()
    }

    fn set(&mut self, kind: RadioKind, on: bool) -> Result<ActionOutcome, String> {
        if self.live {
            self.inner.set(kind, on)
        } else {
            let verb = if on { "activar" } else { "desactivar" };
            Ok(ActionOutcome::simulated(format!("Simulado: se habría pedido {verb} {}", kind.label())))
        }
    }
}

impl<T: NetworkControl> NetworkControl for Guarded<T> {
    fn wifi_networks(&mut self) -> Result<Vec<WifiNetwork>, String> {
        self.inner.wifi_networks()
    }
    fn wifi_connect(&mut self, ssid: &str, password: Option<&str>) -> Result<ActionOutcome, String> {
        if self.live {
            self.inner.wifi_connect(ssid, password)
        } else {
            Ok(ActionOutcome::simulated(format!("Simulado: se habría conectado a {ssid}")))
        }
    }
    fn wifi_disconnect(&mut self) -> Result<ActionOutcome, String> {
        if self.live {
            self.inner.wifi_disconnect()
        } else {
            Ok(ActionOutcome::simulated("Simulado: se habría desconectado del Wi-Fi"))
        }
    }
    fn bluetooth_devices(&mut self) -> Result<Vec<BluetoothDevice>, String> {
        self.inner.bluetooth_devices()
    }
    fn hotspot(&mut self) -> Result<Option<HotspotInfo>, String> {
        self.inner.hotspot()
    }
    fn hotspot_configure(&mut self, ssid: &str, passphrase: &str) -> Result<ActionOutcome, String> {
        if self.live {
            self.inner.hotspot_configure(ssid, passphrase)
        } else {
            Ok(ActionOutcome::simulated("Simulado: se habría cambiado la configuración de compartir red"))
        }
    }
}

/// Nombre de red válido para la zona compartida (1-32 caracteres) y contraseña WPA2 (8-63).
pub fn validate_hotspot(ssid: &str, passphrase: &str) -> Result<(), String> {
    if ssid.trim().is_empty() || ssid.chars().count() > 32 {
        return Err("el nombre de la red debe tener entre 1 y 32 caracteres".into());
    }
    if !(8..=63).contains(&passphrase.chars().count()) {
        return Err("la contraseña debe tener entre 8 y 63 caracteres".into());
    }
    Ok(())
}

/// Imágenes disponibles en la carpeta de fondos, por nombre y en orden.
pub fn list_wallpapers(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| crate::domain::system::is_image_file(n))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Convierte el nombre elegido en la interfaz en una ruta dentro de la carpeta de fondos.
/// Solo acepta nombres de archivo simples de imagen que existan allí (nada de rutas ni `..`).
pub fn resolve_wallpaper(dir: &std::path::Path, name: &str) -> Result<std::path::PathBuf, String> {
    let plain = !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && std::path::Path::new(name).file_name().is_some_and(|f| f == std::ffi::OsStr::new(name));
    if !plain {
        return Err(format!("nombre de imagen no válido: {name}"));
    }
    if !crate::domain::system::is_image_file(name) {
        return Err(format!("{name} no es una imagen (jpg, png, bmp, webp)"));
    }
    let path = dir.join(name);
    if !path.is_file() {
        return Err(format!("no hay ninguna imagen llamada {name} en la carpeta de fondos"));
    }
    Ok(path)
}

/// Todos los servicios de sistema que usa la interfaz. Cada uno tras su propio candado para que una
/// lectura lenta (p. ej. medios) no bloquee a las demás.
pub struct SystemServices {
    pub media: Mutex<Box<dyn MediaControl>>,
    pub power: Mutex<Box<dyn PowerControl>>,
    pub radios: Mutex<Box<dyn RadioControl>>,
    pub network: Mutex<Box<dyn NetworkControl>>,
    pub wallpaper: Mutex<Box<dyn WallpaperControl>>,
    pub engine: Mutex<Box<dyn EngineControl>>,
    pub volume: Mutex<Box<dyn VolumeControl>>,
    pub overview: Mutex<Box<dyn OverviewProvider>>,
    pub clipboard: Mutex<Box<dyn ClipboardControl>>,
    pub apps: Mutex<Box<dyn AppsControl>>,
    pub speedtest: Mutex<Box<dyn SpeedTestControl>>,
}

#[cfg(test)]
pub mod fakes {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[derive(Default, Clone)]
    pub struct Calls(pub Arc<StdMutex<Vec<String>>>);

    impl Calls {
        pub fn log(&self, s: impl Into<String>) {
            self.0.lock().unwrap().push(s.into());
        }
        pub fn all(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    pub struct FakePower(pub Calls);
    impl PowerControl for FakePower {
        fn run(&mut self, a: PowerAction) -> Result<ActionOutcome, String> {
            self.0.log(format!("power:{a:?}"));
            Ok(ActionOutcome::done("hecho"))
        }
    }

    pub struct FakeRadios(pub Calls);
    impl RadioControl for FakeRadios {
        fn states(&mut self) -> Vec<RadioState> {
            vec![RadioState { kind: RadioKind::Wifi, available: true, on: true }]
        }
        fn set(&mut self, kind: RadioKind, on: bool) -> Result<ActionOutcome, String> {
            self.0.log(format!("radio:{kind:?}:{on}"));
            Ok(ActionOutcome::done("hecho"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::*;
    use super::*;

    #[test]
    fn en_modo_seguro_la_energia_no_toca_el_sistema() {
        let calls = Calls::default();
        let mut p = Guarded::new(FakePower(calls.clone()), false);
        for a in [PowerAction::Shutdown, PowerAction::Suspend, PowerAction::Restart, PowerAction::Hibernate] {
            let out = p.run(a).unwrap();
            assert!(out.simulated, "{a:?}");
            assert!(out.description.contains("Simulado") && out.description.contains(a.label()));
        }
        assert!(calls.all().is_empty(), "el adaptador real nunca se llamó: {:?}", calls.all());
    }

    #[test]
    fn con_permiso_expreso_la_energia_se_ejecuta() {
        let calls = Calls::default();
        let mut p = Guarded::new(FakePower(calls.clone()), true);
        let out = p.run(PowerAction::Restart).unwrap();
        assert!(!out.simulated);
        assert_eq!(calls.all(), vec!["power:Restart"]);
    }

    #[test]
    fn en_modo_seguro_los_interruptores_se_leen_pero_no_se_cambian() {
        let calls = Calls::default();
        let mut r = Guarded::new(FakeRadios(calls.clone()), false);
        assert_eq!(r.states().len(), 1, "leer sí funciona");
        let out = r.set(RadioKind::Wifi, false).unwrap();
        assert!(out.simulated && out.description.contains("desactivar") && out.description.contains("Wi-Fi"));
        assert!(calls.all().is_empty());
        let mut live = Guarded::new(FakeRadios(calls.clone()), true);
        live.set(RadioKind::Bluetooth, true).unwrap();
        assert_eq!(calls.all(), vec!["radio:Bluetooth:true"]);
    }

    // ----- fondos -----

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("wm-wall-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lista_solo_imagenes_en_orden() {
        let d = temp_dir("lista");
        for n in ["b.PNG", "a.jpg", "notas.txt", "c.exe"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        std::fs::create_dir(d.join("carpeta.png")).unwrap();
        assert_eq!(list_wallpapers(&d), vec!["a.jpg", "b.PNG"]);
        assert!(list_wallpapers(&d.join("no-existe")).is_empty());
    }

    #[test]
    fn resuelve_solo_imagenes_de_la_carpeta() {
        let d = temp_dir("resolve");
        std::fs::write(d.join("playa.jpg"), b"x").unwrap();
        std::fs::write(d.join("virus.exe"), b"x").unwrap();
        assert_eq!(resolve_wallpaper(&d, "playa.jpg").unwrap(), d.join("playa.jpg"));
        for bad in ["..\\playa.jpg", "../playa.jpg", "C:\\Windows\\a.png", "sub/playa.jpg", "..", ".", "", "virus.exe", "falta.png", "C:playa.jpg"] {
            assert!(resolve_wallpaper(&d, bad).is_err(), "'{bad}' debería rechazarse");
        }
    }

    #[test]
    fn la_zona_compartida_valida_nombre_y_contrasena() {
        assert!(validate_hotspot("MiPC", "12345678").is_ok());
        assert!(validate_hotspot("", "12345678").is_err());
        assert!(validate_hotspot(&"x".repeat(33), "12345678").is_err());
        assert!(validate_hotspot("MiPC", "corta").is_err());
        assert!(validate_hotspot("MiPC", &"x".repeat(64)).is_err());
    }

    #[test]
    fn en_modo_seguro_conectar_o_configurar_la_red_se_simula() {
        struct Spy(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
        impl NetworkControl for Spy {
            fn wifi_networks(&mut self) -> Result<Vec<WifiNetwork>, String> { Ok(vec![]) }
            fn wifi_connect(&mut self, s: &str, _: Option<&str>) -> Result<ActionOutcome, String> { self.0.lock().unwrap().push(format!("connect:{s}")); Ok(ActionOutcome::done("ok")) }
            fn wifi_disconnect(&mut self) -> Result<ActionOutcome, String> { self.0.lock().unwrap().push("disconnect".into()); Ok(ActionOutcome::done("ok")) }
            fn bluetooth_devices(&mut self) -> Result<Vec<BluetoothDevice>, String> { Ok(vec![]) }
            fn hotspot(&mut self) -> Result<Option<HotspotInfo>, String> { Ok(None) }
            fn hotspot_configure(&mut self, _: &str, _: &str) -> Result<ActionOutcome, String> { self.0.lock().unwrap().push("configure".into()); Ok(ActionOutcome::done("ok")) }
        }
        let log = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let mut safe = Guarded::new(Spy(log.clone()), false);
        assert!(safe.wifi_connect("Casa", Some("x")).unwrap().simulated);
        assert!(safe.wifi_disconnect().unwrap().simulated);
        assert!(safe.hotspot_configure("a", "12345678").unwrap().simulated);
        assert!(log.lock().unwrap().is_empty(), "nada llegó al sistema");
        let mut live = Guarded::new(Spy(log.clone()), true);
        assert!(!live.wifi_connect("Casa", None).unwrap().simulated);
        assert_eq!(log.lock().unwrap().as_slice(), ["connect:Casa"]);
    }
}
