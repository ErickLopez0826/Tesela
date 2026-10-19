//! Acciones propias de cada aplicación en segundo plano (silenciar el micrófono de Discord, pausar Wallpaper Engine…).
//!
//! Windows no ofrece una forma común de controlar otra aplicación, así que cada una se maneja con lo que ella misma admite:
//! atajos de teclado globales, argumentos de línea de comandos o enlaces propios (`steam://…`). Hay perfiles de fábrica y
//! se pueden añadir o cambiar en `app-actions.json` (carpeta de datos de Tesela). Además, toda aplicación con sonido puede
//! silenciarse y toda aplicación puede abrirse o cerrarse; eso lo añade `commands.rs`.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Qué hace una acción.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// Pulsa un atajo («ctrl+shift+m», «media_play_pause»…): para apps con atajos globales.
    Keys(String),
    /// Lanza el propio ejecutable de la aplicación con estos argumentos.
    Exec(Vec<String>),
    /// Abre un enlace propio de la aplicación (`steam://open/friends`).
    Uri(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Def {
    pub id: String,
    pub label: String,
    #[serde(flatten)]
    pub effect: Effect,
}

fn def(id: &str, label: &str, effect: Effect) -> Def {
    Def { id: id.into(), label: label.into(), effect }
}

/// Perfiles de fábrica, por nombre de ejecutable en minúsculas.
pub fn builtin() -> HashMap<String, Vec<Def>> {
    let exec = |args: &[&str]| Effect::Exec(args.iter().map(|a| a.to_string()).collect());
    let mut m = HashMap::new();
    // Discord solo admite control externo con una aplicación registrada; lo que sí admite son sus «combinaciones de
    // teclas» globales (Ajustes de usuario → Combinaciones de teclas): se pulsan las mismas que se pongan allí.
    m.insert(
        "discord.exe".to_string(),
        vec![
            def("discord-mute", "Silenciar micrófono", Effect::Keys("ctrl+shift+m".into())),
            def("discord-deafen", "Ensordecer", Effect::Keys("ctrl+shift+d".into())),
        ],
    );
    m.insert(
        "wallpaper64.exe".to_string(),
        vec![
            def("we-pause", "Pausar fondo", exec(&["-control", "pause"])),
            def("we-play", "Reanudar fondo", exec(&["-control", "play"])),
            def("we-mute", "Silenciar fondo", exec(&["-control", "mute"])),
            def("we-unmute", "Activar sonido del fondo", exec(&["-control", "unmute"])),
        ],
    );
    m.insert(
        "spotify.exe".to_string(),
        vec![
            def("spotify-toggle", "Reproducir / pausar", Effect::Keys("media_play_pause".into())),
            def("spotify-next", "Siguiente", Effect::Keys("media_next".into())),
            def("spotify-prev", "Anterior", Effect::Keys("media_prev".into())),
        ],
    );
    m.insert(
        "steam.exe".to_string(),
        vec![
            def("steam-library", "Biblioteca", Effect::Uri("steam://open/games".into())),
            def("steam-friends", "Amigos", Effect::Uri("steam://open/friends".into())),
        ],
    );
    m
}

/// Perfiles del usuario (`app-actions.json`): reemplazan por completo el de fábrica de ese ejecutable. Un archivo ausente o
/// mal formado no rompe nada: se ignora.
pub fn load_user(path: &Path) -> HashMap<String, Vec<Def>> {
    let Ok(text) = std::fs::read_to_string(path) else { return HashMap::new() };
    let Ok(raw) = serde_json::from_str::<HashMap<String, Vec<Def>>>(&text) else { return HashMap::new() };
    raw.into_iter().map(|(exe, defs)| (exe.to_lowercase(), defs)).collect()
}

/// Perfil de un ejecutable: el del usuario si lo hay, si no el de fábrica.
pub fn profile(exe: &str, user: &HashMap<String, Vec<Def>>) -> Vec<Def> {
    let exe = exe.to_lowercase();
    user.get(&exe).cloned().or_else(|| builtin().remove(&exe)).unwrap_or_default()
}

/// Un enlace propio es aceptable si tiene esquema y no apunta a archivos ni a código.
pub fn uri_allowed(uri: &str) -> bool {
    let lower = uri.to_lowercase();
    let Some((scheme, rest)) = lower.split_once("://") else { return false };
    !rest.is_empty()
        && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
        && !matches!(scheme, "file" | "javascript" | "data" | "vbscript")
}

/// Abre un enlace con su aplicación registrada (sin pasar por una consola: nada de lo que contenga se interpreta).
pub fn open_uri(uri: &str) -> Result<(), String> {
    use windows::core::{w, HSTRING};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let result = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(uri), None, None, SW_SHOWNORMAL) };
    // ShellExecute devuelve un valor mayor que 32 si tuvo éxito.
    if result.0 as isize > 32 { Ok(()) } else { Err(format!("no se pudo abrir {uri}")) }
}

