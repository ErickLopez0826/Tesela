//! Fase 9 (preparada, backend simulado): el módulo «Pareja» instalado en la app real.
//! El canal es local con 400 ms de retardo (imita la red); el servidor de sincronización real (Supabase) no existe aún
//! (BLOCKERS.md B-10). La prueba comprueba el camino completo: módulo → API de canales → hub → evento de Tauri →
//! iframe → archivo guardado, con la «pareja virtual» como segundo interlocutor.
//!
//! Se ejecuta a mano: `cargo test --test phase9_partner -- --ignored --nocapture --test-threads=1`
#![cfg(windows)]

mod common;

use std::path::Path;

use common::e2e::*;
use window_manager_lib::domain::modules::Permission;

fn partner_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../modules/partner")
}

#[test]
#[ignore]
fn el_modulo_pareja_envia_y_recibe_mensajes_por_el_canal_simulado() {
    let _dev = DevServer::ensure();
    let base = tmp("partner");
    let s = store(&base);

    let preview = s.preview(&partner_dir()).expect("el módulo es válido");
    let perms: Vec<Permission> = preview.permissions.iter().map(|p| p.permission).collect();
    assert_eq!(perms, vec![Permission::Channels, Permission::Files, Permission::Notifications]);
    s.install(&partner_dir(), &perms).unwrap();

    let report = selftest(&base, "partner");
    let o = outcomes(&report);
    println!("pareja ({}): {o:?}", report["reason"]);
    assert_eq!(result_of(&o, "channels.info", 0), "ok");
    assert_eq!(result_of(&o, "channels.subscribe", 0), "ok");
    assert!(calls(&o, "channels.send") >= 2, "el saludo y la respuesta de la pareja virtual: {o:?}");
    assert!(o.iter().all(|(_, res)| res == "ok"), "{o:?}");
    assert!(calls(&o, "notify.show") >= 1, "avisó del mensaje recibido");

    // El mensaje enviado y la respuesta que volvió por el canal quedaron guardados.
    let saved = std::fs::read_to_string(base.join("module-data/partner/mensajes.json")).expect("historial guardado");
    println!("historial: {saved}");
    let list: serde_json::Value = serde_json::from_str(&saved).unwrap();
    let texts: Vec<&str> = list.as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
    assert_eq!(texts.first(), Some(&"👋"));
    assert!(texts.iter().any(|t| t.starts_with("(simulación) Recibí: 👋")), "la respuesta llegó por el canal: {texts:?}");
    let froms: Vec<&str> = list.as_array().unwrap().iter().map(|m| m["from"].as_str().unwrap()).collect();
    assert_eq!(froms.first(), Some(&"me"));
    assert!(froms.contains(&"partner"));

    // Desinstalar no deja nada.
    s.uninstall("partner").unwrap();
    assert!(!base.join("modules/partner").exists() && !base.join("module-data/partner").exists());
}

#[test]
#[ignore]
fn sin_el_permiso_de_canales_el_modulo_no_puede_enviar_ni_recibir() {
    let _dev = DevServer::ensure();
    let base = tmp("partner-denied");
    store(&base).install(&partner_dir(), &[Permission::Files]).unwrap(); // sin channels ni notifications
    let report = selftest(&base, "partner");
    let o = outcomes(&report);
    println!("pareja sin canales ({}): {o:?}", report["reason"]);
    assert_eq!(result_of(&o, "channels.info", 0), "permission_denied");
    assert!(!o.iter().any(|(m, res)| m.starts_with("channels.") && res == "ok"));
    assert!(!base.join("module-data/partner/mensajes.json").exists(), "no hubo mensajes que guardar");
}
