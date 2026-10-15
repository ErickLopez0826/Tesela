//! Actualizaciones automáticas (Fase 10). Solo funcionan en una compilación con `--features updater`, con la clave
//! pública y el servidor configurados (ver `docs/instalacion-y-actualizaciones.md`). Sin eso, estos comandos explican
//! por qué no están disponibles en vez de fallar en silencio.

use serde::Serialize;

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

#[cfg(feature = "updater")]
mod enabled {
    use super::UpdateInfo;
    use tauri_plugin_updater::UpdaterExt;

    pub async fn check(app: &tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
        let updater = app.updater().map_err(|e| e.to_string())?;
        let update = updater.check().await.map_err(|e| e.to_string())?;
        Ok(update.map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone() }))
    }

    pub async fn install(app: &tauri::AppHandle) -> Result<bool, String> {
        let updater = app.updater().map_err(|e| e.to_string())?;
        let Some(update) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(false) };
        update.download_and_install(|_chunk, _total| {}, || {}).await.map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[cfg(not(feature = "updater"))]
mod enabled {
    use super::UpdateInfo;

    const MSG: &str = "Esta compilación no incluye las actualizaciones automáticas todavía (falta la clave de firma y el servidor).";

    pub async fn check(_app: &tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
        Err(MSG.into())
    }

    pub async fn install(_app: &tauri::AppHandle) -> Result<bool, String> {
        Err(MSG.into())
    }
}

/// ¿Hay una versión nueva? `None` si ya tienes la última.
#[tauri::command]
pub async fn updates_check(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    enabled::check(&app).await
}

/// Descarga e instala la versión nueva y reinicia la app.
#[tauri::command]
pub async fn updates_install(app: tauri::AppHandle) -> Result<(), String> {
    if enabled::install(&app).await? {
        app.restart();
    }
    Ok(())
}
