//! The Overview page: one screen that answers "is my machine coping?".

use egui::{RichText, Ui};

use super::common::{self, Headline, ProcessRanking};
use crate::{
    app::{CardSpec, TabContext, card, card_row, divider},
    ui::{
        format,
        theme::space,
        widgets::{self, GraphLine, GraphUnit},
    },
};

pub fn show(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let snapshot = ctx.snapshot;

    headlines(ui, ctx);
    ui.add_space(space::BASE);

    // The combined graph: the single most useful view during a game, because
    // it shows which component is the limiting factor.
    card(
        ui,
        palette,
        "System load over the last minute",
        "Processor, graphics card and memory on one time line, so you can see which one is holding the others up.",
        |ui| {
            let cpu = ctx.history.get_or_empty("cpu.usage");
            let gpu = ctx.history.get_or_empty("gpu.0.usage");
            let memory = ctx.history.get_or_empty("memory.used_percent");

            let mut lines = vec![GraphLine::new("Processor", &cpu, palette.cpu)];
            if !gpu.is_empty() {
                lines.push(GraphLine::new("Graphics card", &gpu, palette.gpu).outline());
            }
            lines.push(GraphLine::new("Memory in use", &memory, palette.memory).outline());

            widgets::graph(
                ui,
                palette,
                "overview_combined",
                200.0,
                GraphUnit::Percent,
                &lines,
                ctx.window(),
            );

            ui.add_space(space::SNUG);
            let mut legend =
                vec![("Processor", palette.cpu, format::percent(snapshot.cpu.usage_percent))];
            if let Some(gpu) = snapshot.primary_gpu() {
                legend.push((
                    "Graphics card",
                    palette.gpu,
                    format::percent_opt(gpu.utilization_percent),
                ));
            }
            legend.push((
                "Memory in use",
                palette.memory,
                format::percent(snapshot.memory.used_percent()),
            ));
            widgets::graph_legend(ui, palette, &legend);
        },
    );

    ui.add_space(space::BASE);

    card_row(
        ui,
        palette,
        &mut [
            CardSpec::new("Processor", snapshot.cpu.model.clone(), &mut |ui| {
                processor_card(ui, ctx);
            }),
            CardSpec::new(
                "Graphics",
                snapshot
                    .primary_gpu()
                    .map_or_else(|| "No readable graphics card".to_owned(), |g| g.name.clone()),
                &mut |ui| {
                    graphics_card(ui, ctx);
                },
            ),
            CardSpec::new(
                "Memory",
                format!("{} installed", format::bytes(snapshot.memory.total)),
                &mut |ui| {
                    memory_card(ui, ctx);
                },
            ),
        ],
    );

    ui.add_space(space::BASE);

    card_row(
        ui,
        palette,
        &mut [
            CardSpec::new(
                "Drive activity",
                "Everything being read from and written to your drives",
                &mut |ui| {
                    storage_card(ui, ctx);
                },
            ),
            CardSpec::new("Network", "Data coming in and going out", &mut |ui| {
                network_card(ui, ctx);
            }),
        ],
    );

    ui.add_space(space::BASE);
    common::process_table(ui, ctx, ProcessRanking::Cpu, 8);
    ui.add_space(space::BASE);
}

