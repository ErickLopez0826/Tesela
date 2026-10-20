//! Apps empaquetadas (MSIX) que son programas de escritorio normales: Teams, Claude, WhatsApp, Terminal… No tienen acceso
//! directo en el menú Inicio (Windows las lista por su manifiesto), así que el escáner de accesos directos no las ve.
//! Aquí se interpreta ese manifiesto; leerlo del disco es cosa de la infraestructura.
//!
//! Solo cuentan las que arrancan un `.exe` de escritorio (`Windows.FullTrustApplication`) y aparecen en la lista de
//! aplicaciones: las apps UWP puras (Calculadora, Ajustes…) quedan fuera, como se decidió para el buscador (§18).

/// Una aplicación de escritorio dentro de un paquete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagedApp {
    /// `Id` de la aplicación dentro del paquete (la parte tras `!` en su identificador).
    pub id: String,
    /// Ejecutable relativo a la carpeta del paquete (`app\Claude.exe`).
    pub executable: String,
}

/// Valor de un atributo XML dentro de una etiqueta (`Id="x"` → `x`). Distingue `Id` de `AppId`.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(found) = tag[from..].find(name) {
        let at = from + found;
        let before_ok = tag[..at].chars().next_back().is_none_or(|c| c.is_whitespace());
        let rest = tag[at + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let value = rest[1..].trim_start();
            let quote = value.chars().next().filter(|q| *q == '"' || *q == '\'')?;
            let end = value[1..].find(quote)?;
            return Some(value[1..1 + end].to_string());
        }
        from = at + name.len();
    }
    None
}

/// Las aplicaciones de escritorio que muestra la lista de aplicaciones de un `AppxManifest.xml`.
pub fn parse_manifest(xml: &str) -> Vec<PackagedApp> {
    let mut apps = Vec::new();
    let Some(start) = xml.find("<Applications") else { return apps };
    let section = &xml[start..xml[start..].find("</Applications>").map_or(xml.len(), |e| start + e)];
    // Cada `<Application …>` hasta el siguiente: su `VisualElements` está dentro de ese tramo.
    for chunk in section.split("<Application ").skip(1) {
        let Some(tag_end) = chunk.find('>') else { continue };
        let tag = &chunk[..tag_end];
        let (Some(id), Some(executable)) = (attribute(tag, "Id"), attribute(tag, "Executable")) else { continue };
        let desktop = attribute(tag, "EntryPoint").is_some_and(|e| e.eq_ignore_ascii_case("Windows.FullTrustApplication"));
        let hidden = chunk
            .find("<uap:VisualElements")
            .or_else(|| chunk.find("<VisualElements"))
            .map(|v| &chunk[v..chunk[v..].find('>').map_or(chunk.len(), |e| v + e)])
            .and_then(|visual| attribute(visual, "AppListEntry"))
            .is_some_and(|e| e.eq_ignore_ascii_case("none"));
        if desktop && !hidden && executable.to_lowercase().ends_with(".exe") {
            apps.push(PackagedApp { id, executable });
        }
    }
    apps
}

/// Dirección con la que el shell abre una app empaquetada (`shell:AppsFolder\<familia>!<id>`).
pub fn launch_path(family: &str, app_id: &str) -> String {
    format!(r"shell:AppsFolder\{family}!{app_id}")
}

