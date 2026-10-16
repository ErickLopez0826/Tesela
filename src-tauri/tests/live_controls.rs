//! Comprobación REAL (con los adaptadores de Windows) de los controles del panel. Está marcada `ignore` porque
//! cambia cosas de verdad, aunque siempre las devuelve a como estaban:
//!   - volumen: baja 1 punto, lo comprueba y lo restaura;
//!   - Bluetooth: solo si está APAGADO lo enciende, lo comprueba y lo vuelve a apagar (nunca toca Wi-Fi ni
//!     compartir red, porque cortarían la conexión);
//!   - medios: la lectura debe volver a tiempo aunque el servicio de Windows no responda.
//! Energía no se prueba aquí: apagar, suspender, reiniciar o hibernar el equipo es algo que solo se prueba a mano.
//!
//! Ejecutar: `cargo test --test live_controls -- --ignored --nocapture --test-threads=1`

use std::time::{Duration, Instant};

use tesela_lib::application::system::{MediaControl, RadioControl, VolumeControl};
use tesela_lib::domain::system::RadioKind;
use tesela_lib::infrastructure::windows::system::{media::SmtcMedia, radios::WinRtRadios, volume::CoreAudioVolume};

#[test]
#[ignore = "toca el volumen y Bluetooth reales (los restaura)"]
fn el_volumen_se_puede_cambiar_y_restaurar() {
    let mut v = CoreAudioVolume::new();
    let before = v.get().expect("leer el volumen");
    let target = if before.percent >= 1.0 { before.percent - 1.0 } else { before.percent + 1.0 };
    let out = v.set_percent(target).expect("cambiar el volumen");
    assert!(!out.simulated);
    let during = v.get().unwrap();
    assert!((during.percent - target).abs() < 1.0, "el volumen debería estar en {target}, está en {}", during.percent);
    v.set_percent(before.percent).unwrap();
    let after = v.get().unwrap();
    assert!((after.percent - before.percent).abs() < 1.0, "no se restauró el volumen");
    println!("volumen {} -> {} -> {}", before.percent, during.percent, after.percent);
}

#[test]
#[ignore = "enciende y vuelve a apagar Bluetooth"]
fn bluetooth_se_puede_encender_y_apagar() {
    let mut r = WinRtRadios::new();
    let states = r.states();
    let bt = states.iter().find(|s| s.kind == RadioKind::Bluetooth).expect("estado de Bluetooth");
    if !bt.available {
        println!("este equipo no tiene Bluetooth: se omite");
        return;
    }
    if bt.on {
        println!("Bluetooth está ENCENDIDO: no se toca para no desconectar tus dispositivos");
        return;
    }
    let out = r.set(RadioKind::Bluetooth, true).expect("encender Bluetooth");
    assert!(!out.simulated);
    std::thread::sleep(Duration::from_millis(1500));
    let on = r.states().into_iter().find(|s| s.kind == RadioKind::Bluetooth).unwrap();
    // Se devuelve a su estado original ocurra lo que ocurra con la comprobación.
    let back = r.set(RadioKind::Bluetooth, false);
    std::thread::sleep(Duration::from_millis(1500));
    let off = r.states().into_iter().find(|s| s.kind == RadioKind::Bluetooth).unwrap();
    assert!(on.on, "Bluetooth debería haberse encendido");
    back.expect("apagar Bluetooth de nuevo");
    assert!(!off.on, "Bluetooth debería haber vuelto a apagarse");
    println!("Bluetooth: apagado -> {} -> {}", on.on, off.on);
}

