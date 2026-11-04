//! GPU telemetry.
//!
//! NVIDIA cards are read through NVML, which is by far the richest source
//! available on Linux (per-process VRAM, PCIe throughput, encoder load and the
//! exact reason a card is throttling). AMD and Intel fall back to the `amdgpu`
//! / `i915` sysfs interfaces, which cover the headline numbers.
//!
//! NVML is loaded at *runtime*, so the binary still starts on a machine with no
//! NVIDIA driver installed.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use super::sysfs;

/// Which driver stack produced the reading. Shown in the UI so the user knows
/// why some fields are unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuSource {
    /// NVIDIA Management Library.
    Nvml,
    /// Kernel `amdgpu` driver via sysfs.
    AmdgpuSysfs,
    /// Intel integrated graphics via sysfs.
    IntelSysfs,
    #[default]
    Unknown,
}

impl GpuSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Nvml => "NVIDIA NVML",
            Self::AmdgpuSysfs => "AMD kernel driver",
            Self::IntelSysfs => "Intel kernel driver",
            Self::Unknown => "Generic",
        }
    }
}

/// A process currently holding video memory.
#[derive(Debug, Clone)]
pub struct GpuProcess {
    pub pid: u32,
    pub name: String,
    pub vram_bytes: Option<u64>,
    /// Percentage of the GPU's shader time this process used, if known.
    pub sm_percent: Option<u32>,
}

/// A single GPU clock domain with its current and maximum speed.
#[derive(Debug, Clone)]
pub struct ClockDomain {
    /// Human name, e.g. "Graphics core".
    pub label: String,
    pub current_hz: Option<f64>,
    pub max_hz: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct GpuMetrics {
    pub index: usize,
    pub name: String,
    pub vendor: String,
    pub source: GpuSource,
    pub driver_version: Option<String>,
    pub vbios_version: Option<String>,
    pub architecture: Option<String>,
    pub pci_bus_id: Option<String>,

    /// How much of the time the GPU had *any* work running, as a percentage.
    /// This is the number overlays call "GPU usage".
    pub utilization_percent: Option<f64>,
    /// How much of the time the memory bus was being read or written.
    pub memory_bus_percent: Option<f64>,
    pub encoder_percent: Option<f64>,
    pub decoder_percent: Option<f64>,

    pub vram_total: Option<u64>,
    pub vram_used: Option<u64>,
    pub vram_free: Option<u64>,
    /// BAR1 is the window the CPU uses to reach video memory directly.
    pub bar1_total: Option<u64>,
    pub bar1_used: Option<u64>,

    pub temperature_c: Option<f64>,
    /// The hottest point on the die, usually several degrees above the average.
    pub hotspot_c: Option<f64>,
    /// Temperature of the video memory itself.
    pub memory_temp_c: Option<f64>,
    /// Temperature at which the driver starts slowing the card down.
    pub slowdown_temp_c: Option<f64>,

    pub fan_percent: Option<f64>,
    pub fan_rpm: Option<f64>,

    pub power_w: Option<f64>,
    /// The board power limit currently being enforced.
    pub power_limit_w: Option<f64>,
    pub power_limit_max_w: Option<f64>,

    pub clocks: Vec<ClockDomain>,

    pub pcie_generation: Option<u32>,
    pub pcie_max_generation: Option<u32>,
    pub pcie_width: Option<u32>,
    pub pcie_max_width: Option<u32>,
    /// Bytes per second across the PCIe bus, host to card and back.
    pub pcie_tx_bytes_per_sec: Option<f64>,
    pub pcie_rx_bytes_per_sec: Option<f64>,

    /// Plain-English explanations of why the card is not running flat out.
    pub throttle_reasons: Vec<String>,
    /// Driver performance state, translated from `P0`..`P12`.
    pub performance_state: Option<String>,

    pub processes: Vec<GpuProcess>,
}

impl GpuMetrics {
    pub fn vram_used_percent(&self) -> Option<f64> {
        let (used, total) = (self.vram_used?, self.vram_total?);
        (total > 0).then(|| (used as f64 / total as f64) * 100.0)
    }

