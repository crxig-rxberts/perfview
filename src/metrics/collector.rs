//! The sampling thread.
//!
//! All telemetry is gathered off the UI thread so that a slow sysfs read can
//! never cause a dropped frame. Snapshots are pushed down a channel and the
//! egui context is nudged to repaint, which keeps the app at 0% CPU when
//! nothing is changing.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use sysinfo::{Disks, System};

use super::{
    Snapshot, cpu::CpuSampler, gpu::GpuSampler, memory::MemorySampler, network::NetworkSampler,
    process::ProcessSampler, storage::StorageSampler, system::SystemSampler,
};

/// Knobs the UI can turn while the thread is running.
#[derive(Debug)]
struct Shared {
    interval_ms: AtomicU64,
    paused: AtomicBool,
    shutdown: AtomicBool,
}

/// Handle to the background sampler.
#[derive(Debug)]
pub struct Collector {
    receiver: Receiver<Snapshot>,
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
    /// Problem encountered while initialising GPU telemetry, if any.
    pub gpu_notice: Option<String>,
    pub has_gpu: bool,
}

impl Collector {
    /// Spawn the sampler. `ctx` is used to wake the UI when data lands.
    pub fn spawn(interval: Duration, ctx: egui::Context) -> Self {
        let shared = Arc::new(Shared {
            interval_ms: AtomicU64::new(interval.as_millis() as u64),
            paused: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        });
        let (sender, receiver) = channel();

        // Probe the GPU on this thread so the UI can report problems straight
        // away rather than after the first sample lands.
        let gpu_sampler = GpuSampler::new();
        let gpu_notice = gpu_sampler.init_error.clone();
        let has_gpu = gpu_sampler.has_devices();

        let worker_shared = Arc::clone(&shared);
        let handle = std::thread::Builder::new()
            .name("perfview-sampler".to_owned())
            .spawn(move || run(&worker_shared, &sender, gpu_sampler, &ctx))
            .expect("failed to spawn the perfview sampling thread");

        Self { receiver, shared, handle: Some(handle), gpu_notice, has_gpu }
    }

    /// Take the newest snapshot, discarding any backlog. Returns `None` when
    /// nothing new has arrived since the last frame.
    pub fn poll(&self) -> Option<Snapshot> {
        let mut newest = None;
        while let Ok(snapshot) = self.receiver.try_recv() {
            newest = Some(snapshot);
        }
        newest
    }

    pub fn set_interval(&self, interval: Duration) {
        self.shared.interval_ms.store(interval.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn run(shared: &Arc<Shared>, sender: &Sender<Snapshot>, gpu: GpuSampler, ctx: &egui::Context) {
    let mut gpu = gpu;
    let mut cpu = CpuSampler::new();
    let mut memory = MemorySampler::new();
    let mut storage = StorageSampler::new();
    let mut network = NetworkSampler::new();
    let mut processes = ProcessSampler::new();
    let system_sampler = SystemSampler::new();

    let mut system = System::new().expect("failed to initialise system telemetry");
    let mut disks = Disks::new_with_refreshed_list()
        .unwrap_or_else(|_| Disks::new().expect("failed to initialise filesystem telemetry"));
    let memory_modules = super::memory::read_memory_modules();

    let started = Instant::now();
    let mut last_sample = Instant::now();
    // The very first CPU reading has no previous counter to diff against, so
    // prime the samplers and throw the result away.
    let _ = cpu.sample(0.0);
    let _ = storage.sample(0.0, &[]);
    let _ = network.sample(0.0);

    // How long to nap between checks of the shutdown flag.
    const TICK: Duration = Duration::from_millis(50);
    // How often to re-scan mount points and free space.
    const VOLUME_REFRESH_INTERVAL: Duration = Duration::from_secs(3);

    let mut last_volume_refresh: Option<Instant> = None;
    let mut volumes = Vec::new();

    loop {
        if shared.shutdown.load(Ordering::Relaxed) {
            return;
        }

        let interval = Duration::from_millis(shared.interval_ms.load(Ordering::Relaxed).max(100));
        if shared.paused.load(Ordering::Relaxed) || last_sample.elapsed() < interval {
            std::thread::sleep(TICK.min(interval));
            continue;
        }

        let elapsed = last_sample.elapsed().as_secs_f64();
        last_sample = Instant::now();
        let collection_started = Instant::now();

        // Mount points and their free space change rarely and `statvfs` has to
        // touch every filesystem, so this runs on a slow schedule of its own.
        if last_volume_refresh.is_none_or(|at| at.elapsed() >= VOLUME_REFRESH_INTERVAL) {
            last_volume_refresh = Some(Instant::now());
            disks.refresh(true);
            volumes = super::storage::read_volumes(&disks);
        }

        let mut cpu_metrics = cpu.sample(elapsed);
        let mut memory_metrics = memory.sample(elapsed);
        memory_metrics.modules.clone_from(&memory_modules);

        let process_list = processes.sample(&mut system, elapsed);
        cpu_metrics.process_count = process_list.len();
        cpu_metrics.thread_count = process_list.iter().map(|p| p.thread_count).sum();

        let snapshot = Snapshot {
            taken_at: chrono::Local::now(),
            elapsed_secs: started.elapsed().as_secs_f64(),
            system: system_sampler.sample(),
            cpu: cpu_metrics,
            memory: memory_metrics,
            gpus: gpu.sample(elapsed),
            disks: storage.sample(elapsed, &volumes),
            volumes: volumes.clone(),
            networks: network.sample(elapsed),
            processes: process_list,
            collection_time: collection_started.elapsed(),
        };

        if sender.send(snapshot).is_err() {
            return; // UI has gone away.
        }
        ctx.request_repaint();
    }
}
