//! Network telemetry from `/sys/class/net`, used on the overview tab to show
//! whether a stutter is the network's fault rather than the GPU's.

use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};

use super::sysfs;

#[derive(Debug, Clone, Default)]
pub struct NetworkMetrics {
    /// Kernel interface name, e.g. `enp6s0`.
    pub name: String,
    /// "Wired", "Wi-Fi", "Virtual", "Loopback".
    pub kind: String,
    pub is_loopback: bool,
    /// Whether the cable/radio is actually connected.
    pub is_up: bool,
    pub mac_address: Option<String>,
    /// Negotiated link speed in bits per second.
    pub link_speed_bps: Option<f64>,

    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    pub rx_packets_per_sec: f64,
    pub tx_packets_per_sec: f64,

    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,

    /// Dropped or errored packets since boot. Non-zero means a bad cable,
    /// a saturated link or a struggling driver.
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

impl NetworkMetrics {
    /// How much of the negotiated link speed is in use, as a percentage.
    pub fn utilisation_percent(&self) -> Option<f64> {
        let speed = self.link_speed_bps?;
        if speed <= 0.0 {
            return None;
        }
        let bits = (self.rx_bytes_per_sec + self.tx_bytes_per_sec) * 8.0;
        Some((bits / speed * 100.0).clamp(0.0, 100.0))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct NetCounters {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
    rx_errors: u64,
    tx_errors: u64,
    rx_dropped: u64,
    tx_dropped: u64,
}

/// Facts about an interface that rarely or never change.
#[derive(Debug, Clone)]
struct StaticInterfaceInfo {
    kind: &'static str,
    is_loopback: bool,
    mac_address: Option<String>,
}

/// How often to re-check link state and negotiated speed.
const LINK_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub struct NetworkSampler {
    previous: HashMap<String, NetCounters>,
    statics: HashMap<String, StaticInterfaceInfo>,
    /// Cached link state, refreshed on a slow schedule.
    links: HashMap<String, (bool, Option<f64>)>,
    links_checked: Option<Instant>,
}

impl NetworkSampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, elapsed: f64) -> Vec<NetworkMetrics> {
        let counters = read_proc_net_dev();

        // Link state needs a sysfs read per interface, so it is refreshed far
        // less often than the byte counters, which come from one file.
        let refresh_links =
            self.links_checked.is_none_or(|at| at.elapsed() >= LINK_REFRESH_INTERVAL);
        if refresh_links {
            self.links_checked = Some(Instant::now());
            self.links.clear();
        }

        let mut interfaces = Vec::with_capacity(counters.len());
        for (name, now) in &counters {
            let path = std::path::Path::new("/sys/class/net").join(name);
            let info = self
                .statics
                .entry(name.clone())
                .or_insert_with(|| StaticInterfaceInfo {
                    kind: classify_interface(name, &path),
                    is_loopback: name == "lo",
                    mac_address: sysfs::read_string(path.join("address"))
                        .filter(|mac| mac != "00:00:00:00:00:00"),
                })
                .clone();

            let (is_up, link_speed_bps) = *self.links.entry(name.clone()).or_insert_with(|| {
                (
                    sysfs::read_string(path.join("operstate")).as_deref() == Some("up"),
                    // sysfs reports megabits per second; `-1` means "unknown".
                    sysfs::read_f64(path.join("speed"))
                        .filter(|speed| *speed > 0.0)
                        .map(|mbps| mbps * 1e6),
                )
            });

            let before = self.previous.get(name).copied();
            let rate = |now_value: u64, before_value: u64| match before {
                Some(_) if elapsed > 0.0 && now_value >= before_value => {
                    (now_value - before_value) as f64 / elapsed
                }
                _ => 0.0,
            };

            interfaces.push(NetworkMetrics {
                name: name.clone(),
                kind: info.kind.to_owned(),
                is_loopback: info.is_loopback,
                is_up,
                mac_address: info.mac_address,
                link_speed_bps,
                rx_bytes_per_sec: rate(now.rx_bytes, before.map_or(0, |b| b.rx_bytes)),
                tx_bytes_per_sec: rate(now.tx_bytes, before.map_or(0, |b| b.tx_bytes)),
                rx_packets_per_sec: rate(now.rx_packets, before.map_or(0, |b| b.rx_packets)),
                tx_packets_per_sec: rate(now.tx_packets, before.map_or(0, |b| b.tx_packets)),
                total_rx_bytes: now.rx_bytes,
                total_tx_bytes: now.tx_bytes,
                rx_errors: now.rx_errors,
                tx_errors: now.tx_errors,
                rx_dropped: now.rx_dropped,
                tx_dropped: now.tx_dropped,
            });
        }

        self.previous = counters;
        // Busiest real interfaces first; loopback last.
        interfaces.sort_by(|a, b| {
            a.is_loopback.cmp(&b.is_loopback).then_with(|| {
                (b.rx_bytes_per_sec + b.tx_bytes_per_sec)
                    .total_cmp(&(a.rx_bytes_per_sec + a.tx_bytes_per_sec))
            })
        });
        interfaces
    }
}

/// Read every interface's counters from `/proc/net/dev`.
///
/// One file read replaces eight sysfs reads per interface, which matters on
/// machines with a lot of container bridges.
fn read_proc_net_dev() -> HashMap<String, NetCounters> {
    let mut map = HashMap::new();
    let Some(text) = sysfs::read_string("/proc/net/dev") else { return map };

    for line in text.lines().skip(2) {
        let Some((name, values)) = line.split_once(':') else { continue };
        let fields: Vec<u64> = values.split_whitespace().map(|v| v.parse().unwrap_or(0)).collect();
        if fields.len() < 16 {
            continue;
        }
        map.insert(
            name.trim().to_owned(),
            NetCounters {
                rx_bytes: fields[0],
                rx_packets: fields[1],
                rx_errors: fields[2],
                rx_dropped: fields[3],
                tx_bytes: fields[8],
                tx_packets: fields[9],
                tx_errors: fields[10],
                tx_dropped: fields[11],
            },
        );
    }

    map
}

fn classify_interface(name: &str, path: &Path) -> &'static str {
    if name == "lo" {
        return "Loopback";
    }
    if path.join("wireless").exists() || path.join("phy80211").exists() {
        return "Wi-Fi";
    }
    if path.join("device").exists() {
        return "Wired";
    }
    if name.starts_with("docker") || name.starts_with("br-") || name.starts_with("veth") {
        return "Container bridge";
    }
    if name.starts_with("tun") || name.starts_with("wg") {
        return "VPN tunnel";
    }
    "Virtual"
}
