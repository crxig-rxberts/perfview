//! Machine identity and uptime: the "what am I even looking at" header.

use super::sysfs;

#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub hostname: String,
    /// Pretty distribution name, e.g. `Ubuntu 25.10`.
    pub distribution: String,
    pub kernel_version: String,
    /// Desktop environment, e.g. `GNOME`.
    pub desktop: Option<String>,
    /// `Wayland` or `X11`.
    pub display_server: Option<String>,
    /// Motherboard model, handy when sharing screenshots for support.
    pub motherboard: Option<String>,
    pub bios_version: Option<String>,
    pub uptime_secs: u64,
    /// Local time the machine was last booted.
    pub boot_time: Option<chrono::DateTime<chrono::Local>>,
}

/// Static facts are read once at startup; only uptime changes per sample.
#[derive(Debug, Clone)]
pub struct SystemSampler {
    base: SystemMetrics,
}

impl SystemSampler {
    pub fn new() -> Self {
        Self {
            base: SystemMetrics {
                hostname: sysinfo::System::host_name()
                    .unwrap_or_else(|_| "Unknown host".to_owned()),
                distribution: sysinfo::System::long_os_version()
                    .unwrap_or_else(|_| "Linux".to_owned()),
                kernel_version: sysinfo::System::kernel_version()
                    .unwrap_or_else(|_| "Unknown kernel".to_owned()),
                desktop: std::env::var("XDG_CURRENT_DESKTOP").ok().filter(|v| !v.is_empty()),
                display_server: std::env::var("XDG_SESSION_TYPE").ok().map(|raw| {
                    match raw.as_str() {
                        "wayland" => "Wayland".to_owned(),
                        "x11" => "X11".to_owned(),
                        other => other.to_owned(),
                    }
                }),
                motherboard: board_name(),
                bios_version: sysfs::read_string("/sys/class/dmi/id/bios_version"),
                uptime_secs: 0,
                boot_time: None,
            },
        }
    }

    pub fn sample(&self) -> SystemMetrics {
        let uptime = sysinfo::System::uptime().unwrap_or(0);
        SystemMetrics {
            uptime_secs: uptime,
            boot_time: chrono::Local::now()
                .checked_sub_signed(chrono::Duration::seconds(uptime.cast_signed())),
            ..self.base.clone()
        }
    }
}

impl Default for SystemSampler {
    fn default() -> Self {
        Self::new()
    }
}

fn board_name() -> Option<String> {
    let vendor = sysfs::read_string("/sys/class/dmi/id/board_vendor");
    let name = sysfs::read_string("/sys/class/dmi/id/board_name")?;
    Some(match vendor {
        Some(vendor) if !name.starts_with(&vendor) => format!("{vendor} {name}"),
        _ => name,
    })
}
