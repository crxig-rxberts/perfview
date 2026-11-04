//! Memory telemetry, read straight from `/proc/meminfo` and `/proc/vmstat`.
//!
//! The headline number most tools show ("used") is misleading on Linux because
//! the kernel deliberately fills spare RAM with disk cache. We therefore keep
//! the full breakdown so the UI can explain that cached memory is *free for the
//! taking*, not lost.

use super::sysfs;

#[derive(Debug, Clone, Default)]
pub struct MemoryMetrics {
    /// Total physical RAM installed, in bytes.
    pub total: u64,
    /// RAM genuinely held by applications. `total - available`.
    pub used: u64,
    /// What the kernel estimates a new app could claim without swapping.
    pub available: u64,
    /// Completely untouched RAM.
    pub free: u64,
    /// Filesystem cache: reclaimable instantly if something needs the space.
    pub cached: u64,
    /// Block-device write buffers.
    pub buffers: u64,
    /// Kernel's own data structures.
    pub slab: u64,
    pub slab_reclaimable: u64,
    /// `tmpfs` and shared-memory segments, including `/dev/shm`.
    pub shared: u64,
    /// Memory mapped from files, e.g. program code and game assets.
    pub mapped: u64,
    /// Dirty pages still waiting to be written back to disk.
    pub dirty: u64,
    /// Pages currently being written back.
    pub writeback: u64,

    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_free: u64,
    /// Swapped-out pages that are also still cached in RAM.
    pub swap_cached: u64,

    /// Compressed swap pool, if `zswap`/`zram` is in use.
    pub zswap_compressed: u64,
    pub zswap_original: u64,

    /// Huge pages, which games and databases sometimes rely on.
    pub hugepages_total: u64,
    pub hugepages_free: u64,
    pub hugepage_size: u64,

    /// Memory the kernel has promised to processes, which may exceed `total`.
    pub committed: u64,
    pub commit_limit: u64,

    /// Pages pulled in from disk per second. Sustained values mean thrashing.
    pub page_ins_per_sec: f64,
    pub page_outs_per_sec: f64,
    pub swap_ins_per_sec: f64,
    pub swap_outs_per_sec: f64,
    /// Faults that required a disk read, the expensive kind.
    pub major_faults_per_sec: f64,

    /// Populated DIMM slots, from DMI. Requires root for full detail.
    pub modules: Vec<MemoryModule>,
}

/// A physical RAM stick.
#[derive(Debug, Clone)]
pub struct MemoryModule {
    pub locator: String,
    pub size_bytes: u64,
    pub speed_mts: Option<u64>,
    pub kind: Option<String>,
    pub manufacturer: Option<String>,
}

impl MemoryMetrics {
    /// Percentage of RAM actually unavailable to new work.
    pub fn used_percent(&self) -> f64 {
        if self.total == 0 { 0.0 } else { (self.used as f64 / self.total as f64) * 100.0 }
    }

    pub fn swap_used_percent(&self) -> f64 {
        if self.swap_total == 0 {
            0.0
        } else {
            (self.swap_used as f64 / self.swap_total as f64) * 100.0
        }
    }

    /// Cache plus buffers: the "looks used but isn't" portion.
    pub fn reclaimable(&self) -> u64 {
        self.cached + self.buffers + self.slab_reclaimable
    }
}

#[derive(Debug, Default)]
pub struct MemorySampler {
    previous: Option<VmStatCounters>,
}

#[derive(Debug, Clone, Copy, Default)]
struct VmStatCounters {
    pgpgin: u64,
    pgpgout: u64,
    pswpin: u64,
    pswpout: u64,
    pgmajfault: u64,
}

impl MemorySampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, elapsed: f64) -> MemoryMetrics {
        let meminfo = sysfs::read_string("/proc/meminfo").unwrap_or_default();
        let get = |key: &str| sysfs::parse_key_kb(&meminfo, key).unwrap_or(0);

        let total = get("MemTotal");
        let available = get("MemAvailable");
        let swap_total = get("SwapTotal");
        let swap_free = get("SwapFree");

