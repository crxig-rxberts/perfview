# Contributing to perfview

Thanks for considering a contribution. This document covers how to get set up,
the conventions the project follows, and how to get a change merged.

## Presentation conventions

Readings in perfview are written out in full and converted to familiar units.
When adding or changing anything the user sees:

- **Use descriptive labels, not counter names.** `pgmajfault` is displayed as
  "Memory reads that hit the disk".
- **Expand abbreviations**, and mention the common term in the hover hint so it
  is still recognisable. "Requests per second", with IOPS in the hint.
- **Convert units.** `3847000000` is displayed as `3.85 GHz`.
- **Report missing sensors as unavailable**, never as zero, and say why where
  the reason is known.
- **Add a verdict where it is useful.** `84 °C` is displayed as
  `84 °C · Warm`.

All formatting lives in [`src/ui/format.rs`](src/ui/format.rs). If you are
formatting a number anywhere else, it probably belongs there.

## Getting set up

You will need a recent stable Rust toolchain (1.95 or newer) and the usual
Linux GUI development packages.

```bash
# Debian, Ubuntu and derivatives
sudo apt install build-essential libgtk-3-dev libxcb-render0-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev

# Fedora
sudo dnf install gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel

# Arch
sudo pacman -S gtk3 libxcb libxkbcommon openssl
```

Then:

```bash
git clone https://github.com/crxig-rxberts/perfview.git
cd perfview
cargo run
```

Install the commit hooks once, so formatting and commit messages are checked
before anything leaves your machine:

```bash
prek install
prek install --hook-type commit-msg
```

## Before you open a pull request

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

All three must be clean. CI runs exactly the same commands, plus a build on the
minimum supported Rust version.

## How the code is laid out

```text
src/
├── main.rs              Command line, window setup, the generated app icon
├── app.rs               Window chrome, navigation, settings, the frame loop
├── metrics/             Gathering raw numbers. Knows nothing about the UI.
│   ├── collector.rs     The background sampling thread
│   ├── cpu.rs           /proc/stat, cpufreq, hwmon, RAPL
│   ├── memory.rs        /proc/meminfo, /proc/vmstat, DMI
│   ├── gpu.rs           NVML, with an amdgpu/i915 sysfs fallback
│   ├── storage.rs       /proc/diskstats, /sys/block, mount points
│   ├── network.rs       /proc/net/dev, /sys/class/net
│   ├── process.rs       Per-process figures, via sysinfo
│   ├── history.rs       The rolling buffers behind every graph
│   └── sysfs.rs         Small helpers for reading /proc and /sys safely
└── ui/                  Presenting those numbers. Knows nothing about sysfs.
    ├── format.rs        Every unit conversion and plain-English verdict
    ├── theme.rs         The palette, spacing scale and egui style
    ├── widgets.rs       Gauges, graphs, bars, the per-thread heat grid
    └── tabs/            One module per page
```

The split between `metrics` and `ui` is deliberate and worth preserving.
`metrics` stores everything in raw SI-ish units (bytes, hertz, watts, degrees
Celsius) and never formats anything. `ui` never reads a file.

## Adding a new reading

1. Add the field to the relevant struct in `src/metrics/`, in raw units, with a
   doc comment explaining what it actually measures.
2. Read it in that module's sampler. Use `Option` for anything that might not
   exist — a missing sensor must never become a zero.
3. If it should be graphed, record it in `History::ingest`.
4. Add a formatter to `src/ui/format.rs` if the unit is new, with a unit test.
5. Display it with `widgets::metric_row`, giving it a descriptive label and a
   `hint` explaining what it measures and when it matters.

## Performance

Sampling cost matters, since perfview runs alongside whatever is being
measured. Check any new collector against the profiling example.

```bash
cargo run --release --example profile
```

Anything that blocks for more than a few milliseconds — NVML's PCIe throughput
call being the clearest example — belongs on its own schedule or its own
thread, not in the main sample.

## Supporting more hardware

perfview can only be tested against the hardware its contributors own. If a
sensor is unreadable on your machine, a bug report with the output of the
commands in the issue template is useful, even without a fix.

When adding support for a new vendor or driver:

- Keep it behind a capability check, so it degrades gracefully elsewhere.
- Never panic on a missing or malformed file. `src/metrics/sysfs.rs` returns
  `Option` everywhere for exactly this reason.
- Add the vendor's codenames to the translation tables, so `Tccd1` shows as
  "Core cluster 1" rather than leaking through to the user.

## Screenshots

Screenshots in the README are generated, not hand-captured:

```bash
./scripts/capture-screenshots.sh
```

It requires Hyprland, `grim` and `jq`. It applies a synthetic load so the
charts contain data, moves the window to an empty workspace, makes it
fullscreen so no desktop panel is captured, and writes one image per page.

## Commit messages

perfview uses [Conventional Commits](https://www.conventionalcommits.org).
The commit-msg hook enforces this, and the release notes are generated from it.

```text
feat(gpu): report video memory temperature on supported cards
fix(storage): ignore tiny vendor welcome-disk partitions
perf(process): read thread counts from /proc/pid/stat instead of a readdir
docs(readme): explain why cached memory counts as free
```

Merging to `main` bumps the version, writes the changelog, tags the release and
builds the binaries.

## Release credentials

Release automation follows the same PAT-based release-it setup as
`crxig-rxberts/code-standards` and `crxig-rxberts/renovate-config`.
Configure these repository secrets before merging:

- `RELEASE_IT_GITHUB_TOKEN`: a PAT with repository contents read/write access
  and permission to update workflow files. Unlike `GITHUB_TOKEN`, its tag
  pushes trigger the publishing workflow.
- `GIT_EMAIL` and `GIT_USERNAME`: the release commit identity.
- `CARGO_REGISTRY_TOKEN`: a crates.io token with permission to publish perfview.

The release hook updates only perfview's version in `Cargo.lock` using Node.js,
because the release-it container does not include Rust. Dependency versions and
checksums remain unchanged.

Publishing builds run on native x86-64 and ARM64 runners. Pull requests that
change packaging and manual workflow runs verify packages without creating a
release or uploading to crates.io; only version tags publish.

## Code of conduct

By taking part you agree to abide by the
[Code of Conduct](CODE_OF_CONDUCT.md).
