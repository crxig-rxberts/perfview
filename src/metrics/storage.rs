//! Storage telemetry: physical devices from `/proc/diskstats` and `/sys/block`,
//! plus the mounted filesystems people actually recognise.
//!
//! Throughput, IOPS, latency and queue depth are all derived from the kernel's
//! monotonic counters, so the first sample after startup is intentionally
//! blank rather than wrong.

use std::{collections::HashMap, path::Path};

use super::sysfs;

/// The kernel always reports disk I/O in 512-byte sectors, regardless of the
/// device's real block size.
const SECTOR_BYTES: u64 = 512;

/// A physical block device, e.g. an NVMe SSD or a spinning hard disk.
#[derive(Debug, Clone, Default)]
pub struct DiskMetrics {
    /// Kernel name, e.g. `nvme0n1`.
    pub name: String,
    /// Marketing name from the firmware, e.g. `Seagate FireCuda 510 SSD`.
    pub model: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    /// "NVMe solid state drive", "SATA solid state drive", "Hard disk".
    pub kind: String,
    pub size_bytes: u64,
    /// `true` for spinning rust, `false` for flash.
    pub is_rotational: bool,
    /// Whether the device is removable, e.g. a USB stick.
    pub is_removable: bool,
    /// Physical sector size, useful when diagnosing alignment issues.
    pub physical_block_size: Option<u64>,
    /// I/O scheduler currently in use, e.g. `none`, `mq-deadline`.
    pub scheduler: Option<String>,

    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub read_ops_per_sec: f64,
    pub write_ops_per_sec: f64,
    /// Average time a read took to complete, in milliseconds.
    pub read_latency_ms: Option<f64>,
    pub write_latency_ms: Option<f64>,
    /// Percentage of wall-clock time the device had at least one request
    /// in flight. 100% means the drive is the bottleneck.
    pub busy_percent: f64,
    /// Average number of requests queued, i.e. how deep the backlog is.
    pub queue_depth: f64,
    /// Requests in flight at the instant of sampling.
    pub in_flight: u64,

    /// Total bytes read and written since boot.
    pub total_read_bytes: u64,
    pub total_write_bytes: u64,

    pub temperature_c: Option<f64>,
    /// Filesystems living on this device.
    pub volumes: Vec<String>,
}

/// A mounted filesystem.
#[derive(Debug, Clone, Default)]
pub struct VolumeMetrics {
    /// Where it is mounted, e.g. `/` or `/home`.
    pub mount_point: String,
    /// Device node backing it, e.g. `/dev/nvme0n1p2`.
    pub device: String,
    /// Filesystem type, e.g. `ext4`, `btrfs`.
    pub file_system: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
    pub is_read_only: bool,
}

impl VolumeMetrics {
    pub fn used_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }

    /// Volumes above this are worth flagging in amber/red.
    pub fn is_nearly_full(&self) -> bool {
        self.used_percent() >= 90.0
    }

    /// A friendly name for the mount point, e.g. "System drive".
    pub fn friendly_name(&self) -> String {
        match self.mount_point.as_str() {
            "/" => "System drive".to_owned(),
            "/home" => "Home".to_owned(),
            "/boot" => "Boot".to_owned(),
            "/boot/efi" => "EFI boot partition".to_owned(),
            other => {
                other.rsplit('/').find(|segment| !segment.is_empty()).unwrap_or(other).to_owned()
            }
        }
    }
}

impl DiskMetrics {
    pub fn total_bytes_per_sec(&self) -> f64 {
        self.read_bytes_per_sec + self.write_bytes_per_sec
    }

    pub fn total_ops_per_sec(&self) -> f64 {
        self.read_ops_per_sec + self.write_ops_per_sec
    }
}

/// Raw counters for one device from `/proc/diskstats`.
#[derive(Debug, Clone, Copy, Default)]
struct DiskStat {
    reads: u64,
    read_sectors: u64,
    read_ms: u64,
    writes: u64,
    write_sectors: u64,
    write_ms: u64,
    in_flight: u64,
    io_ms: u64,
    weighted_io_ms: u64,
}

