//! The Graphics page: everything about the graphics card, in plain English.

use egui::{RichText, Ui};

use super::common::{self, Headline};
use crate::{
    app::{CardSpec, TabContext, card, card_row, divider},
    metrics::GpuMetrics,
    ui::{
        format,
        theme::space,
        widgets::{self, GraphLine, GraphUnit},
    },
};

pub fn show(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;

    if ctx.snapshot.gpus.is_empty() {
        widgets::empty_state(
            ui,
            palette,
            "No graphics card telemetry available",
            ctx.gpu_notice.unwrap_or(
                "perfview could not find a graphics card it can read.\n\nNVIDIA cards need the proprietary driver installed. AMD and Intel cards are read through the kernel, which usually works out of the box.",
            ),
        );
        return;
    }

    if let Some(notice) = ctx.gpu_notice {
        widgets::notice(ui, palette, "i", notice, palette.network);
        ui.add_space(space::BASE);
    }

    for gpu in &ctx.snapshot.gpus {
        show_gpu(ui, ctx, gpu);
        ui.add_space(space::BASE);
    }
}

fn show_gpu(ui: &mut Ui, ctx: &TabContext<'_>, gpu: &GpuMetrics) {
    let palette = ctx.palette;
    let key = |suffix: &str| format!("gpu.{}.{suffix}", gpu.index);

    let headlines = vec![
        Headline {
            label: "Graphics load",
            value: gpu.performance_state.clone().unwrap_or_else(|| gpu.name.clone()),
            support: if gpu.is_unconstrained() {
                "Running freely, nothing holding it back".to_owned()
            } else {
                format!("{} limits in effect", gpu.throttle_reasons.len())
            },
            gauge: gpu.utilization_percent.map(|p| (p, "in use")),
            colour: palette.severity(gpu.utilization_percent.unwrap_or(0.0)),
            hint: "The ring shows how much of the time the card has work to do. In a game, close to 100% means the card is the limit.",
        },
        Headline {
            label: "Video memory",
            value: match (gpu.vram_used, gpu.vram_total) {
                (Some(used), Some(total)) => format::usage_of(used, total),
                _ => format::UNAVAILABLE.to_owned(),
            },
            support: gpu
                .vram_free
                .map_or_else(String::new, |free| format!("{} free", format::bytes(free))),
            gauge: gpu.vram_used_percent().map(|p| (p, "filled")),
            colour: palette.severity(gpu.vram_used_percent().unwrap_or(0.0)),
            hint: "Memory on the card itself. Running out causes stutter as textures are shuffled over the PCIe bus.",
        },
        Headline {
            label: "Temperature",
            value: format::celsius_opt(gpu.temperature_c),
            support: gpu.temperature_c.map_or_else(String::new, |c| {
                format!(
                    "{} · slows down at {}",
                    format::temperature_verdict(c, format::ThermalKind::Gpu),
                    format::celsius_opt(gpu.slowdown_temp_c)
                )
            }),
            gauge: None,
            colour: gpu
                .temperature_c
                .map_or(palette.text_muted, |c| palette.thermal(c, format::ThermalKind::Gpu)),
            hint: "Temperature of the graphics processor.",
        },
        Headline {
            label: "Power draw",
            value: format::watts_opt(gpu.power_w),
            support: gpu
                .power_limit_w
                .map_or_else(String::new, |limit| format!("Limit {}", format::watts(limit))),
            gauge: gpu.power_percent().map(|p| (p, "of limit")),
            colour: palette.severity(gpu.power_percent().unwrap_or(0.0)),
            hint: "Board power right now, against the limit the card is allowed to draw.",
        },
    ];
    common::headline_row(ui, palette, &headlines);

    ui.add_space(space::BASE);

    card(
        ui,
        palette,
        &gpu.name,
        &format!(
            "{} · read through {}{}",
            gpu.vendor,
            gpu.source.label(),
            gpu.driver_version.as_ref().map_or(String::new(), |v| format!(" · driver {v}"))
        ),
        |ui| {
            let usage = ctx.history.get_or_empty(&key("usage"));
            let memory_bus = ctx.history.get_or_empty(&key("memory_bus"));
            let vram = ctx.history.get_or_empty(&key("vram_percent"));

            let mut lines = vec![GraphLine::new("Graphics load", &usage, palette.gpu)];
            if !memory_bus.is_empty() {
                lines
                    .push(GraphLine::new("Memory bus busy", &memory_bus, palette.memory).outline());
            }
            if !vram.is_empty() {
                lines.push(GraphLine::new("Video memory used", &vram, palette.network).outline());
            }

            widgets::graph(
                ui,
                palette,
                &format!("gpu_load_{}", gpu.index),
                200.0,
                GraphUnit::Percent,
                &lines,
                ctx.window(),
            );
            ui.add_space(space::SNUG);
            widgets::graph_legend(
                ui,
                palette,
                &[
                    ("Graphics load", palette.gpu, format::percent_opt(gpu.utilization_percent)),
                    (
                        "Memory bus busy",
                        palette.memory,
                        format::percent_opt(gpu.memory_bus_percent),
                    ),
                    (
                        "Video memory used",
                        palette.network,
                        gpu.vram_used_percent()
                            .map_or_else(|| format::UNAVAILABLE.to_owned(), format::percent),
                    ),
                ],
            );
        },
    );

    ui.add_space(space::BASE);

    card_row(
        ui,
        palette,
        &mut [
            CardSpec::new("Heat, fans and power", "How hard the cooling is working", &mut |ui| {
                thermals_and_power(ui, ctx, gpu);
            }),
            CardSpec::new("Clock speeds", "How fast each part of the card is running", &mut |ui| {
                clocks(ui, ctx, gpu);
            }),
        ],
    );

    ui.add_space(space::BASE);

    card_row(
        ui,
        palette,
        &mut [
            CardSpec::new(
                "Video memory and the PCIe link",
                "Where the card keeps data, and how fast it talks to the rest of the machine",
                &mut |ui| {
                    memory_and_bus(ui, ctx, gpu);
                },
            ),
            CardSpec::new(
                "What is limiting this card",
                "If the card is not running flat out, this explains why",
                &mut |ui| {
                    limits_and_identity(ui, ctx, gpu);
                },
            ),
        ],
    );

    if !gpu.processes.is_empty() {
        ui.add_space(space::BASE);
        processes(ui, ctx, gpu);
    }
}

