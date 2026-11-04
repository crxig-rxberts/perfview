//! Data model for a single point-in-time reading of the whole machine.
//!
//! Everything in here is deliberately stored in *raw SI-ish units* (bytes,
//! hertz, watts, degrees Celsius). Converting to something a human wants to
//! read is the job of [`crate::ui::format`], never the collector.

pub mod collector;
pub mod cpu;
pub mod gpu;
pub mod history;
pub mod memory;
pub mod network;
pub mod process;
pub mod storage;
pub mod system;

pub(crate) mod sysfs;

use std::time::Duration;

pub use cpu::CpuMetrics;
pub use gpu::GpuMetrics;
pub use memory::MemoryMetrics;
pub use network::NetworkMetrics;
pub use process::ProcessMetrics;
pub use storage::{DiskMetrics, VolumeMetrics};
pub use system::SystemMetrics;

/// One complete reading of the machine, produced by the sampling thread.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Wall-clock time the sample was taken.
    pub taken_at: chrono::DateTime<chrono::Local>,
    /// How long the sample took to gather. Used to show collector overhead.
    pub collection_time: Duration,
    /// Seconds since the sampler started, used as the X axis for every graph.
    pub elapsed_secs: f64,
    pub system: SystemMetrics,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub gpus: Vec<GpuMetrics>,
    pub disks: Vec<DiskMetrics>,
    pub volumes: Vec<VolumeMetrics>,
    pub networks: Vec<NetworkMetrics>,
    pub processes: Vec<ProcessMetrics>,
}

impl Snapshot {
    /// Total bytes/sec read across every physical disk.
    pub fn total_disk_read(&self) -> f64 {
        self.disks.iter().map(|d| d.read_bytes_per_sec).sum()
    }

    /// Total bytes/sec written across every physical disk.
    pub fn total_disk_write(&self) -> f64 {
        self.disks.iter().map(|d| d.write_bytes_per_sec).sum()
    }

    /// Total bytes/sec received across every non-loopback interface.
    pub fn total_net_down(&self) -> f64 {
        self.networks.iter().filter(|n| !n.is_loopback).map(|n| n.rx_bytes_per_sec).sum()
    }

    /// Total bytes/sec sent across every non-loopback interface.
    pub fn total_net_up(&self) -> f64 {
        self.networks.iter().filter(|n| !n.is_loopback).map(|n| n.tx_bytes_per_sec).sum()
    }

    /// The GPU the user most likely cares about: the busiest discrete one.
    pub fn primary_gpu(&self) -> Option<&GpuMetrics> {
        self.gpus.first()
    }
}
