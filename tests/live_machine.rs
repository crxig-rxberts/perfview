//! End-to-end checks against the real machine the tests run on.
//!
//! These intentionally assert on *shape* rather than exact values, because the
//! numbers differ on every machine. What they prove is that the collector can
//! actually read a live Linux system and produce figures that make sense -
//! which is exactly the thing that silently breaks when a kernel changes a
//! file format.

use std::time::Duration;

use perfview::metrics::{
    cpu::CpuSampler, memory::MemorySampler, network::NetworkSampler, storage::StorageSampler,
    system::SystemSampler,
};

/// Give the counters time to move between the two samples.
const SETTLE: Duration = Duration::from_millis(150);

#[test]
fn reads_processor_topology_and_load() {
    let mut sampler = CpuSampler::new();
    let _ = sampler.sample(0.0);
    std::thread::sleep(SETTLE);
    let cpu = sampler.sample(SETTLE.as_secs_f64());

    assert!(cpu.logical_cores > 0, "a machine with no logical processors is impossible");
    assert!(cpu.physical_cores > 0, "a machine with no physical cores is impossible");
    assert!(cpu.logical_cores >= cpu.physical_cores, "there cannot be fewer threads than cores");
    assert_eq!(cpu.cores.len(), cpu.logical_cores, "one entry per logical processor");
    assert!(!cpu.model.is_empty(), "the processor should have a name");

    assert!((0.0..=100.0).contains(&cpu.usage_percent), "usage must be a percentage");
    for core in &cpu.cores {
        assert!(
            (0.0..=100.0).contains(&core.usage_percent),
            "core {} reported {}%, which is not a percentage",
            core.id,
            core.usage_percent
        );
    }

    let breakdown = cpu.breakdown;
    let total = breakdown.user
        + breakdown.nice
        + breakdown.system
        + breakdown.idle
        + breakdown.iowait
        + breakdown.irq
        + breakdown.softirq
        + breakdown.steal;
    assert!((total - 100.0).abs() < 1.0, "the time breakdown should add up to 100%, got {total}");
}

#[test]
fn reads_memory_totals_that_add_up() {
    let mut sampler = MemorySampler::new();
    let memory = sampler.sample(1.0);

    assert!(memory.total > 0, "the machine must have some memory");
    assert!(memory.available <= memory.total, "available memory cannot exceed the total");
    assert!(memory.used <= memory.total, "used memory cannot exceed the total");
    assert!(memory.free <= memory.total, "free memory cannot exceed the total");
    assert_eq!(
        memory.used + memory.available,
        memory.total,
        "used and available should account for all memory"
    );
    assert!(memory.swap_used <= memory.swap_total, "swap in use cannot exceed swap configured");
    assert!((0.0..=100.0).contains(&memory.used_percent()));
}

#[test]
fn reads_physical_drives_and_their_filesystems() {
    let disks = sysinfo::Disks::new_with_refreshed_list().expect("filesystem list");
    let volumes = perfview::metrics::storage::read_volumes(&disks);

    let mut sampler = StorageSampler::new();
    let _ = sampler.sample(0.0, &volumes);
    std::thread::sleep(SETTLE);
    let drives = sampler.sample(SETTLE.as_secs_f64(), &volumes);

    for drive in &drives {
        assert!(!drive.name.is_empty());
        assert!(drive.size_bytes > 0, "{} reported a zero size", drive.name);
        assert!(
            (0.0..=100.0).contains(&drive.busy_percent),
            "{} reported {}% busy",
            drive.name,
            drive.busy_percent
        );
        assert!(drive.read_bytes_per_sec >= 0.0);
        assert!(drive.write_bytes_per_sec >= 0.0);
        assert!(
            !drive.name.starts_with("loop"),
            "snap loopback devices should be filtered out of the drive list"
        );
    }

    for volume in &volumes {
        assert!(volume.total_bytes > 0);
        assert!(volume.used_bytes <= volume.total_bytes);
        assert!((0.0..=100.0).contains(&volume.used_percent()));
        assert!(!volume.friendly_name().is_empty());
    }
}

#[test]
fn reads_network_interfaces_including_loopback() {
    let mut sampler = NetworkSampler::new();
    let _ = sampler.sample(0.0);
    std::thread::sleep(SETTLE);
    let interfaces = sampler.sample(SETTLE.as_secs_f64());

    assert!(!interfaces.is_empty(), "every Linux machine has at least a loopback interface");
    assert!(interfaces.iter().any(|i| i.is_loopback), "loopback should be present");

    for interface in &interfaces {
        assert!(!interface.name.is_empty());
        assert!(!interface.kind.is_empty(), "every interface should be classified in words");
        assert!(interface.rx_bytes_per_sec >= 0.0);
        assert!(interface.tx_bytes_per_sec >= 0.0);
    }
}

#[test]
fn identifies_the_machine() {
    let system = SystemSampler::new().sample();

    assert!(!system.hostname.is_empty());
    assert!(!system.kernel_version.is_empty());
    assert!(!system.distribution.is_empty());
    assert!(system.uptime_secs > 0, "the machine has clearly been running for a moment");
    assert!(system.boot_time.is_some(), "boot time should be derivable from uptime");
}

#[test]
fn gpu_detection_never_panics_without_a_driver() {
    // The point of this test is that it completes at all: the GPU sampler must
    // degrade gracefully on a CI runner with no graphics driver.
    let mut sampler = perfview::metrics::gpu::GpuSampler::new();
    let gpus = sampler.sample(1.0);

    for gpu in &gpus {
        assert!(!gpu.name.is_empty());
        if let Some(utilization) = gpu.utilization_percent {
            assert!((0.0..=100.0).contains(&utilization));
        }
        if let (Some(used), Some(total)) = (gpu.vram_used, gpu.vram_total) {
            assert!(used <= total, "used video memory cannot exceed the total");
        }
    }
}