    pub fn power_percent(&self) -> Option<f64> {
        let (now, limit) = (self.power_w?, self.power_limit_w?);
        (limit > 0.0).then(|| (now / limit) * 100.0)
    }

    /// True when the card is running unconstrained.
    pub fn is_unconstrained(&self) -> bool {
        self.throttle_reasons.is_empty()
    }
}

/// Owns the NVML handle (if any) and the discovered sysfs GPU directories.
#[derive(Debug)]
pub struct GpuSampler {
    #[cfg(target_os = "linux")]
    nvml: Option<Arc<nvml_wrapper::Nvml>>,
    sysfs_cards: Vec<PathBuf>,
    /// Values that never change, read once per device on first sight.
    statics: HashMap<usize, StaticGpuInfo>,
    /// Background reader for PCIe throughput.
    #[cfg(target_os = "linux")]
    pcie: Option<PcieMonitor>,
    /// Reported once at startup so the UI can explain a missing GPU tab.
    pub init_error: Option<String>,
}

/// Facts about a card that cannot change while the machine is running.
#[derive(Debug, Clone, Default)]
struct StaticGpuInfo {
    name: String,
    vbios_version: Option<String>,
    architecture: Option<String>,
    pci_bus_id: Option<String>,
    max_clocks: Vec<Option<f64>>,
    slowdown_temp_c: Option<f64>,
    power_limit_max_w: Option<f64>,
    pcie_max_generation: Option<u32>,
    pcie_max_width: Option<u32>,
}

impl GpuSampler {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        let (nvml, init_error) = match nvml_wrapper::Nvml::init() {
            Ok(nvml) => (Some(Arc::new(nvml)), None),
            Err(error) => (None, Some(humanise_nvml_error(&error))),
        };

        #[cfg(target_os = "linux")]
        let pcie = nvml.as_ref().map(|nvml| PcieMonitor::spawn(Arc::clone(nvml)));

        Self {
            #[cfg(target_os = "linux")]
            nvml,
            sysfs_cards: discover_sysfs_cards(),
            statics: HashMap::new(),
            #[cfg(target_os = "linux")]
            pcie,
            #[cfg(target_os = "linux")]
            init_error,
            #[cfg(not(target_os = "linux"))]
            init_error: Some("GPU monitoring is only implemented for Linux".to_owned()),
        }
    }

    pub fn sample(&mut self, elapsed: f64) -> Vec<GpuMetrics> {
        let mut gpus = Vec::new();

        #[cfg(target_os = "linux")]
        if self.nvml.is_some() {
            gpus.extend(self.sample_nvml());
        }

        // Only fall back to sysfs for cards NVML did not already cover.
        if gpus.is_empty() {
            for (index, card) in self.sysfs_cards.clone().iter().enumerate() {
                if let Some(metrics) = sample_sysfs_card(index, card, elapsed) {
                    gpus.push(metrics);
                }
            }
        }

        gpus
    }

    /// True when we have no way of reading any GPU at all.
    pub fn has_devices(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.nvml.is_some() || !self.sysfs_cards.is_empty()
        }
        #[cfg(not(target_os = "linux"))]
        {
            !self.sysfs_cards.is_empty()
        }
    }
}

/// Reads PCIe throughput on a thread of its own.
///
/// `nvmlDeviceGetPcieThroughput` blocks for around 20 ms per direction while
/// the driver samples the bus. Doing that inline would make every reading a
/// 40 ms affair, so it happens out of band and the main sampler simply picks
/// up whatever the latest value is.
#[cfg(target_os = "linux")]
#[derive(Debug)]
struct PcieMonitor {
    latest: Arc<parking_lot::Mutex<PcieReadings>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

/// Latest bytes-per-second reading for each device, as `(to card, from card)`.
#[cfg(target_os = "linux")]
type PcieReadings = HashMap<usize, (Option<f64>, Option<f64>)>;

#[cfg(target_os = "linux")]
impl PcieMonitor {
    /// How often to take a fresh PCIe reading.
    const INTERVAL: Duration = Duration::from_millis(1_000);

