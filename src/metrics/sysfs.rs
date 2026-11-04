//! Small helpers for poking around `/proc` and `/sys`.
//!
//! These paths are all plain text and extremely cheap to read, but they are
//! also allowed to vanish depending on kernel config, driver and permissions.
//! Every helper therefore returns an `Option` rather than erroring out, so a
//! missing sensor degrades into a greyed-out "Not available" in the UI instead
//! of taking the whole app down.

use std::{fs, path::Path};

/// Read a file and trim it. Returns `None` if the file is missing or unreadable.
pub(crate) fn read_string(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

/// Read a file containing a single integer.
pub(crate) fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_string(path)?.parse().ok()
}

/// Read a file containing a single float.
pub(crate) fn read_f64(path: impl AsRef<Path>) -> Option<f64> {
    read_string(path)?.parse().ok()
}

/// List the entries of a directory, sorted by name for stable UI ordering.
pub(crate) fn list_dir(path: impl AsRef<Path>) -> Vec<std::path::PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    paths
}

/// Parse `/proc/meminfo`-style `key: value kB` lines into a lookup closure.
pub(crate) fn parse_key_kb(contents: &str, key: &str) -> Option<u64> {
    for line in contents.lines() {
        let Some((name, value)) = line.split_once(':') else { continue };
        if name.trim() != key {
            continue;
        }
        let number = value.split_whitespace().next()?;
        // `/proc/meminfo` reports kibibytes; normalise to bytes immediately.
        return number.parse::<u64>().ok().map(|kb| kb * 1024);
    }
    None
}

/// Find the `hwmon` directory belonging to a device, e.g. an NVMe controller.
pub(crate) fn hwmon_for_device(device_dir: &Path) -> Option<std::path::PathBuf> {
    list_dir(device_dir.join("hwmon")).into_iter().next()
}

/// Read a `hwmon` temperature input (millidegrees Celsius) as degrees Celsius.
pub(crate) fn hwmon_temp_c(hwmon: &Path, index: u32) -> Option<f64> {
    read_f64(hwmon.join(format!("temp{index}_input"))).map(|milli| milli / 1000.0)
}

/// Read the human-friendly label for a `hwmon` temperature input.
pub(crate) fn hwmon_temp_label(hwmon: &Path, index: u32) -> Option<String> {
    read_string(hwmon.join(format!("temp{index}_label")))
}
