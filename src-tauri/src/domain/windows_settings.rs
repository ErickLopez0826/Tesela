//! La Configuración de Windows (donde se cambia la barra de tareas, la privacidad, las aplicaciones…) y sus páginas.
//!
//! No es una app de escritorio con acceso directo: es una app UWP, así que el escáner del menú Inicio no la ve (§18). Se
//! ofrece al buscador como una lista fija de páginas, cada una con su dirección `ms-settings:` (la forma documentada de
//! abrirlas). La infraestructura solo las abre; aquí se decide cuáles hay y qué direcciones son válidas.

use crate::domain::search::{match_tier, Tier};

/// Una página de la Configuración.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsPage {
    pub name: &'static str,
    /// Dirección con la que se abre (`ms-settings:` solo, abre la portada).
    pub uri: &'static str,
    /// Otros nombres con los que también se busca (en inglés y como se les dice en el día a día).
    pub aliases: &'static [&'static str],
}

/// Dirección de la app de Configuración en el shell: se usa para sacar su icono (el engranaje), igual que las apps empaquetadas.
pub const ICON_ADDRESS: &str = r"shell:AppsFolder\windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel";

const fn page(name: &'static str, uri: &'static str, aliases: &'static [&'static str]) -> SettingsPage {
    SettingsPage { name, uri, aliases }
}

/// Las páginas que ofrece el buscador, con los nombres de la Configuración de Windows 11 en español.
pub const PAGES: &[SettingsPage] = &[
    page("Configuración", "ms-settings:", &["Ajustes", "Settings", "Windows", "Panel de control"]),
    page("Pantalla", "ms-settings:display", &["Display", "Resolución", "Brillo", "Monitor", "Escala"]),
    page("Sonido", "ms-settings:sound", &["Audio", "Sound", "Volumen", "Altavoces", "Micrófono"]),
    page("Notificaciones", "ms-settings:notifications", &["Notifications", "No molestar", "Avisos"]),
    page("Energía y batería", "ms-settings:powersleep", &["Power", "Battery", "Batería", "Suspensión", "Ahorro de energía"]),
    page("Almacenamiento", "ms-settings:storagesense", &["Storage", "Disco", "Espacio", "Liberar espacio"]),
    page("Multitarea", "ms-settings:multitasking", &["Multitasking", "Ventanas ajustadas", "Escritorios"]),
    page("Portapapeles", "ms-settings:clipboard", &["Clipboard", "Historial del portapapeles"]),
    page("Acerca de", "ms-settings:about", &["About", "Especificaciones", "Nombre del equipo", "Versión de Windows"]),
    page("Recuperación", "ms-settings:recovery", &["Recovery", "Restablecer este equipo", "Reset"]),
    page("Bluetooth y dispositivos", "ms-settings:bluetooth", &["Bluetooth", "Dispositivos", "Devices"]),
    page("Impresoras y escáneres", "ms-settings:printers", &["Printers", "Impresora", "Escáner"]),
    page("Mouse", "ms-settings:mousetouchpad", &["Ratón", "Touchpad", "Panel táctil", "Cursor"]),
    page("Red e Internet", "ms-settings:network", &["Network", "Internet", "Ethernet", "Conexión"]),
    page("Wi-Fi", "ms-settings:network-wifi", &["Wifi", "Redes inalámbricas", "Redes conocidas"]),
    page("VPN", "ms-settings:network-vpn", &["Red privada virtual"]),
    page("Personalización", "ms-settings:personalization", &["Personalization", "Temas", "Themes"]),
    page("Fondo", "ms-settings:personalization-background", &["Background", "Fondo de pantalla", "Wallpaper"]),
    page("Colores", "ms-settings:colors", &["Colors", "Modo oscuro", "Modo claro", "Color de énfasis"]),
    page("Pantalla de bloqueo", "ms-settings:lockscreen", &["Lock screen", "Bloqueo"]),
    page("Inicio", "ms-settings:personalization-start", &["Start", "Menú Inicio"]),
    page("Barra de tareas", "ms-settings:taskbar", &["Taskbar", "Iconos de la bandeja", "Barra"]),
    page("Fuentes", "ms-settings:fonts", &["Fonts", "Tipografías"]),
    page("Aplicaciones instaladas", "ms-settings:appsfeatures", &["Aplicaciones", "Apps", "Installed apps", "Desinstalar", "Programas"]),
    page("Aplicaciones predeterminadas", "ms-settings:defaultapps", &["Default apps", "Navegador predeterminado", "Abrir con"]),
    page("Aplicaciones de inicio", "ms-settings:startupapps", &["Startup apps", "Inicio automático", "Al iniciar sesión"]),
    page("Cuentas", "ms-settings:yourinfo", &["Accounts", "Mi información", "Usuario", "Cuenta de Microsoft"]),
    page("Opciones de inicio de sesión", "ms-settings:signinoptions", &["Sign-in options", "PIN", "Windows Hello", "Contraseña"]),
    page("Fecha y hora", "ms-settings:dateandtime", &["Date and time", "Hora", "Zona horaria", "Reloj"]),
    page("Idioma y región", "ms-settings:regionlanguage", &["Language", "Region", "Teclado", "Idioma", "Región"]),
    page("Juegos", "ms-settings:gaming-gamebar", &["Gaming", "Game Bar", "Modo de juego"]),
    page("Accesibilidad", "ms-settings:easeofaccess", &["Accessibility", "Lupa", "Narrador", "Texto grande"]),
    page("Privacidad y seguridad", "ms-settings:privacy", &["Privacy", "Privacidad", "Seguridad", "Permisos", "Cámara", "Ubicación"]),
    page("Seguridad de Windows", "ms-settings:windowsdefender", &["Windows Security", "Antivirus", "Defender", "Firewall"]),
    page("Windows Update", "ms-settings:windowsupdate", &["Actualizaciones", "Updates", "Actualizar Windows"]),
];

