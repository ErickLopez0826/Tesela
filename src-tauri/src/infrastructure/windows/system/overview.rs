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
use crate::domain::system::{DiskUsage, HardwareSpecs, ProcessUsage, SystemOverview};

/// Cuántas apps entran por cada criterio (CPU, RAM, GPU, disco): la interfaz elige por cuál ordenar y muestra las primeras.
const TOP_PER_METRIC: usize = 6;

/// Uso de la gráfica por proceso (pid → %), leído de la misma consulta que el total.
type GpuByPid = Arc<Mutex<std::collections::HashMap<u32, f32>>>;

/// De «pid_1234_luid_0x…_eng_0_engtype_3D» saca 1234.
fn pid_of(instance: &str) -> Option<u32> {
    instance.strip_prefix("pid_")?.split('_').next()?.parse().ok()
}

/// Uso de GPU como Task Manager: por tipo de motor (3D, Copy, Video...) se suman las instancias y
/// se toma el tipo más cargado. Se lee en un hilo aparte porque PDH tarda y necesita dos muestras.
fn gpu_worker(slot: Arc<Mutex<Option<f32>>>, by_pid: GpuByPid) {
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
            std::thread::sleep(std::time::Duration::from_secs(3));
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
            // Por proceso: igual que el total, se suma por tipo de motor y se toma el más cargado.
            let mut per_pid: std::collections::HashMap<u32, std::collections::HashMap<String, f64>> = std::collections::HashMap::new();
            for i in 0..count as usize {
                let item = &*items.add(i);
                let name = item.szName.to_string().unwrap_or_default();
                let engine_type = name.split("engtype_").nth(1).unwrap_or("").to_string();
                let value = item.FmtValue.Anonymous.doubleValue;
                if let Some(pid) = pid_of(&name) {
                    *per_pid.entry(pid).or_default().entry(engine_type.clone()).or_default() += value;
                }
                *per_type.entry(engine_type).or_default() += value;
            }
            *by_pid.lock().unwrap() = per_pid
                .into_iter()
                .map(|(pid, types)| (pid, types.values().cloned().fold(0.0f64, f64::max).clamp(0.0, 100.0) as f32))
                .collect();
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

/// Tarjeta gráfica «principal» según DXGI: la de más memoria dedicada que no sea el adaptador de software.
fn gpu_info() -> Option<(String, u64)> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut best: Option<(String, u64)> = None;
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(desc) = adapter.GetDesc1() else { continue };
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let name = String::from_utf16_lossy(&desc.Description).trim_end_matches('\0').trim().to_string();
            let vram = desc.DedicatedVideoMemory as u64;
            if !name.is_empty() && best.as_ref().is_none_or(|b| vram > b.1) {
                best = Some((name, vram));
            }
        }
        best
    }
}

pub struct SysinfoOverview {
    hardware: HardwareSpecs,
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    last_net: Instant,
    gpu: Arc<Mutex<Option<f32>>>,
    gpu_by_pid: GpuByPid,
    cache: Option<(Instant, SystemOverview)>,
}

