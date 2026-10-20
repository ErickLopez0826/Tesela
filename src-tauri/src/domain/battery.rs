//! Batería del equipo. Aquí se interpreta lo que informa Windows (`GetSystemPowerStatus`); leerlo es cosa de la infraestructura.
//!
//! Un equipo de escritorio no tiene batería: Windows lo avisa con el indicador 128 y entonces no hay nada que mostrar.

use serde::Serialize;

/// Indicador de «el equipo no tiene batería».
const FLAG_NO_BATTERY: u8 = 128;
/// Indicador de «cargando».
const FLAG_CHARGING: u8 = 8;
/// Valor con el que Windows dice «no lo sé».
const UNKNOWN_U8: u8 = 255;
const UNKNOWN_SECONDS: u32 = u32::MAX;

/// Qué está haciendo la batería.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryMode {
    /// Enchufado y cargando.
    Charging,
    /// Enchufado y sin cargar (llena, o con la carga detenida).
    PluggedIn,
    /// Funcionando con la batería.
    OnBattery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BatteryState {
    pub percent: u8,
    pub mode: BatteryMode,
    /// Minutos que quedan con la batería, si Windows los sabe (solo mientras se descarga).
    pub minutes_left: Option<u32>,
    /// El ahorro de batería de Windows está activado.
    pub saver: bool,
}

/// Interpreta el estado de energía de Windows. `None` si el equipo no tiene batería o no se sabe su carga.
pub fn from_power_status(ac_line: u8, flag: u8, percent: u8, seconds_left: u32, saver: bool) -> Option<BatteryState> {
    if flag == FLAG_NO_BATTERY || percent == UNKNOWN_U8 || percent > 100 {
        return None;
    }
    let mode = if flag != UNKNOWN_U8 && flag & FLAG_CHARGING != 0 {
        BatteryMode::Charging
    } else if ac_line == 1 {
        BatteryMode::PluggedIn
    } else {
        BatteryMode::OnBattery
    };
    let minutes_left = (mode == BatteryMode::OnBattery && seconds_left != UNKNOWN_SECONDS).then_some(seconds_left / 60);
    Some(BatteryState { percent, mode, minutes_left, saver })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_equipo_de_escritorio_no_tiene_bateria() {
        assert_eq!(from_power_status(1, 128, 255, UNKNOWN_SECONDS, false), None);
        // Aunque algún dato venga suelto, el indicador de «sin batería» manda.
        assert_eq!(from_power_status(1, 128, 80, 600, false), None);
    }

    #[test]
    fn con_bateria_se_distingue_cargando_enchufado_y_en_bateria() {
        let charging = from_power_status(1, 8, 64, UNKNOWN_SECONDS, false).unwrap();
        assert_eq!((charging.percent, charging.mode, charging.minutes_left), (64, BatteryMode::Charging, None));
        // Cargando mientras tiene poca carga: el indicador combina «baja» (2) y «cargando» (8).
        assert_eq!(from_power_status(1, 10, 9, UNKNOWN_SECONDS, false).unwrap().mode, BatteryMode::Charging);
        // Enchufada y llena: sin cargar.
        assert_eq!(from_power_status(1, 1, 100, UNKNOWN_SECONDS, false).unwrap().mode, BatteryMode::PluggedIn);
        let on_battery = from_power_status(0, 1, 57, 7_260, false).unwrap();
        assert_eq!((on_battery.mode, on_battery.minutes_left), (BatteryMode::OnBattery, Some(121)));
    }

    #[test]
    fn el_tiempo_restante_solo_cuenta_con_la_bateria_y_si_se_sabe() {
        assert_eq!(from_power_status(0, 1, 50, UNKNOWN_SECONDS, false).unwrap().minutes_left, None);
        assert_eq!(from_power_status(1, 1, 50, 3_600, false).unwrap().minutes_left, None);
    }

    #[test]
    fn el_ahorro_de_bateria_se_conserva() {
        assert!(from_power_status(0, 1, 30, 600, true).unwrap().saver);
    }

    #[test]
    fn una_carga_desconocida_o_absurda_no_se_muestra() {
        assert_eq!(from_power_status(0, 1, 255, 600, false), None);
        assert_eq!(from_power_status(0, 1, 101, 600, false), None);
        // Estado desconocido de corriente: se toma como batería, no como cargando.
        assert_eq!(from_power_status(255, 255, 40, UNKNOWN_SECONDS, false).unwrap().mode, BatteryMode::OnBattery);
    }
}