#[test]
#[ignore = "usa el servicio de medios real"]
fn la_lectura_de_medios_vuelve_a_tiempo_aunque_windows_no_responda() {
    let mut m = SmtcMedia::new();
    let start = Instant::now();
    let r = m.now_playing();
    let took = start.elapsed();
    println!("medios: {r:?} en {took:?}");
    assert!(took < Duration::from_secs(4), "la lectura de medios tardó {took:?}");
    // Una segunda llamada nunca debe quedarse esperando a la primera.
    let start = Instant::now();
    let _ = m.now_playing();
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[test]
#[ignore = "usa el servicio de medios real (solo lectura)"]
fn la_portada_de_lo_que_suena_se_lee_como_imagen() {
    let mut m = SmtcMedia::new();
    match m.thumbnail() {
        Ok(Some(url)) => {
            println!("portada: {} bytes de texto, empieza por {}", url.len(), &url[..url.len().min(40)]);
            assert!(url.starts_with("data:image/") && url.contains(";base64,"));
            assert!(url.len() > 200, "una portada real pesa más que esto");
        }
        Ok(None) => println!("lo que suena no tiene portada (se acepta)"),
        Err(e) => println!("sin respuesta de medios: {e} (se acepta)"),
    }
}

#[test]
#[ignore = "lee el fondo de pantalla real (solo lectura)"]
fn el_color_del_fondo_se_lee_del_fondo_real() {
    use tesela_lib::application::system::WallpaperControl;
    use tesela_lib::infrastructure::windows::system::{wallpaper::ShellWallpaper, wallpaper_color::color_of};
    let Some(path) = ShellWallpaper::new().current().expect("leer el fondo") else {
        println!("sin fondo de imagen: se omite");
        return;
    };
    let hex = color_of(&path).expect("calcular el color");
    println!("color del fondo {path}: {hex}");
    assert!(hex.len() == 7 && hex.starts_with('#'));
}

#[test]
#[ignore = "lee las estadísticas reales (solo lectura)"]
fn las_apps_traen_consumo_de_gpu_y_disco() {
    use tesela_lib::application::system::OverviewProvider;
    use tesela_lib::infrastructure::windows::system::overview::SysinfoOverview;
    let mut o = SysinfoOverview::new();
    std::thread::sleep(Duration::from_secs(5)); // el contador de GPU necesita dos muestras
    let r = o.read();
    for p in &r.top_processes {
        println!("{:<28} cpu {:>5.1}%  ram {:>6} MB  gpu {:>5.1}%  disco {} B/s", p.name, p.cpu_percent, p.ram_bytes / 1048576, p.gpu_percent, p.disk_bytes_per_sec);
    }
    assert!(!r.top_processes.is_empty());
    println!("GPU total: {:?}", r.gpu_percent);
    println!("apps con GPU > 0: {}", r.top_processes.iter().filter(|p| p.gpu_percent > 0.0).count());
}

#[test]
#[ignore = "lee redes, Bluetooth, zona compartida y salidas de audio reales (solo lectura; reasigna la salida actual a sí misma)"]
fn redes_dispositivos_y_salidas_de_audio_se_leen() {
    use tesela_lib::application::system::NetworkControl;
    use tesela_lib::infrastructure::windows::system::network::WinRtNetwork;
    let mut n = WinRtNetwork::new();
    let t = Instant::now();
    match n.wifi_networks() {
        Ok(l) => {
            println!("redes ({:?}):", t.elapsed());
            for w in l.iter().take(8) {
                println!("  {} señal {} protegida {} conectada {}", w.ssid, w.signal, w.secured, w.connected);
            }
        }
        Err(e) => println!("redes: {e}"),
    }
    match n.bluetooth_devices() {
        Ok(l) => println!("bluetooth ({} aparatos): {:?}", l.len(), l.iter().map(|d| format!("{}={}", d.name, d.connected)).collect::<Vec<_>>()),
        Err(e) => println!("bluetooth: {e}"),
    }
    println!("zona compartida: {:?}", n.hotspot().map(|h| h.map(|h| (h.ssid, h.clients.len(), h.max_clients))));
    let mut v = CoreAudioVolume::new();
    let outs = v.outputs().expect("salidas de audio");
    println!("salidas: {outs:?}");
    if let Some(cur) = outs.iter().find(|d| d.default) {
        v.set_output(&cur.id).expect("reasignar la salida actual");
        assert!(v.outputs().unwrap().iter().any(|d| d.default && d.id == cur.id));
    }
}

#[test]
#[ignore = "lee la biblioteca real de Wallpaper Engine; vuelve a aplicar el fondo que ya está puesto"]
fn la_biblioteca_de_wallpaper_engine_se_lee_y_se_reaplica_el_fondo_actual() {
    use tesela_lib::application::system::EngineControl;
    use tesela_lib::infrastructure::windows::system::wallpaper_engine::WallpaperEngine;
    let Some(mut engine) = WallpaperEngine::detect() else {
        println!("Wallpaper Engine no está instalado: se omite");
        return;
    };
    let t = Instant::now();
    let lib = engine.library();
    println!("{} fondos en {:?}", lib.items.len(), t.elapsed());
    let t = Instant::now();
    let again = engine.library();
    println!("segunda lectura (caché): {:?}", t.elapsed());
    assert_eq!(lib.items.len(), again.items.len());
    assert!(lib.items.len() > 10, "debería haber muchos fondos");
    let kinds: std::collections::BTreeMap<_, usize> = lib.items.iter().fold(Default::default(), |mut m, w| { *m.entry(w.kind.clone()).or_default() += 1; m });
    println!("tipos: {kinds:?}; con preview: {}", lib.items.iter().filter(|w| w.preview.is_some()).count());
    println!("puestos ahora: {:?}", lib.current);
    for c in &lib.current {
        if let Some(w) = lib.items.iter().find(|w| w.id == c.id) {
            println!("monitor {} -> {}", c.monitor, w.title);
        }
    }
    // Idempotente: se vuelve a pedir el mismo fondo que ya está puesto en el monitor 0.
    if let Some(c) = lib.current.first() {
        if lib.items.iter().any(|w| w.id == c.id) {
            println!("{:?}", engine.apply(&c.id, Some(c.monitor)));
        }
    }
}

#[test]
#[ignore = "lee la previsualización del fondo actual de Wallpaper Engine (solo lectura)"]
fn el_color_sale_del_fondo_de_wallpaper_engine_y_no_del_fondo_oscuro_de_windows() {
    use tesela_lib::application::system::EngineControl;
    use tesela_lib::infrastructure::windows::system::{wallpaper_color::color_of, wallpaper_engine::WallpaperEngine};
    let Some(mut engine) = WallpaperEngine::detect() else {
        println!("Wallpaper Engine no está instalado: se omite");
        return;
    };
    let lib = engine.library();
    for c in &lib.current {
        let Some(w) = lib.items.iter().find(|w| w.id == c.id) else { continue };
        let Some(p) = &w.preview else { continue };
        println!("monitor {} «{}»: {:?}", c.monitor, w.title, color_of(p));
    }
    let preview = engine.current_preview().expect("el fondo puesto debería tener previsualización");
    let hex = color_of(&preview).expect("color de la previsualización");
    println!("color usado por la interfaz: {hex} (el fondo de Windows daba #191717)");
    assert_ne!(hex, "#191717");
}

#[test]
#[ignore = "dibuja unos segundos la transición con la previsualización del fondo YA puesto (se ve igual, detrás de las ventanas)"]
fn la_transicion_se_dibuja_en_una_ventana_propia_y_desaparece() {
    use std::sync::Arc;
    use tesela_lib::application::system::EngineControl;
    use tesela_lib::infrastructure::windows::facts::list_monitors;
    use tesela_lib::infrastructure::windows::fade::{self, Overlay};
    use tesela_lib::infrastructure::windows::system::wallpaper_engine::WallpaperEngine;

    dpi_aware();
    let Some(mut engine) = WallpaperEngine::detect() else { return };
    let Some(preview) = engine.current_preview() else { return };
    let overlays: Vec<Overlay> = list_monitors().into_iter().map(|m| Overlay { bounds: m.bounds, preview: preview.clone() }).collect();
    let n = overlays.len();
    let t = Instant::now();
    let shown = fade::play(overlays, None, Arc::new(|| true)).expect("transición");
    println!("{shown} de {n} capas visibles en {:?}", t.elapsed());
    assert_eq!(shown, n);
    assert!(fade::count_overlays() >= 1, "las capas existen mientras dura la transición");
    // Dura: apertura + margen + desvanecimiento; después no queda ninguna capa.
    let deadline = Instant::now() + Duration::from_secs(8);
    while fade::count_overlays() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("todo en {:?}", t.elapsed());
    assert_eq!(fade::count_overlays(), 0, "no queda ninguna capa");
}

/// Las miniaturas de DWM de una ventana OCULTA (`cloak`) siguen dando su tamaño: de eso depende la animación de
/// cambio de workspace (las ventanas viejas se ocultan y aun así se ven sus miniaturas al subir).
#[test]
#[ignore = "crea y destruye dos ventanas propias en pantalla un instante"]
fn la_miniatura_de_una_ventana_oculta_con_cloak_sigue_funcionando() {
    use windows::core::w;
    use windows::core::BOOL;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, ShowWindow, SW_SHOWNA, WS_EX_TOOLWINDOW, WS_POPUP};
    use tesela_lib::infrastructure::windows::slide::thumbnail_size;

    unsafe {
        // «STATIC» es una clase del sistema: una ventana simple de 320x200.
        let make = |x: i32| {
            let h = CreateWindowExW(WS_EX_TOOLWINDOW, w!("STATIC"), w!("prueba"), WS_POPUP, x, 40, 320, 200, None, None, None, None).expect("ventana");
            let _ = ShowWindow(h, SW_SHOWNA);
            h
        };
        let (source, dest) = (make(100), make(500));
        std::thread::sleep(Duration::from_millis(150));
        let visible = thumbnail_size(dest, source);
        let value = BOOL(1);
        DwmSetWindowAttribute(source, DWMWA_CLOAK, &value as *const _ as *const _, size_of::<BOOL>() as u32).expect("cloak");
        std::thread::sleep(Duration::from_millis(150));
        let cloaked = thumbnail_size(dest, source);
        let _ = DestroyWindow(source);
        let _ = DestroyWindow(dest);
        println!("miniatura visible: {visible:?}; oculta con cloak: {cloaked:?}");
        assert!(visible.is_some_and(|(w, h)| w > 0 && h > 0));
        assert!(cloaked.is_some_and(|(w, h)| w > 0 && h > 0), "la miniatura de una ventana oculta debe tener tamaño");
    }
}