fn thermals_and_power(ui: &mut Ui, ctx: &TabContext<'_>, gpu: &GpuMetrics) {
    let palette = ctx.palette;
    let key = |suffix: &str| format!("gpu.{}.{suffix}", gpu.index);

    let temperature = ctx.history.get_or_empty(&key("temperature"));
    let power = ctx.history.get_or_empty(&key("power"));

    if !temperature.is_empty() {
        widgets::graph(
            ui,
            palette,
            &format!("gpu_temp_{}", gpu.index),
            86.0,
            GraphUnit::Celsius,
            &[GraphLine::new("Core temperature", &temperature, palette.warn)],
            ctx.window(),
        );
        ui.add_space(space::SNUG);
    }

    common::temperature_line(
        ui,
        palette,
        "Core temperature",
        gpu.temperature_c,
        format::ThermalKind::Gpu,
        "Temperature of the graphics processor itself.",
    );
    common::temperature_line(
        ui,
        palette,
        "Hottest point on the die",
        gpu.hotspot_c,
        format::ThermalKind::Gpu,
        "The single hottest spot measured anywhere on the chip. Usually several degrees above the core reading.",
    );
    common::temperature_line(
        ui,
        palette,
        "Video memory temperature",
        gpu.memory_temp_c,
        format::ThermalKind::Gpu,
        "Temperature of the memory chips around the processor.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Slows down at",
        &format::celsius_opt(gpu.slowdown_temp_c),
        "The temperature at which the card starts cutting its own clock speeds to protect itself.",
    );

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Fan speed",
        &match (gpu.fan_percent, gpu.fan_rpm) {
            (Some(percent), Some(rpm)) => {
                format!("{} · {:.0} rpm", format::percent_whole(percent), rpm)
            }
            (Some(percent), None) => format::percent_whole(percent),
            (None, Some(rpm)) => format!("{rpm:.0} rpm"),
            (None, None) => format::UNAVAILABLE.to_owned(),
        },
        "How fast the card's own fans are spinning, as a share of their maximum.",
    );

    if !power.is_empty() {
        widgets::graph(
            ui,
            palette,
            &format!("gpu_power_{}", gpu.index),
            72.0,
            GraphUnit::Watts,
            &[GraphLine::new("Board power", &power, palette.storage)],
            ctx.window(),
        );
        ui.add_space(space::TIGHT);
    }
    widgets::metric_row(
        ui,
        palette,
        "Power right now",
        &format::watts_opt(gpu.power_w),
        "Total power the whole graphics board is drawing.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Power limit in force",
        &format::watts_opt(gpu.power_limit_w),
        "The card will not exceed this. Raising it in your driver settings can unlock extra performance.",
    );
    if let Some(percent) = gpu.power_percent() {
        widgets::bar(ui, palette, percent / 100.0, palette.severity(percent), 5.0);
    }
}