/// Máximo razonable de una dirección `ms-settings:` (las reales rondan los 40 caracteres).
const MAX_URI_CHARS: usize = 64;

/// ¿Es una dirección `ms-settings:` bien formada? Solo minúsculas, números, `-` y `:`: ni rutas, ni parámetros, ni otros protocolos.
pub fn validate_uri(uri: &str) -> Result<(), String> {
    let Some(rest) = uri.strip_prefix("ms-settings:") else {
        return Err("no es una página de la Configuración de Windows".into());
    };
    if uri.len() > MAX_URI_CHARS || !rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | ':')) {
        return Err("la dirección de la Configuración no es válida".into());
    }
    Ok(())
}

/// El nombre (el suyo o un alias) con el que `page` coincide con lo escrito. Solo cuentan los nombres que empiezan por lo escrito, o
/// una de sus palabras: sin coincidencia difusa, para que unas pocas letras no llenen los resultados de páginas de la Configuración.
/// Con el cuadro vacío devuelve el nombre propio (así el historial de uso las puede sacar).
pub fn matching_name(page: &SettingsPage, query: &str) -> Option<&'static str> {
    if query.trim().is_empty() {
        return Some(page.name);
    }
    std::iter::once(&page.name)
        .chain(page.aliases)
        .find(|name| matches!(match_tier(query, name), Some((Tier::Prefix | Tier::WordPrefix, _))))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todas_las_paginas_tienen_una_direccion_valida_y_no_se_repiten() {
        let mut uris = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for p in PAGES {
            assert!(validate_uri(p.uri).is_ok(), "{}: {}", p.name, p.uri);
            assert!(uris.insert(p.uri), "dirección repetida: {}", p.uri);
            assert!(names.insert(p.name), "nombre repetido: {}", p.name);
        }
        assert_eq!(PAGES[0].uri, "ms-settings:");
    }

    #[test]
    fn solo_se_abren_direcciones_de_la_configuracion() {
        for ok in ["ms-settings:", "ms-settings:taskbar", "ms-settings:network-wifi", "ms-settings:personalization-background"] {
            assert!(validate_uri(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "ms-settings",
            "MS-SETTINGS:taskbar",
            "ms-settings:taskbar?x=1",
            "ms-settings:a b",
            "ms-settings:..\\x",
            "ms-settings:a/b",
            "ms-settings:;calc",
            "ms-settings-connectabledevices:devicediscovery",
            "ms-gamingoverlay:",
            "calc.exe",
            r"C:\Windows\System32\cmd.exe",
            "https://example.com",
        ] {
            assert!(validate_uri(bad).is_err(), "{bad:?} debería rechazarse");
        }
        assert!(validate_uri(&format!("ms-settings:{}", "a".repeat(80))).is_err());
    }

    #[test]
    fn se_encuentran_por_su_nombre_o_un_alias() {
        let find = |q: &str| PAGES.iter().filter(|p| matching_name(p, q).is_some()).map(|p| p.name).collect::<Vec<_>>();
        assert!(find("configuracion").contains(&"Configuración"));
        assert!(find("ajustes").contains(&"Configuración"));
        assert!(find("settings").contains(&"Configuración"));
        assert!(find("privacidad").contains(&"Privacidad y seguridad"));
        assert!(find("barra").contains(&"Barra de tareas"));
        assert!(find("taskbar").contains(&"Barra de tareas"));
        assert!(find("aplicaciones").contains(&"Aplicaciones instaladas"));
        assert!(find("apps").contains(&"Aplicaciones instaladas"));
        assert!(find("bateria").contains(&"Energía y batería"));
    }

    #[test]
    fn el_nombre_propio_manda_sobre_los_alias_y_no_hay_coincidencia_difusa() {
        let page = PAGES.iter().find(|p| p.uri == "ms-settings:privacy").unwrap();
        assert_eq!(matching_name(page, "priv"), Some("Privacidad y seguridad"));
        assert_eq!(matching_name(page, "camara"), Some("Cámara"));
        assert_eq!(matching_name(page, ""), Some("Privacidad y seguridad"));
        assert_eq!(matching_name(page, "zzz"), None);
        // Letras sueltas en orden dentro de otra palabra no bastan.
        assert_eq!(matching_name(page, "pvc"), None);
    }
}