    fn spawn(nvml: Arc<nvml_wrapper::Nvml>) -> Self {
        let latest = Arc::new(parking_lot::Mutex::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));

        let worker_latest = Arc::clone(&latest);
        let worker_shutdown = Arc::clone(&shutdown);
        let handle = std::thread::Builder::new()
            .name("perfview-pcie".to_owned())
            .spawn(move || {
                while !worker_shutdown.load(Ordering::Relaxed) {
                    let count = nvml.device_count().unwrap_or(0);
                    for index in 0..count {
                        if worker_shutdown.load(Ordering::Relaxed) {
                            return;
                        }
                        let Ok(device) = nvml.device_by_index(index) else { continue };
                        let reading = read_pcie_throughput(&device);
                        worker_latest.lock().insert(index as usize, reading);
                    }
                    std::thread::sleep(Self::INTERVAL);
                }
            })
            .ok();

        Self { latest, shutdown, handle }
    }

    fn get(&self, index: usize) -> (Option<f64>, Option<f64>) {
        self.latest.lock().get(&index).copied().unwrap_or((None, None))
    }
}

#[cfg(target_os = "linux")]
impl Drop for PcieMonitor {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Default for GpuSampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
fn humanise_nvml_error(error: &nvml_wrapper::error::NvmlError) -> String {
    use nvml_wrapper::error::NvmlError;
    match error {
        NvmlError::LibloadingError(_) | NvmlError::DriverNotLoaded => {
            "No NVIDIA driver detected - falling back to the kernel's generic GPU interface."
                .to_owned()
        }
        NvmlError::NoPermission => {
            "The NVIDIA driver refused access. Try running perfview as the desktop user that owns the display."
                .to_owned()
        }
        other => format!("NVIDIA telemetry unavailable: {other}"),
    }
}

#[cfg(target_os = "linux")]
impl GpuSampler {
    #[allow(clippy::too_many_lines)]
    fn sample_nvml(&mut self) -> Vec<GpuMetrics> {
        use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};

        let Some(nvml) = &self.nvml else { return Vec::new() };
        let driver_version = nvml.sys_driver_version().ok();
        let count = nvml.device_count().unwrap_or(0);

        let mut gpus = Vec::new();

