//! Puertos de los módulos de sistema y de acción (Fase 6) y el modo seguro que protege lo disruptivo.
//!
//! Regla de seguridad (DECISIONS.md D-4 y D-39): apagar, suspender, reiniciar, hibernar y cambiar
//! Wi-Fi/Bluetooth/compartir red SOLO se ejecutan de verdad si la configuración lo permite
//! (`[system] allow_changes = true`). Por defecto se simulan, sin llamar al sistema.

use std::sync::Mutex;

use crate::domain::system::{
    ActionOutcome, MediaCommand, MediaInfo, PowerAction, RadioKind, RadioState, SystemOverview, VolumeState,
};

pub trait MediaControl: Send {
    fn now_playing(&mut self) -> Result<Option<MediaInfo>, String>;
    fn send(&mut self, command: MediaCommand) -> Result<ActionOutcome, String>;
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

pub trait VolumeControl: Send {
    fn get(&mut self) -> Result<VolumeState, String>;
    fn set_percent(&mut self, percent: f32) -> Result<ActionOutcome, String>;
    fn set_muted(&mut self, muted: bool) -> Result<ActionOutcome, String>;
}

pub trait OverviewProvider: Send {
    fn read(&mut self) -> SystemOverview;
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
    pub wallpaper: Mutex<Box<dyn WallpaperControl>>,
    pub volume: Mutex<Box<dyn VolumeControl>>,
    pub overview: Mutex<Box<dyn OverviewProvider>>,
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
}
