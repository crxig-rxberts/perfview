//! Every number the user sees passes through this module.
//!
//! The rule for the whole app: **no raw units, no acronyms.**
//! `3847000000` becomes `3.85 GHz`, `0x1002` becomes `AMD`, and a throttle
//! bitmask becomes a sentence. If a value is missing we say so in words rather
//! than showing a zero that looks like a real reading.

/// What we print when a sensor genuinely is not available.
pub const UNAVAILABLE: &str = "Not available";

/// Bytes as a binary-prefixed size, e.g. `31.3 GiB`.
///
/// Binary prefixes are used for memory because that is how RAM is sold and how
/// the kernel reports it.
pub fn bytes(value: u64) -> String {
    binary_scale(value as f64, &["bytes", "KiB", "MiB", "GiB", "TiB", "PiB"])
}

/// Bytes as a decimal-prefixed size, e.g. `1.0 TB`.
///
/// Decimal prefixes are used for drives because that is what is printed on the
/// box, and it avoids the "my 1 TB drive shows 931 GB" confusion.
pub fn bytes_decimal(value: u64) -> String {
    let units = ["bytes", "KB", "MB", "GB", "TB", "PB"];
    let mut value = value as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format_scaled(value, units[unit])
}

/// A transfer rate, e.g. `412 MB/s`.
pub fn bytes_per_sec(value: f64) -> String {
    if value < 1.0 {
        return "0 B/s".to_owned();
    }
    let units = ["B/s", "KB/s", "MB/s", "GB/s", "TB/s"];
    let mut value = value;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format_scaled(value, units[unit])
}

/// A network rate in bits per second, e.g. `2.5 Gb/s`, matching how ISPs and
/// network adapters quote speeds.
pub fn bits_per_sec(value: f64) -> String {
    let units = ["bit/s", "Kb/s", "Mb/s", "Gb/s", "Tb/s"];
    let mut value = value;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format_scaled(value, units[unit])
}

/// Hertz as `4.85 GHz` / `1 400 MHz`.
pub fn hertz(value: f64) -> String {
    let units = ["Hz", "kHz", "MHz", "GHz"];
    let mut value = value;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format_scaled(value, units[unit])
}

pub fn hertz_opt(value: Option<f64>) -> String {
    value.map_or_else(|| UNAVAILABLE.to_owned(), hertz)
}

/// A percentage with one decimal place, e.g. `43.7%`.
pub fn percent(value: f64) -> String {
    format!("{value:.1}%")
}

/// A percentage with no decimal place, for dense grids.
pub fn percent_whole(value: f64) -> String {
    format!("{:.0}%", value.round())
}

pub fn percent_opt(value: Option<f64>) -> String {
    value.map_or_else(|| UNAVAILABLE.to_owned(), percent)
}

/// Degrees Celsius, e.g. `62 °C`.
pub fn celsius(value: f64) -> String {
    format!("{value:.0} °C")
}

pub fn celsius_opt(value: Option<f64>) -> String {
    value.map_or_else(|| UNAVAILABLE.to_owned(), celsius)
}

/// Watts, e.g. `143 W` or `4.2 W` for small draws.
pub fn watts(value: f64) -> String {
    if value < 10.0 { format!("{value:.1} W") } else { format!("{value:.0} W") }
}

pub fn watts_opt(value: Option<f64>) -> String {
    value.map_or_else(|| UNAVAILABLE.to_owned(), watts)
}

/// Milliseconds with sensible precision, e.g. `0.12 ms`, `18 ms`.
pub fn milliseconds(value: f64) -> String {
    if value < 1.0 {
        format!("{value:.2} ms")
    } else if value < 100.0 {
        format!("{value:.1} ms")
    } else {
        format!("{value:.0} ms")
    }
}

pub fn milliseconds_opt(value: Option<f64>) -> String {
    value.map_or_else(|| UNAVAILABLE.to_owned(), milliseconds)
}

/// Operations per second with thousands separators, e.g. `12 400 /s`.
pub fn per_sec(value: f64) -> String {
    format!("{} /s", count(value.round() as u64))
}

/// A plain count with thin-space thousands separators, e.g. `1 048 576`.
pub fn count(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('\u{202f}'); // Narrow no-break space.
        }
        out.push(ch);
    }
    out
}

/// A duration as `4 days, 7 hours, 12 minutes`.
pub fn duration_long(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(plural(days, "day"));
    }
    if hours > 0 {
        parts.push(plural(hours, "hour"));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(plural(minutes, "minute"));
    }
    parts.join(", ")
}

/// A duration as `04:17:36`, for process run times.
pub fn duration_clock(seconds: u64) -> String {
    format!("{:02}:{:02}:{:02}", seconds / 3600, (seconds % 3600) / 60, seconds % 60)
}

fn plural(value: u64, unit: &str) -> String {
    if value == 1 { format!("1 {unit}") } else { format!("{value} {unit}s") }
}

/// `used of total`, without a percentage. Used where a gauge beside it is
/// already showing the percentage.
pub fn usage_of(used: u64, total: u64) -> String {
    format!("{} of {}", bytes(used), bytes(total))
}

/// `used of total (percent)`, the phrasing used for every capacity bar.
pub fn usage_summary(used: u64, total: u64) -> String {
    let pct = if total == 0 { 0.0 } else { (used as f64 / total as f64) * 100.0 };
    format!("{} of {} ({})", bytes(used), bytes(total), percent_whole(pct))
}

