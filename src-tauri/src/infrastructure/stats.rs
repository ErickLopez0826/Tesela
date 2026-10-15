//! Métricas del sistema con `sysinfo` (§14).

use sysinfo::System;

use crate::application::ports::SystemStatsProvider;
use crate::domain::snapshot::SystemStats;

pub struct SysinfoStats {
    sys: System,
}

impl SysinfoStats {
    pub fn new() -> Self {
        let mut sys = System::new();
        // Primera lectura: el uso de CPU se calcula entre dos lecturas.
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        SysinfoStats { sys }
    }
}

impl Default for SysinfoStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemStatsProvider for SysinfoStats {
    fn read(&mut self) -> SystemStats {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        SystemStats {
            cpu_percent: self.sys.global_cpu_usage().clamp(0.0, 100.0),
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_valores_coherentes() {
        let mut s = SysinfoStats::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let r = s.read();
        assert!((0.0..=100.0).contains(&r.cpu_percent));
        assert!(r.ram_total > 0 && r.ram_used > 0 && r.ram_used <= r.ram_total);
    }
}
