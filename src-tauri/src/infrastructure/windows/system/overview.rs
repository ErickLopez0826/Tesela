//! Estadísticas detalladas (§9.2): CPU, GPU, RAM, disco, red, tiempo encendido y apps que más consumen.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};
use windows::core::{w, PCWSTR};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};

use crate::application::system::OverviewProvider;
use crate::domain::system::{DiskUsage, ProcessUsage, SystemOverview};

const TOP_PROCESSES: usize = 5;

/// Uso de GPU como Task Manager: por tipo de motor (3D, Copy, Video...) se suman las instancias y
/// se toma el tipo más cargado. Se lee en un hilo aparte porque PDH tarda y necesita dos muestras.
fn gpu_worker(slot: Arc<Mutex<Option<f32>>>) {
    unsafe {
        let mut query = PDH_HQUERY(std::ptr::null_mut());
        if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
            return;
        }
        let mut counter = PDH_HCOUNTER(std::ptr::null_mut());
        // Nombre en inglés: los nombres normales de contadores están localizados.
        if PdhAddEnglishCounterW(query, w!("\\GPU Engine(*)\\Utilization Percentage"), 0, &mut counter) != 0 {
            let _ = PdhCloseQuery(query);
            return; // Sin GPU o sin contadores: el valor queda en None.
        }
        let _ = PdhCollectQueryData(query);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if PdhCollectQueryData(query) != 0 {
                continue;
            }
            let (mut size, mut count) = (0u32, 0u32);
            let status = PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
            if status != PDH_MORE_DATA || size == 0 {
                continue;
            }
            let mut buf = vec![0u8; size as usize];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
                continue;
            }
            let mut per_type: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
            for i in 0..count as usize {
                let item = &*items.add(i);
                let name = item.szName.to_string().unwrap_or_default();
                let engine_type = name.split("engtype_").nth(1).unwrap_or("").to_string();
                *per_type.entry(engine_type).or_default() += item.FmtValue.Anonymous.doubleValue;
            }
            let max = per_type.values().cloned().fold(0.0f64, f64::max).clamp(0.0, 100.0);
            *slot.lock().unwrap() = Some(max as f32);
        }
    }
}

/// Proveedor que crea el lector real la primera vez que se usa, en el hilo que lo pide.
/// `sysinfo` inicializa COM (modo multihilo) al listar sensores, y eso rompería la inicialización OLE
/// del hilo principal de la ventana si se hiciera allí.
pub struct LazyOverview(Option<SysinfoOverview>);

impl LazyOverview {
    pub fn new() -> Self {
        LazyOverview(None)
    }
}

impl Default for LazyOverview {
    fn default() -> Self {
        Self::new()
    }
}

impl OverviewProvider for LazyOverview {
    fn read(&mut self) -> SystemOverview {
        self.0.get_or_insert_with(SysinfoOverview::new).read()
    }
}

pub struct SysinfoOverview {
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    last_net: Instant,
    gpu: Arc<Mutex<Option<f32>>>,
}

impl SysinfoOverview {
    pub fn new() -> Self {
        let gpu = Arc::new(Mutex::new(None));
        let worker_slot = gpu.clone();
        let _ = std::thread::Builder::new().name("wm-gpu".into()).spawn(move || gpu_worker(worker_slot));
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        SysinfoOverview {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_net: Instant::now(),
            gpu,
        }
    }

    fn temperature(&mut self) -> Option<f32> {
        self.components.refresh(false);
        self.components
            .iter()
            .filter_map(|c| c.temperature().map(|t| (c.label().to_lowercase(), t)))
            .filter(|(_, t)| t.is_finite() && *t > 0.0 && *t < 150.0)
            .map(|(label, t)| (label.contains("cpu") || label.contains("core") || label.contains("package"), t))
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)))
            .map(|(_, t)| t)
    }
}

impl Default for SysinfoOverview {
    fn default() -> Self {
        Self::new()
    }
}

impl OverviewProvider for SysinfoOverview {
    fn read(&mut self) -> SystemOverview {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.disks.refresh(true);
        self.networks.refresh(true);

        let elapsed = self.last_net.elapsed().as_secs_f64().max(0.001);
        self.last_net = Instant::now();
        let (down, up) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |(d, u), (_, n)| (d + n.received(), u + n.transmitted()));

        // Varios procesos del mismo programa (navegadores, Overwolf...) cuentan como una app.
        let cpus = self.sys.cpus().len().max(1) as f32;
        let mut by_name: std::collections::HashMap<String, (f32, u64)> = std::collections::HashMap::new();
        for p in self.sys.processes().values().filter(|p| p.pid().as_u32() != 0) {
            let e = by_name.entry(p.name().to_string_lossy().into_owned()).or_default();
            e.0 += p.cpu_usage() / cpus;
            e.1 += p.memory();
        }
        let mut processes: Vec<ProcessUsage> = by_name
            .into_iter()
            .map(|(name, (cpu_percent, ram_bytes))| ProcessUsage { name, cpu_percent, ram_bytes })
            .collect();
        processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.ram_bytes.cmp(&a.ram_bytes))
        });
        processes.truncate(TOP_PROCESSES);

        let disks = self
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| DiskUsage {
                name: d.mount_point().to_string_lossy().trim_end_matches('\\').to_string(),
                used: d.total_space().saturating_sub(d.available_space()),
                total: d.total_space(),
            })
            .collect();

        let gpu_percent = *self.gpu.lock().unwrap();
        SystemOverview {
            cpu_percent: self.sys.global_cpu_usage().clamp(0.0, 100.0),
            gpu_percent,
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            disks,
            net_down: (down as f64 / elapsed) as u64,
            net_up: (up as f64 / elapsed) as u64,
            uptime_secs: System::uptime(),
            temperature_c: self.temperature(),
            top_processes: processes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_un_resumen_real_y_coherente() {
        let mut o = SysinfoOverview::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL * 2);
        let r = o.read();
        println!("resumen: {r:#?}");
        assert!((0.0..=100.0).contains(&r.cpu_percent));
        assert!(r.ram_total > 0 && r.ram_used > 0 && r.ram_used <= r.ram_total);
        assert!(!r.disks.is_empty(), "al menos un disco");
        assert!(r.disks.iter().all(|d| d.used <= d.total));
        assert!(r.uptime_secs > 0);
        assert!(!r.top_processes.is_empty() && r.top_processes.len() <= TOP_PROCESSES);
        assert!(r.top_processes.windows(2).all(|w| w[0].cpu_percent >= w[1].cpu_percent));
        let names: Vec<&str> = r.top_processes.iter().map(|p| p.name.as_str()).collect();
        let unique: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "cada app aparece una sola vez: {names:?}");
    }

    #[test]
    fn la_gpu_aparece_tras_unos_segundos_si_el_equipo_la_expone() {
        let mut o = SysinfoOverview::new();
        let mut gpu = None;
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            gpu = o.read().gpu_percent;
            if gpu.is_some() {
                break;
            }
        }
        println!("GPU: {gpu:?}");
        if let Some(g) = gpu {
            assert!((0.0..=100.0).contains(&g));
        }
    }
}