        let counters = read_vmstat();
        let rate = |now: u64, before: u64| {
            if elapsed <= 0.0 || now < before { 0.0 } else { (now - before) as f64 / elapsed }
        };
        let previous = self.previous.unwrap_or(counters);
        let metrics = MemoryMetrics {
            total,
            used: total.saturating_sub(available),
            available,
            free: get("MemFree"),
            cached: get("Cached"),
            buffers: get("Buffers"),
            slab: get("Slab"),
            slab_reclaimable: get("SReclaimable"),
            shared: get("Shmem"),
            mapped: get("Mapped"),
            dirty: get("Dirty"),
            writeback: get("Writeback"),
            swap_total,
            swap_used: swap_total.saturating_sub(swap_free),
            swap_free,
            swap_cached: get("SwapCached"),
            zswap_compressed: get("Zswap"),
            zswap_original: get("Zswapped"),
            // These two are counts of pages, not kibibytes, so undo the x1024.
            hugepages_total: get("HugePages_Total") / 1024,
            hugepages_free: get("HugePages_Free") / 1024,
            hugepage_size: get("Hugepagesize"),
            committed: get("Committed_AS"),
            commit_limit: get("CommitLimit"),
            page_ins_per_sec: rate(counters.pgpgin, previous.pgpgin),
            page_outs_per_sec: rate(counters.pgpgout, previous.pgpgout),
            swap_ins_per_sec: rate(counters.pswpin, previous.pswpin),
            swap_outs_per_sec: rate(counters.pswpout, previous.pswpout),
            major_faults_per_sec: rate(counters.pgmajfault, previous.pgmajfault),
            modules: Vec::new(),
        };
        self.previous = Some(counters);
        metrics
    }
}

fn read_vmstat() -> VmStatCounters {
    let text = sysfs::read_string("/proc/vmstat").unwrap_or_default();
    let get = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key).filter(|r| r.starts_with(' ')))
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    VmStatCounters {
        // `/proc/vmstat` counts these in kibibytes of pages moved.
        pgpgin: get("pgpgin") * 1024,
        pgpgout: get("pgpgout") * 1024,
        pswpin: get("pswpin"),
        pswpout: get("pswpout"),
        pgmajfault: get("pgmajfault"),
    }
}

/// Read DIMM details from DMI. This is static, so the collector caches it.
pub fn read_memory_modules() -> Vec<MemoryModule> {
    let mut modules = Vec::new();
    for entry in sysfs::list_dir("/sys/firmware/dmi/entries") {
        let name = entry.file_name().unwrap_or_default().to_string_lossy().into_owned();
        // Type 17 is "Memory Device" in the SMBIOS spec.
        if !name.starts_with("17-") {
            continue;
        }
        let Some(raw) = std::fs::read(entry.join("raw")).ok() else { continue };
        if let Some(module) = parse_dmi_memory_device(&raw) {
            modules.push(module);
        }
    }
    modules
}

/// Minimal SMBIOS type-17 parser: just enough for size, speed and type.
fn parse_dmi_memory_device(raw: &[u8]) -> Option<MemoryModule> {
    let header_len = *raw.get(1)? as usize;
    let size_field = u16::from_le_bytes([*raw.get(0x0C)?, *raw.get(0x0D)?]);
    if size_field == 0 {
        return None; // Empty slot.
    }
    let size_bytes = if size_field == 0x7FFF {
        u64::from(u32::from_le_bytes([
            *raw.get(0x1C)?,
            *raw.get(0x1D)?,
            *raw.get(0x1E)?,
            *raw.get(0x1F)?,
        ])) * 1024
            * 1024
    } else if size_field & 0x8000 != 0 {
        u64::from(size_field & 0x7FFF) * 1024
    } else {
        u64::from(size_field) * 1024 * 1024
    };

    let strings: Vec<&[u8]> = raw.get(header_len..)?.split(|b| *b == 0).collect();
    let string_at = |index: u8| -> Option<String> {
        if index == 0 {
            return None;
        }
        strings
            .get(index as usize - 1)
            .map(|s| String::from_utf8_lossy(s).trim().to_owned())
            .filter(|s| !s.is_empty())
    };

    let speed = u16::from_le_bytes([*raw.get(0x15)?, *raw.get(0x16)?]);
    Some(MemoryModule {
        locator: string_at(*raw.get(0x10)?).unwrap_or_else(|| "DIMM".to_owned()),
        size_bytes,
        speed_mts: (speed > 0).then_some(u64::from(speed)),
        kind: raw.get(0x12).map(|c| memory_type_name(*c).to_owned()),
        manufacturer: string_at(*raw.get(0x17)?),
    })
}

/// SMBIOS memory type codes, translated to names people recognise.
fn memory_type_name(code: u8) -> &'static str {
    match code {
        0x12 => "DDR",
        0x13 => "DDR2",
        0x18 => "DDR3",
        0x1A => "DDR4",
        0x22 => "DDR5",
        0x23 => "LPDDR5",
        0x1F => "LPDDR3",
        0x20 => "LPDDR4",
        _ => "Unknown",
    }
}
