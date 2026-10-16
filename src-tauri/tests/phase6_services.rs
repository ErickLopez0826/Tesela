//! Fase 6: los módulos de sistema y de acción con los adaptadores reales de Windows.
//! SEGURIDAD: los servicios se construyen SIEMPRE con `allow_changes = false`: apagar, suspender,
//! reiniciar, hibernar y cambiar Wi-Fi/Bluetooth/compartir red se simulan y no llegan al sistema.
//! Volumen: solo se reescribe el valor que ya tenía. Medios y fondo: solo lectura.
//! Se ejecuta a mano: `cargo test --test phase6_services -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

use tesela_lib::domain::system::{PowerAction, RadioKind};
use tesela_lib::infrastructure::windows::system::build_services;

#[test]
#[ignore]
fn todas_las_acciones_del_panel_responden_y_lo_disruptivo_se_simula() {
    let services = build_services(false, std::sync::Arc::default());

    // Estadísticas (lectura real).
    std::thread::sleep(std::time::Duration::from_secs(3));
    let o = services.overview.lock().unwrap().read();
    println!("CPU {:.0}%  GPU {:?}  RAM {}/{}  discos {}  red ↓{} ↑{}  encendido {}s  top {:?}", o.cpu_percent, o.gpu_percent, o.ram_used, o.ram_total, o.disks.len(), o.net_down, o.net_up, o.uptime_secs, o.top_processes.iter().map(|p| &p.name).collect::<Vec<_>>());
    assert!(o.ram_total > 0 && !o.disks.is_empty() && !o.top_processes.is_empty());

    // Volumen: lectura y reescritura del mismo valor.
    let current = services.volume.lock().unwrap().get();
    if let Ok(v) = current {
        println!("volumen {v:?}");
        let mut vol = services.volume.lock().unwrap();
        vol.set_percent(v.percent).unwrap();
        vol.set_muted(v.muted).unwrap();
    }

    // Medios: solo lectura (no se envía ninguna orden a lo que esté sonando).
    let media = services.media.lock().unwrap().now_playing().expect("la API de medios responde");
    println!("medios {media:?}");

    // Fondo: solo lectura.
    println!("fondo {:?}", services.wallpaper.lock().unwrap().current().expect("IDesktopWallpaper responde"));

    // Conectividad: leer sí; cambiar solo se simula y el estado real no cambia.
    let before = services.radios.lock().unwrap().states();
    println!("conectividad {before:?}");
    for kind in [RadioKind::Wifi, RadioKind::Bluetooth, RadioKind::Hotspot] {
        for on in [false, true] {
            let out = services.radios.lock().unwrap().set(kind, on).unwrap();
            assert!(out.simulated, "{kind:?} debe simularse en modo seguro");
        }
    }
    assert_eq!(services.radios.lock().unwrap().states(), before, "el estado real de las radios no cambió");

    // Energía: las cuatro acciones se simulan (el equipo sigue encendido, por supuesto).
    for action in [PowerAction::Shutdown, PowerAction::Suspend, PowerAction::Restart, PowerAction::Hibernate] {
        let out = services.power.lock().unwrap().run(action).unwrap();
        assert!(out.simulated && out.description.contains("Simulado"), "{action:?}: {out:?}");
    }
}