impl SysinfoOverview {
    pub fn new() -> Self {
        let gpu = Arc::new(Mutex::new(None));
        let gpu_by_pid: GpuByPid = Arc::default();
        let (worker_slot, worker_pids) = (gpu.clone(), gpu_by_pid.clone());
        let _ = std::thread::Builder::new().name("wm-gpu".into()).spawn(move || gpu_worker(worker_slot, worker_pids));
        let mut sys = System::new();
        sys.refresh_cpu_all();
        let hardware = {
            let first = sys.cpus().first();
            let (gpu_name, gpu_vram) = match gpu_info() {
                Some((n, v)) => (Some(n), v),
                None => (None, 0),
            };
            HardwareSpecs {
                cpu_name: first.map(|c| c.brand().trim().to_string()).unwrap_or_default(),
                cpu_cores: System::physical_core_count().unwrap_or_else(|| sys.cpus().len()),
                cpu_threads: sys.cpus().len(),
                cpu_mhz: first.map(|c| c.frequency()).unwrap_or(0),
                gpu_name,
                gpu_vram,
            }
        };
        sys.refresh_memory();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        SysinfoOverview {
            hardware,
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_net: Instant::now(),
            gpu,
            gpu_by_pid,
            cache: None,
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

/// Una lectura reciente se reutiliza: las barras (una por monitor) y el panel preguntan por separado y cada lectura
/// recorre todos los procesos del sistema.
const CACHE_FOR: std::time::Duration = std::time::Duration::from_millis(1500);

impl OverviewProvider for SysinfoOverview {
    fn read(&mut self) -> SystemOverview {
        if let Some((at, cached)) = &self.cache {
            if at.elapsed() < CACHE_FOR {
                return cached.clone();
            }
        }
        let fresh = self.read_fresh();
        self.cache = Some((Instant::now(), fresh.clone()));
        fresh
    }
}

impl SysinfoOverview {
    fn read_fresh(&mut self) -> SystemOverview {
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
        let gpu_pids = self.gpu_by_pid.lock().unwrap().clone();
        let mut by_name: std::collections::HashMap<String, ProcessUsage> = std::collections::HashMap::new();
        for p in self.sys.processes().values().filter(|p| p.pid().as_u32() != 0) {
            let name = p.name().to_string_lossy().into_owned();
            let e = by_name.entry(name.clone()).or_insert(ProcessUsage { name, cpu_percent: 0.0, ram_bytes: 0, gpu_percent: 0.0, disk_bytes_per_sec: 0 });
            let io = p.disk_usage();
            e.cpu_percent += p.cpu_usage() / cpus;
            e.ram_bytes += p.memory();
            e.gpu_percent = (e.gpu_percent + gpu_pids.get(&p.pid().as_u32()).copied().unwrap_or(0.0)).min(100.0);
            e.disk_bytes_per_sec += ((io.read_bytes + io.written_bytes) as f64 / elapsed) as u64;
        }
        let all: Vec<ProcessUsage> = by_name.into_values().collect();
        // Las primeras de cada criterio, sin repetir: así la interfaz puede ordenar por cualquiera de ellos.
        let mut processes: Vec<ProcessUsage> = Vec::new();
        let mut take = |key: &dyn Fn(&ProcessUsage) -> f64| {
            let mut sorted: Vec<&ProcessUsage> = all.iter().collect();
            sorted.sort_by(|a, b| key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal));
            for p in sorted.into_iter().take(TOP_PER_METRIC).filter(|p| key(p) > 0.0) {
                if !processes.iter().any(|q| q.name == p.name) {
                    processes.push(p.clone());
                }
            }
        };
        take(&|p| p.cpu_percent as f64);
        take(&|p| p.ram_bytes as f64);
        take(&|p| p.gpu_percent as f64);
        take(&|p| p.disk_bytes_per_sec as f64);
        processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal).then(b.ram_bytes.cmp(&a.ram_bytes)));

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
            hardware: self.hardware.clone(),
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
    fn el_pid_se_saca_del_nombre_de_la_instancia() {
        assert_eq!(pid_of("pid_1234_luid_0x00000000_0x0000A1B2_phys_0_eng_0_engtype_3D"), Some(1234));
        assert_eq!(pid_of("luid_0x0_phys_0"), None);
        assert_eq!(pid_of("pid_x_eng"), None);
    }

    #[test]
    fn lee_un_resumen_real_y_coherente() {
        let mut o = SysinfoOverview::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL * 2);
        let r = o.read();
        println!("resumen: {r:#?}");
        assert!((0.0..=100.0).contains(&r.cpu_percent));
        assert!(!r.hardware.cpu_name.is_empty(), "nombre del procesador");
        assert!(r.hardware.cpu_threads >= r.hardware.cpu_cores && r.hardware.cpu_cores > 0);
        println!("hardware: {:?}", r.hardware);
        assert!(r.ram_total > 0 && r.ram_used > 0 && r.ram_used <= r.ram_total);
        assert!(!r.disks.is_empty(), "al menos un disco");
        assert!(r.disks.iter().all(|d| d.used <= d.total));
        assert!(r.uptime_secs > 0);
        assert!(!r.top_processes.is_empty() && r.top_processes.len() <= 4 * TOP_PER_METRIC);
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
