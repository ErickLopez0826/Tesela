//! Lee la batería con `GetSystemPowerStatus`. Solo consulta: no cambia nada del sistema.

use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use crate::domain::battery::{from_power_status, BatteryState};

/// Estado de la batería, o `None` si el equipo no tiene (un escritorio) o Windows no responde.
pub fn read() -> Option<BatteryState> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    from_power_status(status.ACLineStatus, status.BatteryFlag, status.BatteryLifePercent, status.BatteryLifeTime, status.SystemStatusFlag == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No hay forma de saber qué equipo ejecuta la prueba: solo se comprueba que la lectura es coherente.
    #[test]
    fn la_lectura_real_es_coherente() {
        match read() {
            Some(b) => {
                println!("Batería: {b:?}");
                assert!(b.percent <= 100);
            }
            None => println!("Este equipo no tiene batería."),
        }
    }
}