        for index in 0..count {
            let Ok(device) = nvml.device_by_index(index) else { continue };
            let key = index as usize;

            let info =
                self.statics.entry(key).or_insert_with(|| read_static_gpu_info(&device)).clone();
            let (pcie_tx, pcie_rx) =
                self.pcie.as_ref().map_or((None, None), |monitor| monitor.get(key));

            let utilization = device.utilization_rates().ok();
            let memory = device.memory_info().ok();
            let bar1 = device.bar1_memory_info().ok();

            let clocks = [
                (Clock::Graphics, "Graphics core"),
                (Clock::SM, "Shader cores"),
                (Clock::Memory, "Video memory"),
                (Clock::Video, "Video engine"),
            ]
            .into_iter()
            .enumerate()
            .map(|(position, (domain, label))| ClockDomain {
                label: label.to_owned(),
                current_hz: device.clock_info(domain).ok().map(|mhz| f64::from(mhz) * 1e6),
                max_hz: info.max_clocks.get(position).copied().flatten(),
            })
            .collect();

            gpus.push(GpuMetrics {
                index: key,
                name: info.name.clone(),
                vendor: "NVIDIA".to_owned(),
                source: GpuSource::Nvml,
                driver_version: driver_version.clone(),
                vbios_version: info.vbios_version.clone(),
                architecture: info.architecture.clone(),
                pci_bus_id: info.pci_bus_id.clone(),

                utilization_percent: utilization.as_ref().map(|u| f64::from(u.gpu)),
                memory_bus_percent: utilization.as_ref().map(|u| f64::from(u.memory)),
                encoder_percent: device
                    .encoder_utilization()
                    .ok()
                    .map(|u| f64::from(u.utilization)),
                decoder_percent: device
                    .decoder_utilization()
                    .ok()
                    .map(|u| f64::from(u.utilization)),

                vram_total: memory.as_ref().map(|m| m.total),
                vram_used: memory.as_ref().map(|m| m.used),
                vram_free: memory.as_ref().map(|m| m.free),
                bar1_total: bar1.as_ref().map(|m| m.total),
                bar1_used: bar1.as_ref().map(|m| m.used),

                temperature_c: device.temperature(TemperatureSensor::Gpu).ok().map(f64::from),
                hotspot_c: None,
                memory_temp_c: None,
                slowdown_temp_c: info.slowdown_temp_c,

                fan_percent: device.fan_speed(0).ok().map(f64::from),
                fan_rpm: None,

                power_w: device.power_usage().ok().map(|mw| f64::from(mw) / 1000.0),
                power_limit_w: device.enforced_power_limit().ok().map(|mw| f64::from(mw) / 1000.0),
                power_limit_max_w: info.power_limit_max_w,

                clocks,

                pcie_generation: device.current_pcie_link_gen().ok(),
                pcie_max_generation: info.pcie_max_generation,
                pcie_width: device.current_pcie_link_width().ok(),
                pcie_max_width: info.pcie_max_width,
                pcie_tx_bytes_per_sec: pcie_tx,
                pcie_rx_bytes_per_sec: pcie_rx,

                throttle_reasons: device
                    .current_throttle_reasons()
                    .map(describe_throttle_reasons)
                    .unwrap_or_default(),
                performance_state: device
                    .performance_state()
                    .ok()
                    .map(|state| describe_perf_state(&format!("{state:?}"))),

                processes: nvml_processes(&device),
            });
        }

        gpus
    }
}

/// Read everything about a card that cannot change while the machine runs.
#[cfg(target_os = "linux")]
fn read_static_gpu_info(device: &nvml_wrapper::Device<'_>) -> StaticGpuInfo {
    use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureThreshold};

    StaticGpuInfo {
        name: device.name().unwrap_or_else(|_| "NVIDIA GPU".to_owned()),
        vbios_version: device.vbios_version().ok(),
        architecture: device.architecture().ok().map(|a| format!("{a:?}")),
        pci_bus_id: device.pci_info().ok().map(|p| p.bus_id),
        max_clocks: [Clock::Graphics, Clock::SM, Clock::Memory, Clock::Video]
            .into_iter()
            .map(|domain| device.max_clock_info(domain).ok().map(|mhz| f64::from(mhz) * 1e6))
            .collect(),
        slowdown_temp_c: device
            .temperature_threshold(TemperatureThreshold::Slowdown)
            .ok()
            .map(f64::from),
        power_limit_max_w: device
            .power_management_limit_constraints()
            .ok()
            .map(|c| f64::from(c.max_limit) / 1000.0),
        pcie_max_generation: device.max_pcie_link_gen().ok(),
        pcie_max_width: device.max_pcie_link_width().ok(),
    }
}

/// NVML reports PCIe throughput in kibibytes per second.
#[cfg(target_os = "linux")]
fn read_pcie_throughput(device: &nvml_wrapper::Device<'_>) -> (Option<f64>, Option<f64>) {
    use nvml_wrapper::enum_wrappers::device::PcieUtilCounter;

    let read = |counter| device.pcie_throughput(counter).ok().map(|kb| f64::from(kb) * 1024.0);
    (read(PcieUtilCounter::Send), read(PcieUtilCounter::Receive))
}