fn headlines(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let snapshot = ctx.snapshot;

    let cpu_usage = snapshot.cpu.usage_percent;
    let memory_usage = snapshot.memory.used_percent();
    let gpu = snapshot.primary_gpu();
    let disk_rate = snapshot.total_disk_read() + snapshot.total_disk_write();
    let busiest_disk = snapshot.disks.iter().map(|d| d.busy_percent).fold(0.0, f64::max);

    let headlines = vec![
        Headline {
            label: "Processor",
            value: format!(
                "{} · {}",
                format::hertz_opt(snapshot.cpu.frequency_hz),
                format::celsius_opt(snapshot.cpu.temperature_c)
            ),
            support: format!(
                "{} cores · {} threads",
                snapshot.cpu.physical_cores, snapshot.cpu.logical_cores
            ),
            gauge: Some((cpu_usage, "in use")),
            colour: palette.severity(cpu_usage),
            hint: "The ring shows the share of all processor threads that is busy doing work.",
        },
        Headline {
            label: "Graphics",
            value: gpu.map_or_else(
                || format::UNAVAILABLE.to_owned(),
                |g| {
                    format!(
                        "{} · {}",
                        format::celsius_opt(g.temperature_c),
                        format::watts_opt(g.power_w)
                    )
                },
            ),
            support: gpu.map_or_else(
                || "No graphics telemetry on this machine".to_owned(),
                |g| g.name.clone(),
            ),
            gauge: gpu.and_then(|g| g.utilization_percent).map(|p| (p, "in use")),
            colour: palette.severity(gpu.and_then(|g| g.utilization_percent).unwrap_or(0.0)),
            hint: "The ring shows how much of the time your graphics card has work to do.",
        },
        Headline {
            label: "Memory",
            value: format::usage_of(snapshot.memory.used, snapshot.memory.total),
            support: format!("{} free for new programs", format::bytes(snapshot.memory.available)),
            gauge: Some((memory_usage, "in use")),
            colour: palette.severity(memory_usage),
            hint: "RAM genuinely in use. Files cached in spare memory count as free.",
        },
        Headline {
            label: "Storage",
            value: format::bytes_per_sec(disk_rate),
            support: format!(
                "{} read · {} written",
                format::bytes_per_sec(snapshot.total_disk_read()),
                format::bytes_per_sec(snapshot.total_disk_write())
            ),
            gauge: Some((busiest_disk, "busiest")),
            colour: palette.severity(busiest_disk),
            hint: "The ring shows how much of the time your hardest-working drive is occupied.",
        },
    ];

    common::headline_row(ui, palette, &headlines);
}

fn processor_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let cpu = &ctx.snapshot.cpu;

    let usage = ctx.history.get_or_empty("cpu.usage");
    widgets::graph(
        ui,
        palette,
        "overview_cpu",
        90.0,
        GraphUnit::Percent,
        &[GraphLine::new("Processor", &usage, palette.cpu)],
        ctx.window(),
    );
    ui.add_space(space::SNUG);

    widgets::metric_row(
        ui,
        palette,
        "Cores and threads",
        &format!("{} cores · {} threads", cpu.physical_cores, cpu.logical_cores),
        "Physical cores, and the logical threads they present to the operating system.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Current speed",
        &format::hertz_opt(cpu.frequency_hz),
        "Average clock speed across every thread, right now.",
    );
    common::temperature_line(
        ui,
        palette,
        "Temperature",
        cpu.temperature_c,
        format::ThermalKind::Cpu,
        "The hottest sensor on the processor package.",
    );
    widgets::metric_row_coloured(
        ui,
        palette,
        "Queue pressure",
        format::load_verdict(cpu.load_average[0], cpu.logical_cores),
        palette.severity(cpu.load_average[0] / cpu.logical_cores.max(1) as f64 * 100.0),
        "Based on the one-minute load average compared with your thread count.",
    );
}

fn graphics_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;

    let Some(gpu) = ctx.snapshot.primary_gpu() else {
        ui.label(
            RichText::new(
                ctx.gpu_notice.unwrap_or("No graphics telemetry is available on this machine."),
            )
            .size(12.0)
            .color(palette.text_muted),
        );
        return;
    };

    let usage = ctx.history.get_or_empty("gpu.0.usage");
    widgets::graph(
        ui,
        palette,
        "overview_gpu",
        90.0,
        GraphUnit::Percent,
        &[GraphLine::new("Graphics card", &usage, palette.gpu)],
        ctx.window(),
    );
    ui.add_space(space::SNUG);

    widgets::metric_row(
        ui,
        palette,
        "Video memory",
        &match (gpu.vram_used, gpu.vram_total) {
            (Some(used), Some(total)) => format::usage_of(used, total),
            _ => format::UNAVAILABLE.to_owned(),
        },
        "Memory on the graphics card itself, used for textures and frame buffers.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Core speed",
        &format::hertz_opt(gpu.clocks.first().and_then(|c| c.current_hz)),
        "Clock speed of the graphics processor.",
    );
    common::temperature_line(
        ui,
        palette,
        "Temperature",
        gpu.temperature_c,
        format::ThermalKind::Gpu,
        "Temperature of the graphics processor.",
    );
    widgets::metric_row_coloured(
        ui,
        palette,
        "Running freely",
        if gpu.is_unconstrained() { "Yes" } else { "No - see the Graphics page" },
        if gpu.is_unconstrained() { palette.good } else { palette.warn },
        "Whether anything is limiting the card's clock speeds.",
    );
}

