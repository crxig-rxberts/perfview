<div align="center">

# perfview

**Live Linux performance, in plain English.**

[![Release](https://img.shields.io/github/v/release/crxig-rxberts/perfview.svg?style=flat-square&label=release)](https://github.com/crxig-rxberts/perfview/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/crxig-rxberts/perfview/total.svg?style=flat-square)](https://github.com/crxig-rxberts/perfview/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/crxig-rxberts/perfview/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/crxig-rxberts/perfview/actions/workflows/ci.yml)
[![Security checks](https://img.shields.io/github/actions/workflow/status/crxig-rxberts/perfview/security-analysis.yml?branch=main&style=flat-square&label=security)](https://github.com/crxig-rxberts/perfview/actions/workflows/security-analysis.yml)

[![Rust](https://img.shields.io/badge/Rust-1.95%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux-lightgrey.svg?style=flat-square&logo=linux)](#requirements)
[![Architecture](https://img.shields.io/badge/arch-x86--64%20%7C%20ARM64-blue.svg?style=flat-square)](#requirements)
[![Licence](https://img.shields.io/badge/licence-MIT-blue.svg?style=flat-square)](LICENSE)

[Install](#installation) · [Explore the pages](#pages) · [Screenshots](#screenshots) · [Contribute](#contributing)

<br />

<img src="docs/screenshots/01-overview.png" alt="perfview Overview: live system load, component summaries, drive and network activity, and busiest processes" width="100%" />

<strong>No telemetry · No network access · No root required</strong>

</div>

---

perfview is a graphical system performance viewer for Linux, covering
**processor, graphics, memory, storage and network activity** across five live
pages. Readings use full-sentence labels, familiar units and explanatory hover
hints. Unreadable sensors are reported as **unavailable**, never as zero.

Built with [egui](https://github.com/emilk/egui), it ships as a single binary
that depends only on libc.

## Contents

[Features](#features) · [Installation](#installation) · [Usage](#usage) ·
[Screenshots](#screenshots) · [Pages](#pages) · [Requirements](#requirements) ·
[Performance](#performance) · [Project layout](#project-layout) ·
[Troubleshooting](#troubleshooting) · [Contributing](#contributing) · [Licence](#licence)

## Features

| Area | What you get |
| --- | --- |
| **Five live pages** | Overview, Graphics, Processor, Memory and Storage, with network activity on Overview. |
| **Charts** | Area charts with 30-second to 15-minute history; hover for a value and its age. |
| **Graphics** | NVIDIA telemetry through NVML: per-process video memory, PCIe throughput, encoder/decoder load, clock domains, power limits and driver throttle reasons. AMD and Intel support through `amdgpu` and `i915` sysfs. |
| **Processor** | Per-thread load split into user, kernel and I/O wait time. |
| **Memory** | Programs, file cache, shared memory, disk buffers, kernel structures and free memory. |
| **Storage** | Per-drive throughput, IOPS, utilisation, queue depth, latency and temperature; capacity for each mounted filesystem. |
| **Processes** | Tables ranked by processor, memory or disk activity. |
| **Interface** | Dark and light themes, adjustable scale and a freeze control. |
| **Low overhead** | Background sampling; the window repaints only when new data arrives. |
| **Privacy and safety** | No network access, telemetry or root requirement. Crate-level `unsafe` code is forbidden. |

## Installation

### Cargo

With Rust and the [build dependencies](#building) installed:

```bash
cargo install perfview --locked
perfview
```

### Release builds

Download a **Debian package** or **portable tarball** from the
[releases page](https://github.com/crxig-rxberts/perfview/releases/latest).
Tarballs are available for x86-64 and ARM64; the Debian package is x86-64.
Each asset includes a `.sha256` checksum.

<details>
<summary><strong>Install a release from the terminal (x86-64)</strong></summary>

These examples use v0.1.2. Match the filenames to the latest release.

```bash
# Debian, Ubuntu and derivatives
curl -fLO https://github.com/crxig-rxberts/perfview/releases/download/v0.1.2/perfview_0.1.2_amd64.deb
sudo apt install ./perfview_0.1.2_amd64.deb

# Portable tarball
curl -fLO https://github.com/crxig-rxberts/perfview/releases/download/v0.1.2/perfview-0.1.2-x86_64-unknown-linux-gnu.tar.gz
tar xzf perfview-0.1.2-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 755 perfview-0.1.2-x86_64-unknown-linux-gnu/perfview /usr/local/bin/
```

</details>

### From source

```bash
git clone https://github.com/crxig-rxberts/perfview.git
cd perfview
cargo build --release
./target/release/perfview
```

Build dependencies are listed under [Requirements](#requirements).

## Usage

```bash
perfview
```

### Keyboard

| Key | Action |
| --- | --- |
| <kbd>1</kbd> – <kbd>5</kbd> | Switch to Overview, Graphics, Processor, Memory or Storage |
| <kbd>Space</kbd> | Freeze or resume sampling |

### Command line

```text
Usage: perfview [OPTIONS]

Options:
      --page <PAGE>             Page to open on startup [default: overview]
                                [possible values: overview, gpu, cpu, memory, storage]
      --refresh <MILLISECONDS>  Milliseconds between readings [default: 1000]
      --history <SECONDS>       Seconds of history kept in every graph [default: 60]
      --theme <THEME>           Colour theme [default: midnight]
                                [possible values: midnight, daylight]
      --scale <FACTOR>          Interface scale, where 1.0 is 100% [default: 1]
      --size <WxH>              Window size on startup, e.g. 1600x1000
      --fresh                   Ignore saved preferences and use these values
  -h, --help                    Print help
  -V, --version                 Print version
```

Examples:

```bash
# Four readings a second, opening on the Graphics page
perfview --fresh --page gpu --refresh 250 --size 1600x1000

# Long-term watching with a 15-minute chart window
perfview --fresh --refresh 5000 --history 900

# Light theme at 125% scale
perfview --fresh --theme daylight --scale 1.25
```

Settings changed in the application are saved and restored on the next run.
`--fresh` ignores them.

## Screenshots

Expand a page to preview it. The Overview is also shown at the top of this README.

<details>
<summary><strong>Overview</strong> — system summary, network activity and busiest processes</summary>

Headline figures for each subsystem, a combined load chart, per-component
summaries, drive and network activity, and the busiest processes.

<img src="docs/screenshots/01-overview.png" alt="Overview page: system summary, charts and busiest processes" width="100%" />

</details>

<details>
<summary><strong>Graphics</strong> — load, video memory, thermals and power</summary>

Load, video memory, clock domains, thermals, power, PCIe link state and
throughput, the driver's throttle reasons, and the processes holding video
memory.

<img src="docs/screenshots/02-graphics.png" alt="Graphics page: GPU load, video memory, thermals, power and PCIe telemetry" width="100%" />

</details>

<details>
<summary><strong>Processor</strong> — per-thread load, clocks and temperatures</summary>

Load broken down by the kind of work, a tile per logical processor, clock
speeds, temperature sensors, cache sizes and scheduler counters.

<img src="docs/screenshots/03-processor.png" alt="Processor page: per-thread load, clock speeds and temperature sensors" width="100%" />

</details>

<details>
<summary><strong>Memory</strong> — memory composition, swap and installed modules</summary>

Composition of physical memory, swap usage and activity, page fault and
writeback rates, commit accounting, and installed memory modules.

<img src="docs/screenshots/04-memory.png" alt="Memory page: physical memory composition, swap activity and installed modules" width="100%" />

</details>

<details>
<summary><strong>Storage</strong> — drive activity, latency and filesystem capacity</summary>

Combined and per-drive throughput, utilisation, queue depth, latency, drive
details and filesystem capacity.

<img src="docs/screenshots/05-storage.png" alt="Storage page: drive throughput, utilisation, latency and filesystem capacity" width="100%" />

</details>

## Pages

Detailed readings by page. Sensor availability depends on your hardware,
drivers and permissions; see [Requirements](#requirements).

<details>
<summary><strong>Overview</strong></summary>

- Headline figures for processor, graphics, memory and storage
- Combined load chart for processor, graphics and memory
- Per-component summary cards with sparklines
- Drive throughput and per-filesystem free space
- Network throughput, link speeds and per-interface totals
- Processes ranked by processor time

</details>

<details>
<summary><strong>Graphics</strong></summary>

- Graphics load, memory bus load, encoder and decoder load
- Video memory used, free and total
- BAR1 window size and usage
- Core, shader, memory and video engine clocks against their maxima
- Core, hotspot and memory temperatures, and the slowdown threshold
- Fan speed as a percentage and in RPM
- Board power against the enforced limit
- PCIe generation, width, theoretical bandwidth and live throughput in both
  directions, flagged when the link is below its maximum
- Throttle reasons reported by the driver, written out in full
- Driver performance state
- Processes holding video memory

</details>

<details>
<summary><strong>Processor</strong></summary>

- Load split into user, kernel and I/O wait time
- A tile per logical processor, with per-thread clock speed on hover
- Average, peak and maximum clock speeds
- Every readable package temperature sensor, with vendor labels translated
  (`Tctl` is shown as "Package (control)", `Tccd1` as "Core cluster 1")
- Package power draw where the sensor is readable
- Core and thread counts, SMT status, instruction set and cache sizes
- Load average compared against the thread count
- Context switches, interrupts and process creations per second
- Running and I/O-blocked task counts
- Scaling governor and driver

</details>

<details>
<summary><strong>Memory</strong></summary>

- Composition bar covering programs, file cache, shared memory, disk buffers,
  kernel structures and free memory
- Used against available, and the difference between them
- Swap usage, swap-in and swap-out rates, and compressed swap where configured
- Major page fault rate
- Page-in and page-out rates, dirty pages and writeback
- Committed memory against the kernel's commit limit
- Huge pages where configured
- Installed memory modules: slot, size, type and speed
- Processes ranked by resident memory

</details>

<details>
<summary><strong>Storage</strong></summary>

- Combined read and write throughput, with peaks
- Every mounted filesystem with a capacity bar, flagged above 90% full
- Per drive: throughput, IOPS, utilisation and queue depth
- Average read and write service time
- Drive temperature
- Model, firmware, capacity in decimal and binary units, and block size
- The active I/O scheduler
- Processes ranked by disk throughput

</details>

## Requirements

### Running

| Requirement | Supported |
| --- | --- |
| Operating system | Linux |
| Architecture | x86-64 or ARM64 |
| Display | X11 or Wayland with OpenGL 3.3 or newer |

No root access is required. Two readings are root-only on most distributions —
the processor's RAPL energy counter and the per-module memory details from DMI —
and are reported as unavailable when running as a normal user.

NVIDIA telemetry requires the proprietary driver. NVML is loaded at runtime, so
the same binary runs on machines without it and falls back to the kernel's
generic interface. AMD and Intel graphics work without additional drivers.

### Building

```bash
# Debian, Ubuntu and derivatives
sudo apt install build-essential libgtk-3-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev

# Fedora
sudo dnf install gcc gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel

# Arch
sudo pacman -S base-devel gtk3 libxcb libxkbcommon openssl
```

Minimum supported Rust version is 1.95, checked in CI.

## Performance

Sampling cost is measured by an example in the repository:

```text
$ cargo run --release --example profile

Collector cost per sample, averaged over 4 rounds

  Processor         0.74 ms
  Memory            0.11 ms
  Filesystems       0.00 ms
  Drives            0.24 ms
  Network           0.23 ms
  Graphics          7.70 ms
  Processes        20.60 ms   (runs at most once a second)
```

Measured on a Ryzen 7 5800X with an RTX 5070 Ti and roughly 230 running
processes. The live figure is also shown in the bottom-left corner of the
application.

<details>
<summary><strong>How sampling overhead is kept low</strong></summary>

- Sampling runs on a background thread, so a slow sysfs read cannot delay a
  frame.
- `nvmlDeviceGetPcieThroughput` blocks for around 20 ms per direction while the
  driver samples the bus. It runs on a separate thread and the main sampler uses
  the most recent value.
- Per-device static values such as the card name, VBIOS version and maximum
  clocks are read once.
- Process enumeration passes `without_tasks` to sysinfo, which otherwise walks
  every thread of every process.
- Thread counts are parsed from `/proc/<pid>/stat` rather than scanning
  `/proc/<pid>/task`.
- Network counters are read from `/proc/net/dev` in a single read rather than
  eight sysfs reads per interface.
- Mount points and free space are refreshed every three seconds rather than
  every sample.
- The window repaints only when a new sample arrives.

</details>

## Project layout

Raw data collection and presentation are kept separate: `metrics` handles
readings; `ui` handles formatting and display.

<details>
<summary><strong>Source tree and architecture</strong></summary>

```text
src/
├── main.rs              Command line, window setup, generated application icon
├── app.rs               Window chrome, navigation, settings, frame loop
├── metrics/             Data collection. No knowledge of the UI.
│   ├── collector.rs     Background sampling thread
│   ├── cpu.rs           /proc/stat, cpufreq, hwmon, RAPL
│   ├── memory.rs        /proc/meminfo, /proc/vmstat, DMI
│   ├── gpu.rs           NVML, with an amdgpu/i915 sysfs fallback
│   ├── storage.rs       /proc/diskstats, /sys/block, mount points
│   ├── network.rs       /proc/net/dev, /sys/class/net
│   ├── process.rs       Per-process figures via sysinfo
│   ├── history.rs       Rolling buffers behind every chart
│   └── sysfs.rs         Helpers for reading /proc and /sys
└── ui/                  Presentation. No knowledge of sysfs.
    ├── format.rs        Unit conversion and wording
    ├── theme.rs         Palette, spacing scale and egui style
    ├── widgets.rs       Gauges, charts, bars, the per-thread grid
    └── tabs/            One module per page
```

`metrics` stores values in raw units — bytes, hertz, watts, degrees Celsius —
and performs no formatting. `ui` reads no files. All unit conversion and wording
lives in `src/ui/format.rs`.

Built with [egui](https://github.com/emilk/egui) and
[eframe](https://github.com/emilk/egui/tree/master/crates/eframe),
[sysinfo](https://github.com/GuillaumeGomez/sysinfo) and
[nvml-wrapper](https://github.com/Cldfire/nvml-wrapper).

</details>

## Troubleshooting

<details>
<summary>The Graphics page reports no readable card</summary>

The notice at the top of the page states the reason. Common causes:

- No NVIDIA driver installed. perfview falls back to the kernel interface, which
  covers AMD and Intel.
- Running in a container or sandbox without `/sys` or the NVIDIA device nodes.
- Running as a different user from the one that owns the display.

</details>

<details>
<summary>Processor power draw is unavailable</summary>

`/sys/class/powercap/.../energy_uj` is root-only on most distributions. perfview
does not request elevated privileges, so the reading is reported as unavailable.

To grant access:

```bash
sudo chmod +r /sys/class/powercap/intel-rapl:0/energy_uj
```

On AMD systems the `zenpower` module exposes package power through `hwmon`,
which perfview reads automatically.

</details>

<details>
<summary>Memory modules are not listed</summary>

Module details come from DMI at `/sys/firmware/dmi/entries`, which is root-only.
The rest of the Memory page does not depend on it.

</details>

<details>
<summary>Temperatures are missing</summary>

Load the sensor module for the processor:

```bash
sudo modprobe k10temp    # AMD
sudo modprobe coretemp   # Intel
```

</details>

<details>
<summary>The window does not open</summary>

perfview requires OpenGL 3.3.

```bash
glxinfo | grep "OpenGL version"
RUST_LOG=perfview=debug,eframe=debug perfview
```

</details>

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build setup, project conventions and
how to add a new reading. Bug reports are useful even without a fix, since
perfview can only be tested against hardware its contributors own.

See also the [Code of Conduct](CODE_OF_CONDUCT.md) and the
[Security policy](SECURITY.md).

## Licence

[MIT](LICENSE) © Craig Roberts