#[cfg(target_os = "linux")]
fn nvml_processes(device: &nvml_wrapper::Device<'_>) -> Vec<GpuProcess> {
    use nvml_wrapper::enums::device::UsedGpuMemory;

    let mut processes: Vec<GpuProcess> = device
        .running_graphics_processes()
        .or_else(|_| device.running_compute_processes())
        .unwrap_or_default()
        .into_iter()
        .map(|info| GpuProcess {
            pid: info.pid,
            name: process_name(info.pid),
            vram_bytes: match info.used_gpu_memory {
                UsedGpuMemory::Used(bytes) => Some(bytes),
                UsedGpuMemory::Unavailable => None,
            },
            sm_percent: None,
        })
        .collect();

    processes.sort_by_key(|p| std::cmp::Reverse(p.vram_bytes.unwrap_or(0)));
    processes
}

fn process_name(pid: u32) -> String {
    sysfs::read_string(format!("/proc/{pid}/comm"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("PID {pid}"))
}

/// Turn NVML's throttle bitmask into sentences a gamer can act on.
#[cfg(target_os = "linux")]
fn describe_throttle_reasons(
    reasons: nvml_wrapper::bitmasks::device::ThrottleReasons,
) -> Vec<String> {
    use nvml_wrapper::bitmasks::device::ThrottleReasons as R;

    let mut out = Vec::new();
    let mut push = |flag: R, text: &str| {
        if reasons.contains(flag) {
            out.push(text.to_owned());
        }
    };

    push(R::SW_POWER_CAP, "Held back by the card's power limit");
    push(R::HW_SLOWDOWN, "Hardware slowdown active - check temperature and power delivery");
    push(R::HW_THERMAL_SLOWDOWN, "Too hot - the hardware is cutting clocks to cool down");
    push(R::HW_POWER_BRAKE_SLOWDOWN, "Power supply asked the card to back off");
    push(R::SW_THERMAL_SLOWDOWN, "Driver is lowering clocks to keep temperature in check");
    push(R::SYNC_BOOST, "Clocks locked to match another GPU in a sync-boost group");
    push(R::DISPLAY_CLOCK_SETTING, "Clocks limited by the current display mode");
    push(R::APPLICATIONS_CLOCKS_SETTING, "Clocks pinned by an application clock setting");
    out
}

/// `P0` is full speed and `P12` is deepest idle, which is not obvious.
///
/// NVML's Rust binding names the states `Zero`..`Fifteen`, so the debug string
/// has to be mapped back to the number everyone recognises.
fn describe_perf_state(raw: &str) -> String {
    const WORDS: [&str; 16] = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve", "Thirteen", "Fourteen", "Fifteen",
    ];

    let level = WORDS
        .iter()
        .position(|word| *word == raw)
        .map(|index| index as u8)
        .or_else(|| raw.trim_start_matches(['P', 'p']).parse().ok());

    match level {
        Some(0) => "P0 - Maximum performance".to_owned(),
        Some(level @ 1..=2) => format!("P{level} - High performance"),
        Some(level @ 3..=7) => format!("P{level} - Balanced"),
        Some(level @ 8..=11) => format!("P{level} - Power saving"),
        Some(level) => format!("P{level} - Idle"),
        None => "Unknown".to_owned(),
    }
}

/// Find `/sys/class/drm/cardN/device` directories that look like real GPUs.
fn discover_sysfs_cards() -> Vec<PathBuf> {
    sysfs::list_dir("/sys/class/drm")
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            // `card1` yes, `card1-DP-1` (a connector) no.
            name.starts_with("card") && name[4..].chars().all(|c| c.is_ascii_digit())
        })
        .map(|path| path.join("device"))
        .filter(|device| device.join("vendor").exists())
        .collect()
}

