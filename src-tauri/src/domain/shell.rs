//! Cuánto de la interfaz de Windows reemplaza Tesela: su barra de tareas y la tecla Windows.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellSettings {
    /// Oculta la barra de tareas de Windows y muestra la de Tesela (abajo, con las apps por workspace).
    pub replace_taskbar: bool,
    /// Tocar la tecla Windows abre el panel de Tesela en vez del menú Inicio (los atajos con Windows siguen funcionando).
    pub win_key_panel: bool,
}

impl Default for ShellSettings {
    fn default() -> Self {
        ShellSettings { replace_taskbar: true, win_key_panel: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn por_defecto_todo_esta_activado() {
        let s = ShellSettings::default();
        assert!(s.replace_taskbar && s.win_key_panel);
    }

    #[test]
    fn faltan_campos_se_completan_con_los_de_siempre() {
        let s: ShellSettings = serde_json::from_str(r#"{"replace_taskbar": false}"#).unwrap();
        assert_eq!(s, ShellSettings { replace_taskbar: false, win_key_panel: true });
        let s: ShellSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, ShellSettings::default());
    }
}
