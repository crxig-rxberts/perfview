//! Measure how long each collector takes.
//!
//! Run with `cargo run --release --example profile`.
//!
//! perfview sits on screen while you play games, so the cost of gathering a
//! reading matters. This prints the per-collector breakdown so a regression is
//! obvious. On a typical desktop the whole sample should land in single-digit
//! milliseconds.

use std::time::{Duration, Instant};

use perfview::metrics::{
    cpu::CpuSampler, gpu::GpuSampler, memory::MemorySampler, network::NetworkSampler,
    process::ProcessSampler, storage::StorageSampler,
};

const ROUNDS: usize = 4;
const GAP: Duration = Duration::from_millis(1_000);

fn main() {
    let mut system = sysinfo::System::new().expect("system telemetry");
    let disks = sysinfo::Disks::new_with_refreshed_list().expect("filesystem list");

    let mut cpu = CpuSampler::new();
    let mut memory = MemorySampler::new();
    let mut storage = StorageSampler::new();
    let mut network = NetworkSampler::new();
    let mut gpu = GpuSampler::new();
    let mut processes = ProcessSampler::new();

    // Prime every sampler so the first measured round is representative.
    let _ = cpu.sample(0.0);
    let _ = storage.sample(0.0, &[]);
    let _ = network.sample(0.0);
    let _ = gpu.sample(0.0);
    let _ = processes.sample(&mut system, 0.0);

    println!("Collector cost per sample, averaged over {ROUNDS} rounds\n");

    let mut totals = [0.0; 7];
    for _ in 0..ROUNDS {
        std::thread::sleep(GAP);
        let seconds = GAP.as_secs_f64();

        totals[0] += time(|| drop(cpu.sample(seconds)));
        totals[1] += time(|| drop(memory.sample(seconds)));
        let volumes = perfview::metrics::storage::read_volumes(&disks);
        totals[2] += time(|| drop(perfview::metrics::storage::read_volumes(&disks)));
        totals[3] += time(|| drop(storage.sample(seconds, &volumes)));
        totals[4] += time(|| drop(network.sample(seconds)));
        totals[5] += time(|| drop(gpu.sample(seconds)));
        totals[6] += time(|| drop(processes.sample(&mut system, seconds)));
    }

    const NAMES: [&str; 7] =
        ["Processor", "Memory", "Filesystems", "Drives", "Network", "Graphics", "Processes"];
    for (name, total) in NAMES.iter().zip(totals) {
        println!("  {name:<14}{:>8.2} ms", total / ROUNDS as f64);
    }
    println!("  {:<14}{:>8.2} ms", "TOTAL", totals.iter().sum::<f64>() / ROUNDS as f64);
}

fn time(work: impl FnOnce()) -> f64 {
    let started = Instant::now();
    work();
    started.elapsed().as_secs_f64() * 1000.0
}
