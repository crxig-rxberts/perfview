//! The Processor page: per-thread load, clock speeds, heat and scheduler
//! pressure, with every figure explained.

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
    let cpu = &ctx.snapshot.cpu;

    let headlines = vec![
        Headline {
            label: "Processor load",
            value: format!("{} threads busy", busy_threads_phrase(cpu)),
            support: cpu.model.clone(),
            gauge: Some((cpu.usage_percent, "in use")),
            colour: palette.severity(cpu.usage_percent),
            hint: "The ring shows the share of all threads busy doing real work.",
        },
        Headline {
            label: "Clock speed",
            value: format::hertz_opt(cpu.frequency_hz),
            support: format!(
                "Fastest thread {} · boosts to {}",
                format::hertz_opt(cpu.peak_frequency_hz),
                format::hertz_opt(cpu.max_frequency_hz)
            ),
            gauge: None,
            colour: palette.cpu,
            hint: "Average across every thread. Idle threads drop their speed to save power.",
        },
        Headline {
            label: "Temperature",
            value: format::celsius_opt(cpu.temperature_c),
            support: cpu.temperature_c.map_or_else(
                || "No readable temperature sensor".to_owned(),
                |c| format::temperature_verdict(c, format::ThermalKind::Cpu).to_owned(),
            ),
            gauge: None,
            colour: cpu
                .temperature_c
                .map_or(palette.text_muted, |c| palette.thermal(c, format::ThermalKind::Cpu)),
            hint: "The hottest sensor on the processor package.",
        },
        Headline {
            label: "Power draw",
            value: format::watts_opt(cpu.package_power_w),
            support: if cpu.package_power_w.is_some() {
                "Processor package".to_owned()
            } else {
                "Sensor needs elevated access".to_owned()
            },
            gauge: None,
            colour: palette.cpu,
            hint: "Energy the processor package is drawing. On most distributions this counter is readable only by the administrator, so perfview shows it as unavailable when run normally.",
        },
    ];
    common::headline_row(ui, palette, &headlines);

    ui.add_space(space::BASE);

    card(
        ui,
        palette,
        "What the processor is spending its time on",
        "Split by the kind of work, because 90% spent waiting on a disk is a very different problem from 90% spent running a game.",
        |ui| {
            let total = ctx.history.get_or_empty("cpu.usage");
            let user = ctx.history.get_or_empty("cpu.user");
            let system = ctx.history.get_or_empty("cpu.system");
            let iowait = ctx.history.get_or_empty("cpu.iowait");

            widgets::graph(
                ui,
                palette,
                "cpu_breakdown",
                210.0,
                GraphUnit::Percent,
                &[
                    GraphLine::new("Total in use", &total, palette.cpu),
                    GraphLine::new("Your applications", &user, palette.gpu).outline(),
                    GraphLine::new("The operating system", &system, palette.memory).outline(),
                    GraphLine::new("Waiting on storage", &iowait, palette.storage).outline(),
                ],
                ctx.window(),
            );
            ui.add_space(space::SNUG);
            widgets::graph_legend(
                ui,
                palette,
                &[
                    ("Total in use", palette.cpu, format::percent(cpu.usage_percent)),
                    (
                        "Your applications",
                        palette.gpu,
                        format::percent(cpu.breakdown.user + cpu.breakdown.nice),
                    ),
                    ("The operating system", palette.memory, format::percent(cpu.breakdown.system)),
                    ("Waiting on storage", palette.storage, format::percent(cpu.breakdown.iowait)),
                ],
            );
        },
    );

    if ctx.settings.show_core_grid && !cpu.cores.is_empty() {
        ui.add_space(space::BASE);
        card(
            ui,
            palette,
            "Every thread at a glance",
            "One tile per logical processor. Hover a tile for its clock speed and a breakdown of what it is doing.",
            |ui| {
                let columns = cpu.cores.len().clamp(4, 16);
                widgets::core_grid(ui, palette, &cpu.cores, columns);
            },
        );
    }

    ui.add_space(space::BASE);

    card_row(
        ui,
        palette,
        &mut [
            CardSpec::new("This processor", cpu.model.clone(), &mut |ui| {
                specification_card(ui, ctx);
            }),
            CardSpec::new(
                "Scheduler activity",
                "How hard the operating system is working to share the processor out",
                &mut |ui| {
                    activity_card(ui, ctx);
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
                "Temperature and power",
                "Readings from the sensors built into the processor",
                &mut |ui| {
                    thermal_card(ui, ctx);
                },
            ),
            CardSpec::new(
                "Clock speed",
                "How fast the processor is running, and what is deciding that",
                &mut |ui| {
                    clocks_card(ui, ctx);
                },
            ),
        ],
    );

    ui.add_space(space::BASE);
    common::process_table(ui, ctx, ProcessRanking::Cpu, 10);
    ui.add_space(space::BASE);
}