/// ¿Es la dirección de una app empaquetada (y no la ruta de un archivo)?
pub fn is_launch_path(path: &str) -> bool {
    path.get(..r"shell:AppsFolder\".len()).is_some_and(|p| p.eq_ignore_ascii_case(r"shell:AppsFolder\"))
}

/// Máximo razonable de una dirección de lanzamiento (un nombre de familia y un id son cortos).
const MAX_LAUNCH_PATH_CHARS: usize = 300;

/// ¿Es un trozo válido de una dirección de lanzamiento? Solo letras, números, `.`, `-` y `_` (lo que usan Windows y la Tienda).
fn is_name_part(part: &str) -> bool {
    !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Valida una dirección `shell:AppsFolder\<familia>!<id>` antes de abrirla. Es más estricta que la de los `.exe`: nada de otras
/// carpetas del shell, rutas, `..`, espacios, comillas ni parámetros; solo el identificador de una aplicación empaquetada.
pub fn validate_launch_path(path: &str) -> Result<(), String> {
    if !is_launch_path(path) || path.len() > MAX_LAUNCH_PATH_CHARS {
        return Err("no es la dirección de una app empaquetada".into());
    }
    let id = &path[r"shell:AppsFolder\".len()..];
    match id.split_once('!') {
        Some((family, app)) if is_name_part(family) && is_name_part(app) && !family.starts_with('.') && !app.starts_with('.') => Ok(()),
        _ => Err("el identificador de la app empaquetada no es válido".into()),
    }
}

/// Nombre con el que se muestra una app: el del paquete si es la única, y con el programa añadido si el paquete trae
/// varias (así «Files» y «Calendar» no se llaman igual).
pub fn display_name(package_name: &str, executable: &str, siblings: usize) -> String {
    if siblings <= 1 {
        return package_name.to_string();
    }
    let stem = executable.rsplit(['\\', '/']).next().unwrap_or(executable);
    let stem = stem.strip_suffix(".exe").or_else(|| stem.strip_suffix(".EXE")).unwrap_or(stem);
    format!("{package_name} ({stem})")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEAMS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="x">
  <Identity Name="MSTeams" Version="1.0.0.0" />
  <Applications>
    <Application Id="MSTeams" Executable="ms-teams.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="ms-resource:AppName" Square44x44Logo="x.png" />
    </Application>
  </Applications>
</Package>"#;

    #[test]
    fn se_lee_una_app_de_escritorio_empaquetada() {
        assert_eq!(parse_manifest(TEAMS), vec![PackagedApp { id: "MSTeams".into(), executable: "ms-teams.exe".into() }]);
    }

    #[test]
    fn las_apps_uwp_puras_no_cuentan() {
        let xml = r#"<Package><Applications>
          <Application Id="App" Executable="Calc.exe" EntryPoint="Calculator.App"><uap:VisualElements DisplayName="x" /></Application>
        </Applications></Package>"#;
        assert!(parse_manifest(xml).is_empty());
        let sin_entrada = r#"<Package><Applications><Application Id="App" Executable="Calc.exe"><uap:VisualElements /></Application></Applications></Package>"#;
        assert!(parse_manifest(sin_entrada).is_empty());
    }

    #[test]
    fn las_que_no_salen_en_la_lista_de_aplicaciones_no_cuentan() {
        let xml = r#"<Package><Applications>
          <Application Id="Helper" Executable="helper.exe" EntryPoint="Windows.FullTrustApplication">
            <uap:VisualElements DisplayName="x" AppListEntry="none" />
          </Application>
        </Applications></Package>"#;
        assert!(parse_manifest(xml).is_empty());
    }

    #[test]
    fn un_paquete_con_varias_apps_las_da_todas_y_cada_visibilidad_es_suya() {
        let xml = r#"<Package><Applications>
          <Application Id="People" Executable="People\People.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="x" /></Application>
          <Application Id="Hidden" Executable="Hidden\Hidden.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements AppListEntry="none" /></Application>
          <Application Id="Files" Executable="Files\Files.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="y" /></Application>
        </Applications></Package>"#;
        let ids: Vec<_> = parse_manifest(xml).into_iter().map(|a| a.id).collect();
        assert_eq!(ids, ["People", "Files"]);
    }

    #[test]
    fn un_manifiesto_roto_no_da_nada_ni_falla() {
        assert!(parse_manifest("").is_empty());
        assert!(parse_manifest("<Package><Applications><Application Id=").is_empty());
        assert!(parse_manifest("<Package></Package>").is_empty());
    }

    #[test]
    fn el_atributo_id_no_se_confunde_con_otro_que_termina_igual() {
        assert_eq!(attribute(r#"AppId="mal" Id="bien""#, "Id").as_deref(), Some("bien"));
        assert_eq!(attribute("Id = 'comillas simples'", "Id").as_deref(), Some("comillas simples"));
        assert_eq!(attribute("Otro=\"1\"", "Id"), None);
    }

    #[test]
    fn la_direccion_de_lanzamiento_se_reconoce() {
        let path = launch_path("MSTeams_8wekyb3d8bbwe", "MSTeams");
        assert_eq!(path, r"shell:AppsFolder\MSTeams_8wekyb3d8bbwe!MSTeams");
        assert!(is_launch_path(&path));
        assert!(is_launch_path(r"SHELL:appsfolder\x!y"));
        assert!(!is_launch_path(r"C:\Programas\app.exe"));
        assert!(!is_launch_path("é"));
    }

    #[test]
    fn solo_se_abren_identificadores_de_apps_empaquetadas_bien_formados() {
        for ok in [
            r"shell:AppsFolder\MSTeams_8wekyb3d8bbwe!MSTeams",
            r"shell:AppsFolder\Claude_pzs8sxrjxfjjc!Claude",
            r"shell:AppsFolder\Microsoft.GamingApp_8wekyb3d8bbwe!Microsoft.Xbox.App",
            r"SHELL:appsfolder\AppUp.IntelArcSoftware_8j3eq9eme6ctt!INTELGRAPHICSSOFTWARE",
        ] {
            assert!(validate_launch_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            r"shell:AppsFolder\",
            r"shell:AppsFolder\sin-id",
            r"shell:AppsFolder\!id",
            r"shell:AppsFolder\fam!",
            r"shell:AppsFolder\a!b!c",
            r"shell:AppsFolder\..\x!y",
            r"shell:AppsFolder\a\b!c",
            r"shell:AppsFolder\a b!c",
            r#"shell:AppsFolder\a"!c"#,
            r"shell:AppsFolder\a!c --flag",
            r"shell:AppsFolder\{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\cmd.exe",
            r"shell:Desktop\x!y",
            r"C:\Windows\System32\cmd.exe",
            "https://example.com",
        ] {
            assert!(validate_launch_path(bad).is_err(), "{bad:?} debería rechazarse");
        }
        assert!(validate_launch_path(&format!(r"shell:AppsFolder\{}!x", "a".repeat(400))).is_err());
    }

    #[test]
    fn el_nombre_distingue_las_apps_de_un_mismo_paquete() {
        assert_eq!(display_name("Microsoft Teams", "ms-teams.exe", 1), "Microsoft Teams");
        assert_eq!(display_name("Microsoft 365", r"Files\Files.exe", 3), "Microsoft 365 (Files)");
    }
}