#[derive(Debug, Default)]
pub struct StorageSampler {
    previous: HashMap<String, DiskStat>,
    /// Static device facts are read once; firmware strings never change.
    static_cache: HashMap<String, StaticDiskInfo>,
}

#[derive(Debug, Clone, Default)]
struct StaticDiskInfo {
    model: Option<String>,
    serial: Option<String>,
    firmware: Option<String>,
    kind: String,
    size_bytes: u64,
    is_rotational: bool,
    is_removable: bool,
    physical_block_size: Option<u64>,
}

impl StorageSampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, elapsed: f64, volumes: &[VolumeMetrics]) -> Vec<DiskMetrics> {
        let stats = read_diskstats();
        let mut disks = Vec::new();

        for (name, now) in &stats {
            if !is_interesting_device(name) {
                continue;
            }

            let before = self.previous.get(name).copied();
            let info = self
                .static_cache
                .entry(name.clone())
                .or_insert_with(|| read_static_disk_info(name))
                .clone();

            // Skip pseudo devices such as the tiny vendor "welcome disk"
            // partitions that ship on some hardware; they are not drives in
            // any sense the user cares about.
            const SMALLEST_REAL_DRIVE: u64 = 64 * 1024 * 1024;
            if info.size_bytes < SMALLEST_REAL_DRIVE {
                continue;
            }

            let rate = |now_v: u64, before_v: u64| -> f64 {
                match before {
                    Some(_) if elapsed > 0.0 && now_v >= before_v => {
                        (now_v - before_v) as f64 / elapsed
                    }
                    _ => 0.0,
                }
            };

            let read_ops = rate(now.reads, before.map_or(0, |b| b.reads));
            let write_ops = rate(now.writes, before.map_or(0, |b| b.writes));
            let delta_read_ms = before.map_or(0, |b| now.read_ms.saturating_sub(b.read_ms));
            let delta_write_ms = before.map_or(0, |b| now.write_ms.saturating_sub(b.write_ms));
            let delta_reads = before.map_or(0, |b| now.reads.saturating_sub(b.reads));
            let delta_writes = before.map_or(0, |b| now.writes.saturating_sub(b.writes));

            let elapsed_ms = elapsed * 1000.0;
            let busy_percent = if elapsed_ms > 0.0 {
                let delta = before.map_or(0, |b| now.io_ms.saturating_sub(b.io_ms));
                ((delta as f64 / elapsed_ms) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            let queue_depth = if elapsed_ms > 0.0 {
                let delta =
                    before.map_or(0, |b| now.weighted_io_ms.saturating_sub(b.weighted_io_ms));
                delta as f64 / elapsed_ms
            } else {
                0.0
            };

            let matching_volumes: Vec<String> = volumes
                .iter()
                .filter(|v| v.device.trim_start_matches("/dev/").starts_with(name.as_str()))
                .map(VolumeMetrics::friendly_name)
                .collect();

            disks.push(DiskMetrics {
                name: name.clone(),
                model: info.model,
                serial: info.serial,
                firmware: info.firmware,
                kind: info.kind,
                size_bytes: info.size_bytes,
                is_rotational: info.is_rotational,
                is_removable: info.is_removable,
                physical_block_size: info.physical_block_size,
                scheduler: read_scheduler(name),
                read_bytes_per_sec: rate(now.read_sectors, before.map_or(0, |b| b.read_sectors))
                    * SECTOR_BYTES as f64,
                write_bytes_per_sec: rate(now.write_sectors, before.map_or(0, |b| b.write_sectors))
                    * SECTOR_BYTES as f64,
                read_ops_per_sec: read_ops,
                write_ops_per_sec: write_ops,
                read_latency_ms: (delta_reads > 0)
                    .then(|| delta_read_ms as f64 / delta_reads as f64),
                write_latency_ms: (delta_writes > 0)
                    .then(|| delta_write_ms as f64 / delta_writes as f64),
                busy_percent,
                queue_depth,
                in_flight: now.in_flight,
                total_read_bytes: now.read_sectors * SECTOR_BYTES,
                total_write_bytes: now.write_sectors * SECTOR_BYTES,
                temperature_c: read_disk_temperature(name),
                volumes: matching_volumes,
            });
        }

        self.previous = stats;
        disks.sort_by(|a, b| a.name.cmp(&b.name));
        disks
    }
}

fn read_diskstats() -> HashMap<String, DiskStat> {
    let mut map = HashMap::new();
    let Some(text) = sysfs::read_string("/proc/diskstats") else { return map };

    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let parse = |i: usize| fields.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        map.insert(
            fields[2].to_owned(),
            DiskStat {
                reads: parse(3),
                read_sectors: parse(5),
                read_ms: parse(6),
                writes: parse(7),
                write_sectors: parse(9),
                write_ms: parse(10),
                in_flight: parse(11),
                io_ms: parse(12),
                weighted_io_ms: parse(13),
            },
        );
    }

