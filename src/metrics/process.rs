//! Per-process telemetry, used to answer "what is actually doing this?".

use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone, Default)]
pub struct ProcessMetrics {
    pub pid: u32,
    /// Short executable name, e.g. `steam`.
    pub name: String,
    /// Full command line, shown on hover.
    pub command: String,
    pub user: Option<String>,
    /// CPU usage as a percentage of a *single* core; can exceed 100.
    pub cpu_percent: f64,
    /// Resident set size: physical RAM the process is actually holding.
    pub memory_bytes: u64,
    /// Virtual size: address space reserved, usually far larger and less useful.
    pub virtual_bytes: u64,
    pub disk_read_bytes_per_sec: f64,
    pub disk_write_bytes_per_sec: f64,
    /// Human-readable state, e.g. "Running", "Sleeping".
    pub status: String,
    pub thread_count: usize,
    /// Seconds since the process started.
    pub run_time_secs: u64,
}

impl ProcessMetrics {
    /// CPU usage normalised across the whole machine, which is what a user
    /// means when they say "that app is using 25% of my CPU".
    pub fn cpu_percent_of_machine(&self, logical_cores: usize) -> f64 {
        if logical_cores == 0 { self.cpu_percent } else { self.cpu_percent / logical_cores as f64 }
    }
}

/// The slowest collector by far, so it runs on its own schedule.
///
/// Enumerating every process costs roughly 20 ms on a busy desktop. A table of
/// applications does not need updating four times a second, so we cap it at
/// once per second and reuse the previous list in between. Graphs stay smooth;
/// the table stays readable; the machine stays quiet.
#[derive(Debug, Default)]
pub struct ProcessSampler {
    last_sampled: Option<Instant>,
    cached: Vec<ProcessMetrics>,
}

/// Minimum gap between full process scans.
const SCAN_INTERVAL: Duration = Duration::from_millis(900);

impl ProcessSampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, system: &mut System, elapsed: f64) -> Vec<ProcessMetrics> {
        if self.last_sampled.is_some_and(|at| at.elapsed() < SCAN_INTERVAL) {
            return self.cached.clone();
        }
        let since_last = self.last_sampled.map_or(elapsed, |at| at.elapsed().as_secs_f64());
        self.last_sampled = Some(Instant::now());

        // `without_tasks` is the single most important flag here: by default
        // sysinfo walks every thread of every process, which on a busy desktop
        // means thousands of directory scans. We only want real processes.
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_user(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );

        let divisor = if since_last > 0.0 { since_last } else { 1.0 };
        let mut processes: Vec<ProcessMetrics> = system
            .processes()
            .iter()
            .filter(|(_, process)| process.thread_kind().is_none())
            .map(|(pid, process)| {
                let disk = process.disk_usage();
                ProcessMetrics {
                    pid: pid.as_u32(),
                    name: process.name().to_string_lossy().into_owned(),
                    command: process
                        .cmd()
                        .iter()
                        .map(|a| a.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" "),
                    user: None,
                    cpu_percent: f64::from(process.cpu_usage()),
                    memory_bytes: process.memory(),
                    virtual_bytes: process.virtual_memory(),
                    disk_read_bytes_per_sec: disk.read_bytes as f64 / divisor,
                    disk_write_bytes_per_sec: disk.written_bytes as f64 / divisor,
                    status: describe_status(&process.status().to_string()),
                    thread_count: count_threads(*pid),
                    run_time_secs: process.run_time(),
                }
            })
            .collect();

        processes.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        self.cached.clone_from(&processes);
        processes
    }
}

/// `sysinfo` returns terse kernel state names; spell them out.
fn describe_status(raw: &str) -> String {
    match raw {
        "Runnable" | "Run" | "Running" => "Running",
        "Sleeping" | "Interruptible" => "Sleeping",
        "UninterruptibleDiskSleep" | "Uninterruptible" => "Waiting on disk",
        "Stopped" => "Paused",
        "Zombie" => "Finished (not cleaned up)",
        "Idle" => "Idle",
        "Tracing" => "Being debugged",
        "Dead" => "Dead",
        other => other,
    }
    .to_owned()
}

/// Read a process's thread count from `/proc/<pid>/stat`.
///
/// This is deliberately not a directory scan of `/proc/<pid>/task`: a single
/// small file read is roughly three times cheaper, and it is done for every
/// process on every sample.
fn count_threads(pid: Pid) -> usize {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { return 1 };
    // The process name sits in brackets and may itself contain spaces, so the
    // numbered fields are counted from after the final closing bracket.
    // `num_threads` is field 20 overall, which is index 17 after the name.
    stat.rsplit(')')
        .next()
        .and_then(|rest| rest.split_whitespace().nth(17))
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
}