/// Transición de fondos con el fondo REAL: se copia el fondo actual, se pone congelado a pantalla completa (sin tapar las
/// ventanas) y pasados unos segundos se abre y desaparece. Se lee UN píxel de un punto del escritorio libre de ventanas.
#[test]
#[ignore = "dibuja unos segundos una copia congelada del fondo actual detrás de tus ventanas (no cambia tu fondo)"]
fn la_transicion_de_fondo_real_congela_el_fondo_viejo_y_luego_lo_abre() {
    use std::sync::Arc;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
    use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, WindowFromPoint, GA_ROOT};
    use tesela_lib::infrastructure::windows::facts::list_monitors;
    use tesela_lib::infrastructure::windows::fade::{self, Overlay};

    dpi_aware();
    let Some((host, _)) = fade::find_host_and_engine() else {
        println!("sin anfitrión del fondo: se omite");
        return;
    };
    let monitors = list_monitors();
    let sample_at = |x: i32, y: i32| unsafe {
        let dc = GetDC(None);
        let c = GetPixel(dc, x, y).0;
        ReleaseDC(None, dc);
        (c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff)
    };

    // Cuánto tarda la copia del fondo.
    let t = Instant::now();
    let (origin, cap) = fade::capture_host(host).expect("copiar el fondo");
    println!("copia del fondo {}x{} en {:?}", cap.w, cap.h, t.elapsed());

    let overlays: Vec<Overlay> = monitors.iter().map(|m| Overlay { bounds: m.bounds, preview: String::new() }).collect();
    let start = Instant::now();
    let shown = fade::play(overlays, None, Arc::new(move || start.elapsed() > Duration::from_millis(500))).expect("transición");
    println!("{shown} capas puestas en {:?}", start.elapsed());
    assert_eq!(shown, monitors.len());
    assert!(fade::count_overlays() >= 1);
    std::thread::sleep(Duration::from_millis(300));

    // Puntos donde la capa es lo que se ve (ninguna ventana encima). La capa deja pasar los clics, así que
    // `WindowFromPoint` la salta y devuelve el escritorio.
    // Se recorre cada monitor con un paso de 8 px (los huecos entre ventanas del mosaico son estrechos) y se guardan hasta 60.
    let mut points = Vec::new();
    let mut classes: std::collections::BTreeMap<String, usize> = Default::default();
    'scan: for m in &monitors {
        let mut y = m.bounds.y + 4;
        while y < m.bounds.y + m.bounds.h {
            let mut x = m.bounds.x + 4;
            while x < m.bounds.x + m.bounds.w {
                let hit = unsafe { WindowFromPoint(POINT { x, y }) };
                let root_class = if hit.0.is_null() { "(nada)".to_string() } else { fade::class_name(unsafe { GetAncestor(hit, GA_ROOT) }) };
                *classes.entry(root_class.clone()).or_default() += 1;
                if matches!(root_class.as_str(), "Progman" | "WorkerW" | "WMWallpaperFade") {
                    points.push((x, y));
                    if points.len() >= 60 {
                        break 'scan;
                    }
                }
                x += 8;
            }
            y += 8;
        }
    }
    println!("clases en cada punto: {classes:?}");
    println!("{} puntos donde se ve la capa", points.len());
    if points.is_empty() {
        println!("no hay ningún punto del escritorio libre de ventanas: no se puede comprobar el píxel");
    }
    let first: Vec<_> = points.iter().map(|&(x, y)| sample_at(x, y)).collect();
    std::thread::sleep(Duration::from_millis(400));
    let second: Vec<_> = points.iter().map(|&(x, y)| sample_at(x, y)).collect();
    assert_eq!(first, second, "la copia está congelada: no cambia con el tiempo");
    // Y se parece al fondo copiado (la copia se hizo unos milisegundos antes).
    let close = |a: (u32, u32, u32), b: (u32, u32, u32)| a.0.abs_diff(b.0) < 60 && a.1.abs_diff(b.1) < 60 && a.2.abs_diff(b.2) < 60;
    let matching = points
        .iter()
        .zip(&first)
        .filter(|((x, y), p)| {
            let i = (((y - origin.1) * cap.w + (x - origin.0)) * 4) as usize;
            close(**p, (cap.pixels[i + 2] as u32, cap.pixels[i + 1] as u32, cap.pixels[i] as u32))
        })
        .count();
    println!("{matching} de {} puntos coinciden con el fondo copiado", points.len());
    assert!(matching * 10 >= points.len() * 9, "la capa muestra el fondo copiado");

    let deadline = Instant::now() + Duration::from_secs(8);
    while fade::count_overlays() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(fade::count_overlays(), 0, "al terminar no queda ninguna capa");
}