/// Códigos de tecla (en orden de pulsación) de un atajo como «ctrl+shift+m».
pub fn parse_keys(text: &str) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        let code: u16 = match part.as_str() {
            "ctrl" | "control" => 0x11,
            "shift" => 0x10,
            "alt" => 0x12,
            "win" => 0x5B,
            "space" => 0x20,
            "enter" => 0x0D,
            "esc" | "escape" => 0x1B,
            "tab" => 0x09,
            "media_play_pause" => 0xB3,
            "media_next" => 0xB0,
            "media_prev" => 0xB1,
            "media_stop" => 0xB2,
            "volume_mute" => 0xAD,
            p if p.len() == 1 && p.as_bytes()[0].is_ascii_alphanumeric() => p.to_uppercase().as_bytes()[0] as u16,
            p if p.starts_with('f') && p[1..].parse::<u16>().is_ok_and(|n| (1..=24).contains(&n)) => 0x70 + p[1..].parse::<u16>().unwrap() - 1,
            _ => return None,
        };
        out.push(code);
    }
    (!out.is_empty()).then_some(out)
}

/// Pulsa el atajo: todas las teclas hacia abajo en orden y luego hacia arriba al revés.
pub fn send_chord(keys: &[u16]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY};
    let mk = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                dwExtraInfo: crate::infrastructure::windows::winkey::OWN_TAG,
                ..Default::default()
            },
        },
    };
    let mut inputs: Vec<INPUT> = keys.iter().map(|&k| mk(k, false)).collect();
    inputs.extend(keys.iter().rev().map(|&k| mk(k, true)));
    unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_atajos_se_convierten_en_codigos_de_tecla() {
        assert_eq!(parse_keys("ctrl+shift+m"), Some(vec![0x11, 0x10, 0x4D]));
        assert_eq!(parse_keys("Alt + F4"), Some(vec![0x12, 0x73]));
        assert_eq!(parse_keys("media_play_pause"), Some(vec![0xB3]));
        assert_eq!(parse_keys("ctrl+7"), Some(vec![0x11, 0x37]));
        assert_eq!(parse_keys("ctrl+raro"), None);
        assert_eq!(parse_keys(""), None);
        assert_eq!(parse_keys("f25"), None);
    }

    #[test]
    fn los_perfiles_de_fabrica_cubren_las_apps_conocidas() {
        let m = builtin();
        for exe in ["discord.exe", "wallpaper64.exe", "spotify.exe", "steam.exe"] {
            assert!(!m[exe].is_empty(), "{exe}");
        }
        // Todos los atajos de fábrica son válidos y los enlaces están permitidos.
        for defs in m.values() {
            for d in defs {
                match &d.effect {
                    Effect::Keys(k) => assert!(parse_keys(k).is_some(), "{}", d.id),
                    Effect::Uri(u) => assert!(uri_allowed(u), "{}", d.id),
                    Effect::Exec(args) => assert!(!args.is_empty()),
                }
            }
        }
        let ids: Vec<&String> = m.values().flatten().map(|d| &d.id).collect();
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len(), "ids únicos");
    }

    #[test]
    fn solo_se_abren_enlaces_con_esquema_y_nunca_archivos_ni_codigo() {
        assert!(uri_allowed("steam://open/friends"));
        assert!(uri_allowed("https://example.com"));
        for bad in ["file:///C:/x.exe", "javascript://x", "C:\\x.exe", "steam:", "steam://", "1abc://x", "data://x"] {
            assert!(!uri_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn el_perfil_del_usuario_reemplaza_al_de_fabrica_y_un_archivo_roto_se_ignora() {
        let dir = std::env::temp_dir().join(format!("tesela-actions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app-actions.json");
        std::fs::write(&file, r#"{"Discord.exe": [{"id": "x", "label": "Mi acción", "keys": "ctrl+alt+d"}]}"#).unwrap();
        let user = load_user(&file);
        let d = profile("DISCORD.exe", &user);
        assert_eq!(d, vec![Def { id: "x".into(), label: "Mi acción".into(), effect: Effect::Keys("ctrl+alt+d".into()) }]);
        assert_eq!(profile("steam.exe", &user).len(), 2, "lo demás sigue siendo de fábrica");
        std::fs::write(&file, "esto no es json").unwrap();
        assert!(load_user(&file).is_empty());
        assert!(load_user(&dir.join("no-existe.json")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