fn specification_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let cpu = &ctx.snapshot.cpu;

    widgets::metric_row(ui, palette, "Maker", &cpu.vendor, "");
    widgets::metric_row(
        ui,
        palette,
        "Instruction set",
        &cpu.architecture,
        "The kind of machine code this processor runs.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Physical cores",
        &cpu.physical_cores.to_string(),
        "Independent processing units on the chip.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Logical threads",
        &format!(
            "{}{}",
            cpu.logical_cores,
            if cpu.threads_per_core > 1 { " (multithreading is on)" } else { "" }
        ),
        "Two threads per core lets each core work on a second task while the first waits.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Speed range",
        &format!(
            "{} to {}",
            format::hertz_opt(cpu.min_frequency_hz),
            format::hertz_opt(cpu.max_frequency_hz)
        ),
        "The slowest power-saving speed and the fastest boost speed.",
    );

    divider(ui, palette);

    for (label, size, hint) in [
        ("Level 1 data cache", cpu.cache_l1d, "Fastest memory, per core, for data."),
        ("Level 1 instruction cache", cpu.cache_l1i, "Fastest memory, per core, for code."),
        ("Level 2 cache", cpu.cache_l2, "Larger per-core cache."),
        (
            "Level 3 cache",
            cpu.cache_l3,
            "Large cache shared between cores. Games benefit a lot from this.",
        ),
    ] {
        widgets::metric_row(
            ui,
            palette,
            label,
            &size.map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes),
            hint,
        );
    }
}

fn activity_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let cpu = &ctx.snapshot.cpu;

    let ratio = cpu.load_average[0] / cpu.logical_cores.max(1) as f64;
    widgets::metric_row_coloured(
        ui,
        palette,
        "Verdict",
        format::load_verdict(cpu.load_average[0], cpu.logical_cores),
        palette.severity(ratio * 100.0),
        "Compares the number of tasks wanting to run against the threads you have.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Tasks wanting to run",
        &format!(
            "{:.2} now · {:.2} over 5 min · {:.2} over 15 min",
            cpu.load_average[0], cpu.load_average[1], cpu.load_average[2]
        ),
        "The classic load average. Compare it with your thread count: below that number means no queuing.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Currently running",
        &format!("{} tasks", cpu.procs_running),
        "Tasks actually on a processor or waiting for one this instant.",
    );
    widgets::metric_row_coloured(
        ui,
        palette,
        "Stuck waiting on disk",
        &format!("{} tasks", cpu.procs_blocked),
        if cpu.procs_blocked > 2 { palette.warn } else { palette.text },
        "Tasks that cannot continue until a drive responds. Consistently high means storage is your bottleneck.",
    );

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Task switches",
        &format::per_sec(cpu.context_switches_per_sec),
        "How often the processor swaps between tasks. Tens of thousands per second is normal under load.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Hardware interrupts",
        &format::per_sec(cpu.interrupts_per_sec),
        "Signals from devices such as your network card, mouse and drives.",
    );
    widgets::metric_row(
        ui,
        palette,
        "New processes started",
        &format::per_sec(cpu.forks_per_sec),
        "Programs being launched per second.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Open programs",
        &format!(
            "{} processes · {} threads",
            format::count(cpu.process_count as u64),
            format::count(cpu.thread_count as u64)
        ),
        "",
    );

    if cpu.breakdown.steal > 0.5 {
        ui.add_space(space::SNUG);
        widgets::notice(
            ui,
            palette,
            "!",
            &format!(
                "{} of processor time is being taken by the host machine. This is normal in a virtual machine.",
                format::percent(cpu.breakdown.steal)
            ),
            palette.warn,
        );
    }
}

