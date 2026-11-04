//! Rolling history buffers that back every graph in the UI.
//!
//! Each series is a fixed-capacity ring of `(elapsed_seconds, value)` points.
//! Capacity is expressed in *seconds of wall clock*, so changing the refresh
//! rate does not silently change how much history a graph shows.

use std::collections::{HashMap, VecDeque};

use super::Snapshot;

/// A single plotted line.
#[derive(Debug, Clone, Default)]
pub struct Series {
    points: VecDeque<[f64; 2]>,
    capacity: usize,
}

impl Series {
    pub fn new(capacity: usize) -> Self {
        Self { points: VecDeque::with_capacity(capacity), capacity: capacity.max(2) }
    }

    pub fn push(&mut self, x: f64, y: f64) {
        if self.points.len() >= self.capacity {
            self.points.pop_front();
        }
        self.points.push_back([x, y.max(0.0)]);
    }

    pub fn points(&self) -> Vec<[f64; 2]> {
        self.points.iter().copied().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn latest(&self) -> Option<f64> {
        self.points.back().map(|p| p[1])
    }

    pub fn max(&self) -> f64 {
        self.points.iter().map(|p| p[1]).fold(0.0, f64::max)
    }

    pub fn min(&self) -> f64 {
        self.points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min)
    }

    pub fn average(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        self.points.iter().map(|p| p[1]).sum::<f64>() / self.points.len() as f64
    }

    /// Oldest and newest X values, used to lock the plot's time axis.
    pub fn x_range(&self) -> Option<(f64, f64)> {
        Some((self.points.front()?[0], self.points.back()?[0]))
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }
}

/// Every graph in the app, keyed by a stable string id such as
/// `cpu.usage`, `cpu.core.3`, `gpu.0.temperature` or `disk.nvme0n1.read`.
#[derive(Debug)]
pub struct History {
    series: HashMap<String, Series>,
    capacity: usize,
    /// Seconds of history retained, shown next to each graph.
    pub window_secs: f64,
}

impl History {
    pub fn new(window_secs: f64, refresh_hz: f64) -> Self {
        let capacity = ((window_secs * refresh_hz).ceil() as usize).clamp(16, 20_000);
        Self { series: HashMap::new(), capacity, window_secs }
    }

    /// Re-size every buffer after the user changes the refresh rate.
    pub fn resize(&mut self, window_secs: f64, refresh_hz: f64) {
        self.window_secs = window_secs;
        self.capacity = ((window_secs * refresh_hz).ceil() as usize).clamp(16, 20_000);
        for series in self.series.values_mut() {
            series.capacity = self.capacity;
            while series.points.len() > self.capacity {
                series.points.pop_front();
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&Series> {
        self.series.get(key)
    }

    /// Fetch a series, or an empty placeholder so callers never branch.
    pub fn get_or_empty(&self, key: &str) -> Series {
        self.series.get(key).cloned().unwrap_or_default()
    }

    fn record(&mut self, key: impl Into<String>, x: f64, y: f64) {
        let capacity = self.capacity;
        self.series.entry(key.into()).or_insert_with(|| Series::new(capacity)).push(x, y);
    }

    fn record_opt(&mut self, key: impl Into<String>, x: f64, y: Option<f64>) {
        if let Some(value) = y {
            self.record(key, x, value);
        }
    }

    pub fn clear(&mut self) {
        self.series.clear();
    }

    /// Fold one snapshot into every relevant series.
    pub fn ingest(&mut self, snapshot: &Snapshot) {
        let x = snapshot.elapsed_secs;

        // ---- CPU ----
        self.record("cpu.usage", x, snapshot.cpu.usage_percent);
        self.record("cpu.user", x, snapshot.cpu.breakdown.user + snapshot.cpu.breakdown.nice);
        self.record("cpu.system", x, snapshot.cpu.breakdown.system);
        self.record("cpu.iowait", x, snapshot.cpu.breakdown.iowait);
        self.record_opt("cpu.frequency", x, snapshot.cpu.frequency_hz.map(|hz| hz / 1e9));
        self.record_opt("cpu.temperature", x, snapshot.cpu.temperature_c);
        self.record_opt("cpu.power", x, snapshot.cpu.package_power_w);
        self.record("cpu.load1", x, snapshot.cpu.load_average[0]);
        self.record("cpu.context_switches", x, snapshot.cpu.context_switches_per_sec);
        for core in &snapshot.cpu.cores {
            self.record(format!("cpu.core.{}", core.id), x, core.usage_percent);
        }

        // ---- Memory ----
        self.record("memory.used_percent", x, snapshot.memory.used_percent());
        self.record("memory.used", x, snapshot.memory.used as f64);
        self.record("memory.cached", x, snapshot.memory.reclaimable() as f64);
        self.record("memory.swap_percent", x, snapshot.memory.swap_used_percent());
        self.record("memory.swap_used", x, snapshot.memory.swap_used as f64);
        self.record("memory.major_faults", x, snapshot.memory.major_faults_per_sec);

        // ---- GPU ----
        for gpu in &snapshot.gpus {
            let prefix = format!("gpu.{}", gpu.index);
            self.record_opt(format!("{prefix}.usage"), x, gpu.utilization_percent);
            self.record_opt(format!("{prefix}.memory_bus"), x, gpu.memory_bus_percent);
            self.record_opt(format!("{prefix}.vram_percent"), x, gpu.vram_used_percent());
            self.record_opt(format!("{prefix}.vram_used"), x, gpu.vram_used.map(|v| v as f64));
            self.record_opt(format!("{prefix}.temperature"), x, gpu.temperature_c);
            self.record_opt(format!("{prefix}.power"), x, gpu.power_w);
            self.record_opt(format!("{prefix}.fan"), x, gpu.fan_percent);
            self.record_opt(format!("{prefix}.encoder"), x, gpu.encoder_percent);
            self.record_opt(format!("{prefix}.decoder"), x, gpu.decoder_percent);
            self.record_opt(format!("{prefix}.pcie_tx"), x, gpu.pcie_tx_bytes_per_sec);
            self.record_opt(format!("{prefix}.pcie_rx"), x, gpu.pcie_rx_bytes_per_sec);
            if let Some(core) = gpu.clocks.first() {
                self.record_opt(format!("{prefix}.clock"), x, core.current_hz.map(|hz| hz / 1e6));
            }
        }

        // ---- Storage ----
        for disk in &snapshot.disks {
            let prefix = format!("disk.{}", disk.name);
            self.record(format!("{prefix}.read"), x, disk.read_bytes_per_sec);
            self.record(format!("{prefix}.write"), x, disk.write_bytes_per_sec);
            self.record(format!("{prefix}.busy"), x, disk.busy_percent);
            self.record(format!("{prefix}.iops"), x, disk.total_ops_per_sec());
            self.record(format!("{prefix}.queue"), x, disk.queue_depth);
            self.record_opt(format!("{prefix}.temperature"), x, disk.temperature_c);
        }
        self.record("disk.total.read", x, snapshot.total_disk_read());
        self.record("disk.total.write", x, snapshot.total_disk_write());

        // ---- Network ----
        self.record("net.total.down", x, snapshot.total_net_down());
        self.record("net.total.up", x, snapshot.total_net_up());
        for interface in snapshot.networks.iter().filter(|n| !n.is_loopback) {
            self.record(format!("net.{}.down", interface.name), x, interface.rx_bytes_per_sec);
            self.record(format!("net.{}.up", interface.name), x, interface.tx_bytes_per_sec);
        }
    }
}