    map
}

/// Filter out loopback (snap packages), ram disks and device-mapper noise, and
/// partitions - we only want whole drives on the storage tab.
fn is_interesting_device(name: &str) -> bool {
    const NOISE: [&str; 4] = ["loop", "ram", "zram", "dm-"];
    if NOISE.iter().any(|prefix| name.starts_with(prefix)) {
        return false;
    }
    // A whole device always has its own `/sys/block` entry; partitions do not.
    Path::new(&format!("/sys/block/{name}")).exists()
}

fn read_static_disk_info(name: &str) -> StaticDiskInfo {
    let block = Path::new("/sys/block").join(name);
    let device = block.join("device");
    let is_rotational = sysfs::read_u64(block.join("queue/rotational")) == Some(1);
    let is_nvme = name.starts_with("nvme");

    StaticDiskInfo {
        model: sysfs::read_string(device.join("model"))
            .filter(|m| !m.is_empty())
            .map(|m| m.trim().to_owned()),
        serial: sysfs::read_string(device.join("serial")),
        firmware: sysfs::read_string(device.join("firmware_rev"))
            .or_else(|| sysfs::read_string(device.join("rev"))),
        kind: match (is_nvme, is_rotational) {
            (true, _) => "NVMe solid state drive".to_owned(),
            (false, true) => "Hard disk drive".to_owned(),
            (false, false) => "Solid state drive".to_owned(),
        },
        // `/sys/block/*/size` is in 512-byte sectors.
        size_bytes: sysfs::read_u64(block.join("size")).unwrap_or(0) * SECTOR_BYTES,
        is_rotational,
        is_removable: sysfs::read_u64(block.join("removable")) == Some(1),
        physical_block_size: sysfs::read_u64(block.join("queue/physical_block_size")),
    }
}

fn read_scheduler(name: &str) -> Option<String> {
    // The file lists every scheduler with the active one in brackets.
    let raw = sysfs::read_string(format!("/sys/block/{name}/queue/scheduler"))?;
    raw.split_whitespace()
        .find(|token| token.starts_with('['))
        .map(|token| token.trim_matches(['[', ']']).to_owned())
        .or(Some(raw))
}

/// NVMe drives expose a composite temperature through `hwmon`.
fn read_disk_temperature(name: &str) -> Option<f64> {
    let device = Path::new("/sys/block").join(name).join("device");
    let hwmon = sysfs::hwmon_for_device(&device)
        .or_else(|| sysfs::hwmon_for_device(&device.join("device")))?;
    sysfs::hwmon_temp_c(&hwmon, 1)
}

/// Build the volume list from `sysinfo`, which handles the awkward parts of
/// `statvfs` and mount-table parsing for us.
pub fn read_volumes(disks: &sysinfo::Disks) -> Vec<VolumeMetrics> {
    let mut volumes: Vec<VolumeMetrics> = disks
        .iter()
        .filter(|disk| disk.total_space() > 0)
        .map(|disk| {
            let total = disk.total_space();
            let available = disk.available_space();
            VolumeMetrics {
                mount_point: disk.mount_point().to_string_lossy().into_owned(),
                device: disk.name().to_string_lossy().into_owned(),
                file_system: disk.file_system().to_string_lossy().into_owned(),
                total_bytes: total,
                used_bytes: total.saturating_sub(available),
                available_bytes: available,
                is_removable: disk.is_removable(),
                is_read_only: disk.is_read_only(),
            }
        })
        .collect();

    volumes.sort_by_key(|v| std::cmp::Reverse(v.total_bytes));
    volumes
}