fn sample_sysfs_card(index: usize, device: &Path, _elapsed: f64) -> Option<GpuMetrics> {
    let vendor_id = sysfs::read_string(device.join("vendor"))?;
    let (vendor, source) = match vendor_id.as_str() {
        "0x1002" => ("AMD", GpuSource::AmdgpuSysfs),
        "0x8086" => ("Intel", GpuSource::IntelSysfs),
        "0x10de" => ("NVIDIA", GpuSource::Unknown),
        _ => ("Unknown", GpuSource::Unknown),
    };

    let hwmon = sysfs::hwmon_for_device(device);
    let hwmon_value = |file: &str| hwmon.as_ref().and_then(|h| sysfs::read_f64(h.join(file)));

    let vram_total = sysfs::read_u64(device.join("mem_info_vram_total"));
    let vram_used = sysfs::read_u64(device.join("mem_info_vram_used"));

    Some(GpuMetrics {
        index,
        name: sysfs::read_string(device.join("product_name"))
            .unwrap_or_else(|| format!("{vendor} graphics")),
        vendor: vendor.to_owned(),
        source,
        driver_version: sysfs::read_string(device.join("driver/module/version")),
        vbios_version: None,
        architecture: None,
        pci_bus_id: device
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned()),

        utilization_percent: sysfs::read_f64(device.join("gpu_busy_percent")),
        memory_bus_percent: sysfs::read_f64(device.join("mem_busy_percent")),
        encoder_percent: None,
        decoder_percent: None,

        vram_total,
        vram_used,
        vram_free: match (vram_total, vram_used) {
            (Some(total), Some(used)) => Some(total.saturating_sub(used)),
            _ => None,
        },
        bar1_total: None,
        bar1_used: None,

        temperature_c: hwmon_value("temp1_input").map(|m| m / 1000.0),
        hotspot_c: hwmon_value("temp2_input").map(|m| m / 1000.0),
        memory_temp_c: hwmon_value("temp3_input").map(|m| m / 1000.0),
        slowdown_temp_c: hwmon_value("temp1_crit").map(|m| m / 1000.0),

        fan_percent: hwmon_value("pwm1").map(|pwm| (pwm / 255.0) * 100.0),
        fan_rpm: hwmon_value("fan1_input"),

        power_w: hwmon_value("power1_average").map(|uw| uw / 1_000_000.0),
        power_limit_w: hwmon_value("power1_cap").map(|uw| uw / 1_000_000.0),
        power_limit_max_w: hwmon_value("power1_cap_max").map(|uw| uw / 1_000_000.0),

        clocks: vec![
            ClockDomain {
                label: "Graphics core".to_owned(),
                current_hz: hwmon_value("freq1_input"),
                max_hz: None,
            },
            ClockDomain {
                label: "Video memory".to_owned(),
                current_hz: hwmon_value("freq2_input"),
                max_hz: None,
            },
        ],

        pcie_generation: sysfs::read_string(device.join("current_link_speed"))
            .and_then(|s| pcie_gen_from_speed(&s)),
        pcie_max_generation: sysfs::read_string(device.join("max_link_speed"))
            .and_then(|s| pcie_gen_from_speed(&s)),
        pcie_width: sysfs::read_u64(device.join("current_link_width")).map(|w| w as u32),
        pcie_max_width: sysfs::read_u64(device.join("max_link_width")).map(|w| w as u32),
        pcie_tx_bytes_per_sec: None,
        pcie_rx_bytes_per_sec: None,

        throttle_reasons: Vec::new(),
        performance_state: sysfs::read_string(device.join("power_dpm_force_performance_level")),
        processes: Vec::new(),
    })
}

/// sysfs reports link speed as e.g. `16.0 GT/s PCIe`; map it back to a gen.
fn pcie_gen_from_speed(raw: &str) -> Option<u32> {
    let gts: f64 = raw.split_whitespace().next()?.parse().ok()?;
    Some(match gts {
        g if g >= 64.0 => 6,
        g if g >= 32.0 => 5,
        g if g >= 16.0 => 4,
        g if g >= 8.0 => 3,
        g if g >= 5.0 => 2,
        _ => 1,
    })
}