fn thermal_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let cpu = &ctx.snapshot.cpu;

    let temperature = ctx.history.get_or_empty("cpu.temperature");
    if temperature.is_empty() {
        widgets::notice(
            ui,
            palette,
            "i",
            "No processor temperature sensor is readable on this machine. On some systems the k10temp or coretemp kernel module needs loading.",
            palette.network,
        );
    } else {
        widgets::graph(
            ui,
            palette,
            "cpu_temperature",
            104.0,
            GraphUnit::Celsius,
            &[GraphLine::new("Package", &temperature, palette.warn)],
            ctx.window(),
        );
        ui.add_space(space::SNUG);
        widgets::graph_legend(
            ui,
            palette,
            &[("Peak this period", palette.warn, format::celsius(temperature.max()))],
        );
        divider(ui, palette);
    }

    for sensor in &cpu.sensors {
        common::temperature_line(
            ui,
            palette,
            &sensor.label,
            Some(sensor.celsius),
            format::ThermalKind::Cpu,
            "",
        );
    }

    divider(ui, palette);
    widgets::metric_row(
        ui,
        palette,
        "Package power",
        &format::watts_opt(cpu.package_power_w),
        "Energy the whole processor package is drawing right now.",
    );
}

fn clocks_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let cpu = &ctx.snapshot.cpu;

    let frequency = ctx.history.get_or_empty("cpu.frequency");
    if frequency.is_empty() {
        ui.label(
            RichText::new("Live clock speeds are not available on this machine.")
                .size(12.0)
                .color(palette.text_muted),
        );
    } else {
        widgets::graph(
            ui,
            palette,
            "cpu_frequency",
            104.0,
            GraphUnit::Gigahertz,
            &[GraphLine::new("Average speed", &frequency, palette.cpu)],
            ctx.window(),
        );
        ui.add_space(space::SNUG);
        widgets::graph_legend(
            ui,
            palette,
            &[
                ("Now", palette.cpu, format::hertz_opt(cpu.frequency_hz)),
                ("Peak this period", palette.gpu, format!("{:.2} GHz", frequency.max())),
            ],
        );
    }

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Fastest thread right now",
        &format::hertz_opt(cpu.peak_frequency_hz),
        "Modern processors boost a few threads much higher than the rest. This is that number.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Power plan",
        cpu.governor.as_deref().map_or(format::UNAVAILABLE, describe_governor),
        "The rule the kernel uses to decide clock speeds.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Managed by",
        cpu.scaling_driver.as_deref().unwrap_or(format::UNAVAILABLE),
        "The kernel driver controlling the processor's speed and voltage.",
    );
}

/// Express load as "roughly N of your M threads", which is far more intuitive
/// than a bare percentage.
fn busy_threads_phrase(cpu: &crate::metrics::CpuMetrics) -> String {
    let busy = cpu.usage_percent / 100.0 * cpu.logical_cores as f64;
    format!("{busy:.1} of {}", cpu.logical_cores)
}

/// Governor names are jargon; say what they actually do.
fn describe_governor(raw: &str) -> &'static str {
    match raw {
        "performance" => "Performance - always full speed",
        "powersave" => "Power saving - favour low clocks",
        "schedutil" => "Balanced - follows demand",
        "ondemand" => "On demand - speeds up when busy",
        "conservative" => "Conservative - raises speed gradually",
        "userspace" => "Fixed by a program",
        _ => "Custom",
    }
}