fn memory_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let memory = &ctx.snapshot.memory;

    let used = ctx.history.get_or_empty("memory.used_percent");
    widgets::graph(
        ui,
        palette,
        "overview_memory",
        90.0,
        GraphUnit::Percent,
        &[GraphLine::new("In use", &used, palette.memory)],
        ctx.window(),
    );
    ui.add_space(space::SNUG);

    widgets::metric_row(
        ui,
        palette,
        "In use by programs",
        &format::usage_of(memory.used, memory.total),
        "Memory that programs are genuinely holding on to.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Held as file cache",
        &format::bytes(memory.reclaimable()),
        "Recently used files kept in spare RAM to make things faster. This is handed back the moment a program needs it.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Free for new programs",
        &format::bytes(memory.available),
        "What a new program could claim immediately, without anything being swapped to disk.",
    );
    let swapping = memory.swap_outs_per_sec > 1.0;
    widgets::metric_row_coloured(
        ui,
        palette,
        "Verdict",
        format::memory_verdict(memory.used_percent(), swapping),
        if swapping { palette.danger } else { palette.severity(memory.used_percent()) },
        "A plain-English read on whether memory is a problem right now.",
    );
}

fn storage_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let snapshot = ctx.snapshot;

    let read = ctx.history.get_or_empty("disk.total.read");
    let write = ctx.history.get_or_empty("disk.total.write");
    widgets::graph(
        ui,
        palette,
        "overview_disk",
        116.0,
        GraphUnit::BytesPerSec,
        &[
            GraphLine::new("Reading", &read, palette.storage),
            GraphLine::new("Writing", &write, palette.alternate).outline(),
        ],
        ctx.window(),
    );
    ui.add_space(space::SNUG);
    widgets::graph_legend(
        ui,
        palette,
        &[
            ("Reading", palette.storage, format::bytes_per_sec(snapshot.total_disk_read())),
            ("Writing", palette.alternate, format::bytes_per_sec(snapshot.total_disk_write())),
        ],
    );

    divider(ui, palette);
    for volume in snapshot.volumes.iter().take(3) {
        let used = volume.used_percent();
        widgets::metric_row_coloured(
            ui,
            palette,
            &volume.friendly_name(),
            &format!(
                "{} free of {}",
                format::bytes(volume.available_bytes),
                format::bytes(volume.total_bytes)
            ),
            palette.severity(used),
            &format!(
                "{} · {} · {}",
                volume.mount_point,
                volume.file_system,
                format::capacity_verdict(used)
            ),
        );
        widgets::bar(ui, palette, used / 100.0, palette.severity(used), 4.0);
        ui.add_space(space::SNUG);
    }
}

fn network_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let snapshot = ctx.snapshot;

    let down = ctx.history.get_or_empty("net.total.down");
    let up = ctx.history.get_or_empty("net.total.up");
    widgets::graph(
        ui,
        palette,
        "overview_net",
        116.0,
        GraphUnit::BytesPerSec,
        &[
            GraphLine::new("Downloading", &down, palette.network),
            GraphLine::new("Uploading", &up, palette.alternate).outline(),
        ],
        ctx.window(),
    );
    ui.add_space(space::SNUG);
    widgets::graph_legend(
        ui,
        palette,
        &[
            ("Downloading", palette.network, format::bytes_per_sec(snapshot.total_net_down())),
            ("Uploading", palette.alternate, format::bytes_per_sec(snapshot.total_net_up())),
        ],
    );

    divider(ui, palette);
    let active: Vec<_> =
        snapshot.networks.iter().filter(|n| !n.is_loopback && n.is_up).take(3).collect();

    if active.is_empty() {
        ui.label(
            RichText::new("No active network connections.").size(12.0).color(palette.text_muted),
        );
        return;
    }

    for interface in active {
        widgets::metric_row(
            ui,
            palette,
            &format!("{} ({})", interface.name, interface.kind),
            &interface.link_speed_bps.map_or_else(|| "Connected".to_owned(), format::bits_per_sec),
            &format!(
                "Received {} and sent {} since this machine booted.",
                format::bytes_decimal(interface.total_rx_bytes),
                format::bytes_decimal(interface.total_tx_bytes)
            ),
        );
    }
}
