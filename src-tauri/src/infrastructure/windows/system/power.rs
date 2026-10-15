//! Energía (§9.2): apagar, suspender, reiniciar, hibernar.
//!
//! IMPORTANTE: este adaptador SÍ apaga/reinicia el equipo. Solo debe usarse envuelto en `Guarded`
//! con `live = true`, es decir, si la configuración tiene `[system] allow_changes = true`.
//! Las pruebas no lo ejecutan: solo comprueban a qué comando corresponde cada acción.

use std::os::windows::process::CommandExt;

use windows::Win32::System::Power::SetSuspendState;

use crate::application::system::PowerControl;
use crate::domain::system::{ActionOutcome, PowerAction};

/// `CREATE_NO_WINDOW`
const NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerCommand {
    /// Programa y argumentos.
    Exe(&'static str, &'static [&'static str]),
    /// Suspensión mediante `SetSuspendState`.
    Suspend,
}

pub fn command_for(action: PowerAction) -> PowerCommand {
    match action {
        PowerAction::Shutdown => PowerCommand::Exe("shutdown", &["/s", "/t", "0"]),
        PowerAction::Restart => PowerCommand::Exe("shutdown", &["/r", "/t", "0"]),
        PowerAction::Hibernate => PowerCommand::Exe("shutdown", &["/h"]),
        PowerAction::Suspend => PowerCommand::Suspend,
    }
}

pub struct LivePower;

impl PowerControl for LivePower {
    fn run(&mut self, action: PowerAction) -> Result<ActionOutcome, String> {
        match command_for(action) {
            PowerCommand::Exe(program, args) => {
                std::process::Command::new(program)
                    .args(args)
                    .creation_flags(NO_WINDOW)
                    .spawn()
                    .map_err(|e| format!("no se pudo ejecutar {program}: {e}"))?;
            }
            PowerCommand::Suspend => unsafe {
                // bHibernate = false: suspensión normal.
                if !SetSuspendState(false, false, false) {
                    return Err("Windows rechazó la suspensión".into());
                }
            },
        }
        Ok(ActionOutcome::done(format!("Pedido: {}", action.label())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_accion_corresponde_a_su_comando() {
        assert_eq!(command_for(PowerAction::Shutdown), PowerCommand::Exe("shutdown", &["/s", "/t", "0"]));
        assert_eq!(command_for(PowerAction::Restart), PowerCommand::Exe("shutdown", &["/r", "/t", "0"]));
        assert_eq!(command_for(PowerAction::Hibernate), PowerCommand::Exe("shutdown", &["/h"]));
        assert_eq!(command_for(PowerAction::Suspend), PowerCommand::Suspend);
    }

    #[test]
    fn ningun_comando_fuerza_el_cierre_de_apps() {
        for a in [PowerAction::Shutdown, PowerAction::Restart, PowerAction::Hibernate, PowerAction::Suspend] {
            if let PowerCommand::Exe(_, args) = command_for(a) {
                assert!(!args.contains(&"/f"), "{a:?} no debe forzar el cierre de aplicaciones");
            }
        }
    }
}