/// PCIe link described the way a review site would, e.g. `PCIe 5.0 x16`.
pub fn pcie_link(generation: Option<u32>, width: Option<u32>) -> String {
    match (generation, width) {
        (Some(generation), Some(width)) => format!("PCIe {generation}.0 x{width}"),
        (Some(generation), None) => format!("PCIe {generation}.0"),
        (None, Some(width)) => format!("x{width}"),
        (None, None) => UNAVAILABLE.to_owned(),
    }
}

/// Theoretical one-way bandwidth of a PCIe link, in bytes per second.
pub fn pcie_bandwidth(generation: u32, width: u32) -> f64 {
    // Effective bytes/s per lane after encoding overhead.
    let per_lane = match generation {
        1 => 250e6,
        2 => 500e6,
        3 => 985e6,
        4 => 1_969e6,
        5 => 3_938e6,
        6 => 7_877e6,
        _ => 0.0,
    };
    per_lane * f64::from(width)
}

/// Describe a load average relative to core count, in plain words.
pub fn load_verdict(load: f64, cores: usize) -> &'static str {
    if cores == 0 {
        return "Unknown";
    }
    let ratio = load / cores as f64;
    match ratio {
        r if r < 0.3 => "Plenty of headroom",
        r if r < 0.7 => "Comfortable",
        r if r < 1.0 => "Working hard",
        r if r < 1.5 => "Fully committed",
        _ => "Oversubscribed - tasks are queuing",
    }
}

/// Describe a temperature in words so the user knows if it is a problem.
pub fn temperature_verdict(celsius: f64, kind: ThermalKind) -> &'static str {
    let (warm, hot) = match kind {
        ThermalKind::Cpu => (75.0, 90.0),
        ThermalKind::Gpu => (75.0, 85.0),
        ThermalKind::Drive => (55.0, 70.0),
    };
    match celsius {
        c if c < warm - 20.0 => "Cool",
        c if c < warm => "Normal",
        c if c < hot => "Warm",
        _ => "Hot - check cooling",
    }
}

/// Which thermal thresholds to judge a temperature against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalKind {
    Cpu,
    Gpu,
    Drive,
}

/// Describe how much free space is left, in words.
pub fn capacity_verdict(used_percent: f64) -> &'static str {
    match used_percent {
        p if p < 70.0 => "Healthy",
        p if p < 85.0 => "Filling up",
        p if p < 95.0 => "Low on space",
        _ => "Critically full",
    }
}

/// Describe memory pressure, accounting for Linux's use of spare RAM as cache.
pub fn memory_verdict(used_percent: f64, swapping: bool) -> &'static str {
    if swapping {
        return "Swapping to disk - close something";
    }
    match used_percent {
        p if p < 60.0 => "Plenty free",
        p if p < 80.0 => "Comfortable",
        p if p < 92.0 => "Getting tight",
        _ => "Nearly full",
    }
}

fn binary_scale(mut value: f64, units: &[&str]) -> String {
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format_scaled(value, units[unit])
}

/// Three significant figures reads better than a fixed decimal count:
/// `9.87 GiB`, `98.7 GiB`, `987 GiB`.
fn format_scaled(value: f64, unit: &str) -> String {
    if unit == "bytes" || unit == "Hz" || unit == "bit/s" {
        format!("{value:.0} {unit}")
    } else if value < 10.0 {
        format!("{value:.2} {unit}")
    } else if value < 100.0 {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_binary_sizes_for_humans() {
        assert_eq!(bytes(512), "512 bytes");
        assert_eq!(bytes(1024), "1.00 KiB");
        assert_eq!(bytes(33_674_000_000), "31.4 GiB");
    }

    #[test]
    fn scales_drive_sizes_the_way_the_box_does() {
        assert_eq!(bytes_decimal(1_000_204_886_016), "1.00 TB");
    }

    #[test]
    fn formats_clock_speeds() {
        assert_eq!(hertz(4_850_000_000.0), "4.85 GHz");
        assert_eq!(hertz(555_000_000.0), "555 MHz");
    }

    #[test]
    fn separates_thousands() {
        assert_eq!(count(1_048_576), "1\u{202f}048\u{202f}576");
        assert_eq!(count(42), "42");
    }

    #[test]
    fn describes_uptime_in_words() {
        assert_eq!(duration_long(0), "0 minutes");
        assert_eq!(duration_long(3_600), "1 hour");
        assert_eq!(duration_long(93_784), "1 day, 2 hours, 3 minutes");
    }

    #[test]
    fn reports_missing_sensors_in_words_not_zeroes() {
        assert_eq!(celsius_opt(None), UNAVAILABLE);
        assert_eq!(watts_opt(None), UNAVAILABLE);
        assert_eq!(hertz_opt(None), UNAVAILABLE);
    }

    #[test]
    fn judges_temperatures() {
        assert_eq!(temperature_verdict(40.0, ThermalKind::Cpu), "Cool");
        assert_eq!(temperature_verdict(95.0, ThermalKind::Cpu), "Hot - check cooling");
        assert_eq!(temperature_verdict(60.0, ThermalKind::Drive), "Warm");
    }

    #[test]
    fn describes_pcie_links() {
        assert_eq!(pcie_link(Some(5), Some(16)), "PCIe 5.0 x16");
        assert_eq!(pcie_link(None, None), UNAVAILABLE);
    }
}