/// La animación de cambio de workspace de punta a punta con dos ventanas PROPIAS (una blanca que sale y una gris, oculta,
/// que entra): al terminar, en el sitio de la blanca debe verse la miniatura gris de la que entra.
#[test]
#[ignore = "dibuja un instante dos ventanas propias y la capa de la animación en el monitor principal"]
fn el_cambio_de_workspace_deja_la_ventana_que_entra_en_su_sitio() {
    use windows::core::{w, BOOL};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
    use windows::Win32::UI::WindowsAndMessaging::*;
    use tesela_lib::domain::geometry::Rect;
    use tesela_lib::infrastructure::windows::facts::list_monitors;
    use tesela_lib::infrastructure::windows::slide;

    dpi_aware();
    let area = list_monitors()[0].bounds;
    let rect = Rect::new(area.x + 300, area.y + 250, 240, 140);
    let (cx, cy) = (rect.x + rect.w / 2, rect.y + rect.h / 2);
    let sample = || unsafe {
        let dc = GetDC(None);
        let c = GetPixel(dc, cx, cy).0;
        ReleaseDC(None, dc);
        (c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff)
    };
    let make = |style: u32| unsafe {
        let h = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, w!("STATIC"), w!("prueba"), WS_POPUP | WINDOW_STYLE(style), rect.x, rect.y, rect.w, rect.h, None, None, None, None).expect("ventana");
        let _ = ShowWindow(h, SW_SHOWNA);
        h
    };
    let cloak = |h: HWND, on: bool| unsafe {
        let value = BOOL(on as i32);
        let _ = DwmSetWindowAttribute(h, DWMWA_CLOAK, &value as *const _ as *const _, size_of::<BOOL>() as u32);
    };

    let incoming = make(5); // SS_GRAYRECT
    pump_for(300);
    let gray = sample();
    cloak(incoming, true);
    let outgoing = make(6); // SS_WHITERECT
    pump_for(300);
    let white = sample();
    println!("entrante {gray:?}; saliente {white:?}");
    assert_eq!(white, (255, 255, 255));
    assert_ne!(gray, white);

    let (out_id, in_id) = (outgoing.0 as isize, incoming.0 as isize);
    let worker = std::thread::spawn(move || slide::begin(area, vec![(out_id, rect)], vec![(in_id, rect)]));
    pump_for(900); // animación (300 ms) + preparación; el hilo de la animación bombea sus propios mensajes
    let after = sample();
    println!("píxel tras la animación: {after:?}");
    let slide = worker.join().unwrap().expect("la animación debe arrancar");
    assert_eq!(after, gray, "en el sitio de la ventana que salió se ve ahora la que entró");
    slide.end();
    unsafe {
        let _ = DestroyWindow(outgoing);
        let _ = DestroyWindow(incoming);
    }
}

fn pump_for(ms: u64) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// El ejecutable de pruebas no lleva el manifiesto de la app: sin esto Windows virtualiza las coordenadas por DPI y las
/// posiciones en píxeles no coinciden con las reales.
fn dpi_aware() {
    use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
