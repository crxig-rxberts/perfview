//! CPU telemetry: per-core load broken down by what the core was *doing*,
//! live clock speeds, temperatures and scheduler pressure.

use std::path::Path;

use super::sysfs;

/// How a core spent its time since the previous sample, as percentages that
/// sum to 100. Keeping the breakdown (rather than one "usage" number) is what
/// lets the UI explain *why* a core is busy.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CpuTimeBreakdown {
    /// Normal user-space programs: games, browsers, compilers.
    pub user: f64,
    /// Low-priority ("nice") background work.
    pub nice: f64,
    /// The kernel itself: drivers, syscalls, networking.
    pub system: f64,
    /// Genuinely doing nothing.
    pub idle: f64,
    /// Idle, but only because it was waiting on a disk.
    pub iowait: f64,
    /// Handling hardware interrupts.
    pub irq: f64,
    /// Handling deferred (software) interrupts.
    pub softirq: f64,
    /// Time taken away by the hypervisor. Non-zero only in a VM.
    pub steal: f64,
}

impl CpuTimeBreakdown {
    /// Everything that is not idle and not disk-wait, i.e. real work.
    pub fn busy(self) -> f64 {
        (100.0 - self.idle - self.iowait).clamp(0.0, 100.0)
    }
}

/// A single logical processor (a hyper-thread, in most modern CPUs).
#[derive(Debug, Clone, Default)]
pub struct CoreMetrics {
    /// Index as the kernel sees it, e.g. `cpu0`.
    pub id: usize,
    /// Which physical core this logical CPU lives on.
    pub physical_core: Option<usize>,
    pub usage_percent: f64,
    pub breakdown: CpuTimeBreakdown,
    pub frequency_hz: Option<f64>,
    pub max_frequency_hz: Option<f64>,
}

/// A named temperature reading from somewhere on the CPU package.
#[derive(Debug, Clone)]
pub struct TemperatureSensor {
    pub label: String,
    pub celsius: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CpuMetrics {
    pub model: String,
    pub vendor: String,
    pub architecture: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    /// Threads per physical core. `2` means SMT / Hyper-Threading is on.
    pub threads_per_core: usize,

    pub usage_percent: f64,
    pub breakdown: CpuTimeBreakdown,
    pub cores: Vec<CoreMetrics>,

    /// Mean of the current per-core clocks, in hertz.
    pub frequency_hz: Option<f64>,
    /// Fastest core right now, in hertz. This is the number gamers look at.
    pub peak_frequency_hz: Option<f64>,
    /// Advertised maximum boost clock, in hertz.
    pub max_frequency_hz: Option<f64>,
    /// Advertised minimum clock, in hertz.
    pub min_frequency_hz: Option<f64>,

    /// Hottest package sensor, used for the headline temperature.
    pub temperature_c: Option<f64>,
    pub sensors: Vec<TemperatureSensor>,

    /// Package power draw in watts, from the RAPL energy counter.
    pub package_power_w: Option<f64>,

    /// Active frequency-scaling governor, e.g. `schedutil`.
    pub governor: Option<String>,
    /// Active scaling driver, e.g. `amd-pstate-epp`.
    pub scaling_driver: Option<String>,

    pub load_average: [f64; 3],
    pub context_switches_per_sec: f64,
    pub interrupts_per_sec: f64,
    pub forks_per_sec: f64,
    /// Tasks currently on a run queue, waiting for or using CPU time.
    pub procs_running: u64,
    /// Tasks blocked waiting for I/O to complete.
    pub procs_blocked: u64,

    pub process_count: usize,
    pub thread_count: usize,

    /// L1/L2/L3 cache sizes in bytes, if the kernel reports them.
    pub cache_l1d: Option<u64>,
    pub cache_l1i: Option<u64>,
    pub cache_l2: Option<u64>,
    pub cache_l3: Option<u64>,
}

/// Raw jiffy counters straight out of `/proc/stat`.
#[derive(Debug, Clone, Copy, Default)]
struct Jiffies {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl Jiffies {
    fn parse(fields: &[&str]) -> Self {
        let get = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        Self {
            user: get(0),
            nice: get(1),
            system: get(2),
            idle: get(3),
            iowait: get(4),
            irq: get(5),
            softirq: get(6),
            steal: get(7),
        }
    }