fn clocks(ui: &mut Ui, ctx: &TabContext<'_>, gpu: &GpuMetrics) {
    let palette = ctx.palette;
    let key = |suffix: &str| format!("gpu.{}.{suffix}", gpu.index);

    let clock = ctx.history.get_or_empty(&key("clock"));
    if !clock.is_empty() {
        widgets::graph(
            ui,
            palette,
            &format!("gpu_clock_{}", gpu.index),
            86.0,
            GraphUnit::Megahertz,
            &[GraphLine::new("Graphics core", &clock, palette.gpu)],
            ctx.window(),
        );
        ui.add_space(space::SNUG);
    }

    for domain in &gpu.clocks {
        let headroom = match (domain.current_hz, domain.max_hz) {
            (Some(current), Some(max)) if max > 0.0 => {
                format!(" · {} of maximum", format::percent_whole(current / max * 100.0))
            }
            _ => String::new(),
        };
        widgets::metric_row(
            ui,
            palette,
            &domain.label,
            &format!("{}{headroom}", format::hertz_opt(domain.current_hz)),
            &match domain.max_hz {
                Some(max) => format!("This card can reach {} here.", format::hertz(max)),
                None => String::new(),
            },
        );
        if let (Some(current), Some(max)) = (domain.current_hz, domain.max_hz)
            && max > 0.0
        {
            widgets::bar(ui, palette, current / max, palette.gpu, 4.0);
            ui.add_space(space::TIGHT);
        }
    }

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Video encoder",
        &format::percent_opt(gpu.encoder_percent),
        "Used by streaming and recording software such as OBS. Separate from the main graphics engine.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Video decoder",
        &format::percent_opt(gpu.decoder_percent),
        "Used when playing back video. Also separate from the main graphics engine.",
    );
}

fn memory_and_bus(ui: &mut Ui, ctx: &TabContext<'_>, gpu: &GpuMetrics) {
    let palette = ctx.palette;
    let key = |suffix: &str| format!("gpu.{}.{suffix}", gpu.index);

    if let (Some(used), Some(total)) = (gpu.vram_used, gpu.vram_total) {
        widgets::stacked_bar(
            ui,
            palette,
            total,
            &[
                widgets::Segment {
                    label: "In use".to_owned(),
                    bytes: used,
                    colour: palette.gpu,
                    hint: "Textures, geometry, shaders and frame buffers currently loaded."
                        .to_owned(),
                },
                widgets::Segment {
                    label: "Free".to_owned(),
                    bytes: total.saturating_sub(used),
                    colour: palette.fill(palette.text_faint, 70),
                    hint:
                        "Room left for more game assets before the card starts swapping over PCIe."
                            .to_owned(),
                },
            ],
            14.0,
        );
        ui.add_space(space::SNUG);
    }

    widgets::metric_row(
        ui,
        palette,
        "Video memory fitted",
        &gpu.vram_total.map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes),
        "Total memory on the graphics card.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Free video memory",
        &gpu.vram_free.map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes),
        "",
    );
    if let (Some(used), Some(total)) = (gpu.bar1_used, gpu.bar1_total) {
        widgets::metric_row(
            ui,
            palette,
            "Window into video memory",
            &format::usage_summary(used, total),
            "The region of video memory your processor can reach directly. Known as the BAR1 or Resizable BAR window.",
        );
    }

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Connection",
        &format::pcie_link(gpu.pcie_generation, gpu.pcie_width),
        "The slot link between the card and the rest of the machine.",
    );
    if let (Some(generation), Some(width)) = (gpu.pcie_max_generation, gpu.pcie_max_width) {
        let running_slow = gpu.pcie_generation.is_some_and(|g| g < generation)
            || gpu.pcie_width.is_some_and(|w| w < width);
        widgets::metric_row_coloured(
            ui,
            palette,
            "Best this card supports",
            &format::pcie_link(Some(generation), Some(width)),
            if running_slow { palette.warn } else { palette.good },
            if running_slow {
                "The card is currently linked below its maximum. Cards drop down when idle to save power, so check this under load."
            } else {
                "The card is linked at its full speed."
            },
        );
        widgets::metric_row(
            ui,
            palette,
            "Theoretical link speed",
            &format::bytes_per_sec(format::pcie_bandwidth(generation, width)),
            "How much data the slot can carry in one direction per second.",
        );
    }

    let tx = ctx.history.get_or_empty(&key("pcie_tx"));
    let rx = ctx.history.get_or_empty(&key("pcie_rx"));
    if !tx.is_empty() || !rx.is_empty() {
        ui.add_space(space::SNUG);
        widgets::graph(
            ui,
            palette,
            &format!("gpu_pcie_{}", gpu.index),
            80.0,
            GraphUnit::BytesPerSec,
            &[
                GraphLine::new("To the card", &tx, palette.network),
                GraphLine::new("From the card", &rx, palette.alternate).outline(),
            ],
            ctx.window(),
        );
        ui.add_space(space::SNUG);
        widgets::graph_legend(
            ui,
            palette,
            &[
                (
                    "To the card",
                    palette.network,
                    gpu.pcie_tx_bytes_per_sec
                        .map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes_per_sec),
                ),
                (
                    "From the card",
                    palette.alternate,
                    gpu.pcie_rx_bytes_per_sec
                        .map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes_per_sec),
                ),
            ],
        );
    }
}