    fn total(self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    fn delta_percent(now: Self, before: Self) -> CpuTimeBreakdown {
        let total = now.total().saturating_sub(before.total());
        if total == 0 {
            return CpuTimeBreakdown::default();
        }
        let pct = |a: u64, b: u64| (a.saturating_sub(b) as f64 / total as f64) * 100.0;
        CpuTimeBreakdown {
            user: pct(now.user, before.user),
            nice: pct(now.nice, before.nice),
            system: pct(now.system, before.system),
            idle: pct(now.idle, before.idle),
            iowait: pct(now.iowait, before.iowait),
            irq: pct(now.irq, before.irq),
            softirq: pct(now.softirq, before.softirq),
            steal: pct(now.steal, before.steal),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StatCounters {
    context_switches: u64,
    interrupts: u64,
    forks: u64,
}

/// Holds the previous `/proc/stat` reading so usage can be expressed as a
/// delta. CPU usage is meaningless without two points in time.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous_total: Jiffies,
    previous_cores: Vec<Jiffies>,
    previous_counters: StatCounters,
    /// Microjoules from the RAPL counter, for deriving watts.
    previous_energy_uj: Option<u64>,
    /// Cached location of the RAPL counter; empty when there is not one.
    rapl_path: Option<std::path::PathBuf>,
    static_info: Option<StaticCpuInfo>,
}

#[derive(Debug, Clone, Default)]
struct StaticCpuInfo {
    model: String,
    vendor: String,
    architecture: String,
    physical_cores: usize,
    logical_cores: usize,
    threads_per_core: usize,
    core_of_cpu: Vec<Option<usize>>,
    cache_l1d: Option<u64>,
    cache_l1i: Option<u64>,
    cache_l2: Option<u64>,
    cache_l3: Option<u64>,
}

impl CpuSampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, elapsed: f64) -> CpuMetrics {
        let info = self.static_info.get_or_insert_with(read_static_info).clone();
        let stat = sysfs::read_string("/proc/stat").unwrap_or_default();

        let (total, per_core, counters) = parse_proc_stat(&stat);

        let breakdown = Jiffies::delta_percent(total, self.previous_total);
        let mut cores = Vec::with_capacity(per_core.len());
        for (id, now) in per_core.iter().copied().enumerate() {
            let before = self.previous_cores.get(id).copied().unwrap_or_default();
            let core_breakdown = Jiffies::delta_percent(now, before);
            cores.push(CoreMetrics {
                id,
                physical_core: info.core_of_cpu.get(id).copied().flatten(),
                usage_percent: core_breakdown.busy(),
                breakdown: core_breakdown,
                frequency_hz: current_core_frequency_hz(id),
                max_frequency_hz: sysfs::read_f64(format!(
                    "/sys/devices/system/cpu/cpu{id}/cpufreq/cpuinfo_max_freq"
                ))
                .map(|khz| khz * 1000.0),
            });
        }

        self.previous_total = total;
        self.previous_cores = per_core;

        let rate = |now: u64, before: u64| {
            if elapsed <= 0.0 || now < before { 0.0 } else { (now - before) as f64 / elapsed }
        };
        let context_switches_per_sec =
            rate(counters.context_switches, self.previous_counters.context_switches);
        let interrupts_per_sec = rate(counters.interrupts, self.previous_counters.interrupts);
        let forks_per_sec = rate(counters.forks, self.previous_counters.forks);
        self.previous_counters = counters;

        let live_freqs: Vec<f64> = cores.iter().filter_map(|c| c.frequency_hz).collect();
        let frequency_hz = if live_freqs.is_empty() {
            None
        } else {
            Some(live_freqs.iter().sum::<f64>() / live_freqs.len() as f64)
        };
        let peak_frequency_hz = live_freqs
            .iter()
            .copied()
            .fold(None, |acc: Option<f64>, f| Some(acc.map_or(f, |a: f64| a.max(f))));

        let sensors = read_cpu_sensors();
        let temperature_c = pick_package_temperature(&sensors);
        let (procs_running, procs_blocked) = parse_proc_counts(&stat);

        CpuMetrics {
            model: info.model,
            vendor: info.vendor,
            architecture: info.architecture,
            physical_cores: info.physical_cores,
            logical_cores: if info.logical_cores == 0 { cores.len() } else { info.logical_cores },
            threads_per_core: info.threads_per_core,
            usage_percent: breakdown.busy(),
            breakdown,
            cores,
            frequency_hz,
            peak_frequency_hz,
            max_frequency_hz: sysfs::read_f64(
                "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq",
            )
            .map(|khz| khz * 1000.0),
            min_frequency_hz: sysfs::read_f64(
                "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_min_freq",
            )
            .map(|khz| khz * 1000.0),
            temperature_c,
            sensors,
            package_power_w: self.sample_package_power(elapsed),
            governor: sysfs::read_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            scaling_driver: sysfs::read_string(
                "/sys/devices/system/cpu/cpu0/cpufreq/scaling_driver",
            ),
            load_average: read_load_average(),
            context_switches_per_sec,
            interrupts_per_sec,
            forks_per_sec,
            procs_running,
            procs_blocked,
            process_count: 0,
            thread_count: 0,
            cache_l1d: info.cache_l1d,
            cache_l1i: info.cache_l1i,
            cache_l2: info.cache_l2,
            cache_l3: info.cache_l3,
        }
    }

    /// RAPL exposes a monotonic energy counter in microjoules; watts are the
    /// derivative of that. The counter is root-only on most distributions, so
    /// we fall back to a `hwmon` power sensor where one exists (`zenpower` on
    /// AMD, some server boards on Intel).
    fn sample_package_power(&mut self, elapsed: f64) -> Option<f64> {
        if let Some(watts) = read_hwmon_cpu_power() {
            return Some(watts);
        }

        let path =
            self.rapl_path.get_or_insert_with(|| find_rapl_package().unwrap_or_default()).clone();
        if path.as_os_str().is_empty() {
            return None;
        }

        let now = sysfs::read_u64(&path)?;
        let watts = match self.previous_energy_uj {
            Some(before) if now >= before && elapsed > 0.0 => {
                Some((now - before) as f64 / 1_000_000.0 / elapsed)
            }
            _ => None,
        };
        self.previous_energy_uj = Some(now);
        watts
    }
}

/// Locate the whole-package energy counter, whichever vendor driver provides
/// it. The directory is called `intel-rapl` even on AMD hardware.
fn find_rapl_package() -> Option<std::path::PathBuf> {
    sysfs::list_dir("/sys/class/powercap").into_iter().find_map(|dir| {
        let name = sysfs::read_string(dir.join("name"))?;
        if !name.starts_with("package") {
            return None;
        }
        let counter = dir.join("energy_uj");
        // Confirm it is actually readable; it is root-only on most distros.
        sysfs::read_u64(&counter).map(|_| counter)
    })
}

/// Some CPU `hwmon` drivers expose package power directly, in microwatts.
fn read_hwmon_cpu_power() -> Option<f64> {
    const CPU_CHIPS: [&str; 3] = ["zenpower", "k10temp", "coretemp"];
    sysfs::list_dir("/sys/class/hwmon").into_iter().find_map(|hwmon| {
        let name = sysfs::read_string(hwmon.join("name"))?;
        CPU_CHIPS
            .contains(&name.as_str())
            .then(|| sysfs::read_f64(hwmon.join("power1_input")).map(|uw| uw / 1_000_000.0))?
    })
}

fn parse_proc_stat(contents: &str) -> (Jiffies, Vec<Jiffies>, StatCounters) {
    let mut total = Jiffies::default();
    let mut cores = Vec::new();
    let mut counters = StatCounters::default();

    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else { continue };
        let rest: Vec<&str> = fields.collect();

        match label {
            "cpu" => total = Jiffies::parse(&rest),
            "ctxt" => {
                counters.context_switches = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            "intr" => {
                counters.interrupts = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            "processes" => {
                counters.forks = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            _ if label.starts_with("cpu") => cores.push(Jiffies::parse(&rest)),
            _ => {}
        }
    }

    (total, cores, counters)
}

fn parse_proc_counts(contents: &str) -> (u64, u64) {
    let find = |key: &str| {
        contents
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    (find("procs_running "), find("procs_blocked "))
}

fn current_core_frequency_hz(id: usize) -> Option<f64> {
    // `scaling_cur_freq` is the governor's view and is cheap; on some drivers
    // it is a cached value, so fall back to the hardware counter.
    sysfs::read_f64(format!("/sys/devices/system/cpu/cpu{id}/cpufreq/scaling_cur_freq"))
        .or_else(|| {
            sysfs::read_f64(format!("/sys/devices/system/cpu/cpu{id}/cpufreq/cpuinfo_cur_freq"))
        })
        .map(|khz| khz * 1000.0)
}

fn read_load_average() -> [f64; 3] {
    let Some(text) = sysfs::read_string("/proc/loadavg") else { return [0.0; 3] };
    let mut parts = text.split_whitespace();
    let mut out = [0.0; 3];
    for slot in &mut out {
        *slot = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    }
    out
}

/// Walk `hwmon` looking for anything that looks like a CPU package sensor.
fn read_cpu_sensors() -> Vec<TemperatureSensor> {
    const CPU_CHIPS: [&str; 5] = ["k10temp", "coretemp", "zenpower", "k8temp", "cpu_thermal"];
    let mut sensors = Vec::new();

    for hwmon in sysfs::list_dir("/sys/class/hwmon") {
        let Some(name) = sysfs::read_string(hwmon.join("name")) else { continue };
        if !CPU_CHIPS.contains(&name.as_str()) {
            continue;
        }
        for index in 1..=16 {
            let Some(celsius) = sysfs::hwmon_temp_c(&hwmon, index) else { continue };
            let raw =
                sysfs::hwmon_temp_label(&hwmon, index).unwrap_or_else(|| format!("Sensor {index}"));
            sensors.push(TemperatureSensor { label: humanise_sensor_label(&raw), celsius });
        }
    }

    sensors
}

/// Translate vendor sensor codenames into something a person understands.
fn humanise_sensor_label(raw: &str) -> String {
    match raw {
        "Tctl" => "Package (control)".to_owned(),
        "Tccd1" => "Core cluster 1".to_owned(),
        "Tccd2" => "Core cluster 2".to_owned(),
        "Tdie" => "Die".to_owned(),
        "Package id 0" => "Package".to_owned(),
        other if other.starts_with("Core ") => format!("Physical core {}", &other[5..]),
        other => other.to_owned(),
    }
}

fn pick_package_temperature(sensors: &[TemperatureSensor]) -> Option<f64> {
    sensors
        .iter()
        .find(|s| s.label.starts_with("Package") || s.label == "Die")
        .or_else(|| sensors.first())
        .map(|s| s.celsius)
}

fn read_static_info() -> StaticCpuInfo {
    let cpuinfo = sysfs::read_string("/proc/cpuinfo").unwrap_or_default();
    let field = |key: &str| {
        cpuinfo.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    };

    let logical_cores = cpuinfo.lines().filter(|l| l.starts_with("processor")).count();
    let core_of_cpu: Vec<Option<usize>> = (0..logical_cores)
        .map(|id| {
            sysfs::read_u64(format!("/sys/devices/system/cpu/cpu{id}/topology/core_id"))
                .map(|v| v as usize)
        })
        .collect();
    let mut unique: Vec<usize> = core_of_cpu.iter().flatten().copied().collect();
    unique.sort_unstable();
    unique.dedup();
    let physical_cores = if unique.is_empty() { logical_cores } else { unique.len() };

    StaticCpuInfo {
        model: field("model name").unwrap_or_else(|| "Unknown processor".to_owned()),
        vendor: humanise_vendor(field("vendor_id").as_deref().unwrap_or("")),
        architecture: std::env::consts::ARCH.to_owned(),
        physical_cores,
        logical_cores,
        threads_per_core: logical_cores.checked_div(physical_cores).unwrap_or(1).max(1),
        core_of_cpu,
        cache_l1d: read_cache_size(0, "Data"),
        cache_l1i: read_cache_size(0, "Instruction"),
        cache_l2: read_cache_level(2),
        cache_l3: read_cache_level(3),
    }
}

fn humanise_vendor(raw: &str) -> String {
    match raw {
        "AuthenticAMD" => "AMD",
        "GenuineIntel" => "Intel",
        "" => "Unknown",
        other => other,
    }
    .to_owned()
}

fn read_cache_level(level: u64) -> Option<u64> {
    cache_index_dirs().into_iter().find_map(|dir| {
        (sysfs::read_u64(dir.join("level"))? == level).then(|| parse_cache_size(&dir))?
    })
}

fn read_cache_size(level: u64, kind: &str) -> Option<u64> {
    cache_index_dirs().into_iter().find_map(|dir| {
        let matches = sysfs::read_u64(dir.join("level"))? == level + 1
            && sysfs::read_string(dir.join("type"))? == kind;
        matches.then(|| parse_cache_size(&dir))?
    })
}

fn cache_index_dirs() -> Vec<std::path::PathBuf> {
    sysfs::list_dir("/sys/devices/system/cpu/cpu0/cache")
        .into_iter()
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("index")))
        .collect()
}

/// Cache sizes are reported as strings like `32K` or `512K`.
fn parse_cache_size(dir: &Path) -> Option<u64> {
    let raw = sysfs::read_string(dir.join("size"))?;
    let (number, suffix) = raw.split_at(raw.find(|c: char| !c.is_ascii_digit())?);
    let value: u64 = number.parse().ok()?;
    Some(match suffix.to_ascii_uppercase().as_str() {
        "K" => value * 1024,
        "M" => value * 1024 * 1024,
        "G" => value * 1024 * 1024 * 1024,
        _ => value,
    })
}