fn limits_and_identity(ui: &mut Ui, ctx: &TabContext<'_>, gpu: &GpuMetrics) {
    let palette = ctx.palette;

    if gpu.throttle_reasons.is_empty() {
        widgets::notice(
            ui,
            palette,
            "✓",
            "Nothing is holding this card back. It is free to boost as high as its cooling and power allow.",
            palette.good,
        );
    } else {
        for reason in &gpu.throttle_reasons {
            widgets::notice(ui, palette, "!", reason, palette.warn);
            ui.add_space(space::TIGHT);
        }
    }

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Driver power state",
        gpu.performance_state.as_deref().unwrap_or(format::UNAVAILABLE),
        "P0 means maximum performance; higher numbers mean the card has stepped down to save power.",
    );
    widgets::metric_row(ui, palette, "Model", &gpu.name, "");
    widgets::metric_row(
        ui,
        palette,
        "Chip family",
        gpu.architecture.as_deref().unwrap_or(format::UNAVAILABLE),
        "The generation of graphics chip this card is built on.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Driver version",
        gpu.driver_version.as_deref().unwrap_or(format::UNAVAILABLE),
        "",
    );
    widgets::metric_row(
        ui,
        palette,
        "Card firmware",
        gpu.vbios_version.as_deref().unwrap_or(format::UNAVAILABLE),
        "The version of the card's own on-board firmware.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Slot address",
        gpu.pci_bus_id.as_deref().unwrap_or(format::UNAVAILABLE),
        "Where the card sits on the motherboard, as the system sees it.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Readings come from",
        gpu.source.label(),
        "The driver interface perfview is using to read this card.",
    );
}

fn processes(ui: &mut Ui, ctx: &TabContext<'_>, gpu: &GpuMetrics) {
    let palette = ctx.palette;

    card(
        ui,
        palette,
        "Programs using the graphics card",
        "Everything currently holding video memory",
        |ui| {
            for process in gpu.processes.iter().take(12) {
                let share = match (process.vram_bytes, gpu.vram_total) {
                    (Some(used), Some(total)) if total > 0 => (used as f64 / total as f64) * 100.0,
                    _ => 0.0,
                };
                widgets::metric_row(
                    ui,
                    palette,
                    &process.name,
                    &process
                        .vram_bytes
                        .map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes),
                    &format!("Process ID {}", process.pid),
                );
                if share > 0.0 {
                    widgets::bar(ui, palette, share / 100.0, palette.gpu, 4.0);
                    ui.add_space(space::TIGHT);
                }
            }
            if gpu.processes.is_empty() {
                ui.label(
                    RichText::new("Nothing is using the graphics card right now.")
                        .size(12.0)
                        .color(palette.text_muted),
                );
            }
        },
    );
}
