//! The Memory page.
//!
//! The goal here is to kill the "Linux is eating my RAM" myth: the composition
//! bar shows exactly how much of what looks like used memory is really just
//! cache that will be handed back on demand.

use egui::{RichText, Ui};

use super::common::{self, Headline, ProcessRanking};
use crate::{
    app::{CardSpec, TabContext, card, card_row, divider},
    ui::{
        format,
        theme::space,
        widgets::{self, GraphLine, GraphUnit, Segment},
    },
};

pub fn show(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let memory = &ctx.snapshot.memory;
    let swapping = memory.swap_outs_per_sec > 1.0;

    let headlines = vec![
        Headline {
            label: "In use by programs",
            value: format::bytes(memory.used),
            support: format!(
                "of {} installed · {}",
                format::bytes(memory.total),
                format::memory_verdict(memory.used_percent(), swapping)
            ),
            gauge: Some((memory.used_percent(), "in use")),
            colour: palette.severity(memory.used_percent()),
            hint: "Memory programs are genuinely holding. Cache is not counted.",
        },
        Headline {
            label: "Free for new programs",
            value: format::bytes(memory.available),
            support: format!("{} completely untouched", format::bytes(memory.free)),
            gauge: None,
            colour: palette.memory,
            hint: "What a new program could take right now without anything being pushed to disk.",
        },
        Headline {
            label: "Held as cache",
            value: format::bytes(memory.reclaimable()),
            support: "Released the instant a program needs it".to_owned(),
            gauge: None,
            colour: palette.network,
            hint: "Files kept in spare RAM to speed things up. This is why Linux appears to use all your memory.",
        },
        Headline {
            label: "Swap",
            value: if memory.swap_total == 0 {
                "Not configured".to_owned()
            } else {
                format::bytes(memory.swap_used)
            },
            support: if memory.swap_total == 0 {
                "Nothing can overflow to disk".to_owned()
            } else if swapping {
                "Actively swapping - expect stutter".to_owned()
            } else {
                format!("of {} · quiet", format::bytes(memory.swap_total))
            },
            gauge: (memory.swap_total > 0).then(|| (memory.swap_used_percent(), "in use")),
            colour: if swapping {
                palette.danger
            } else {
                palette.severity(memory.swap_used_percent())
            },
            hint: "Memory pushed out to disk. Heavy use here causes noticeable stutter.",
        },
    ];
    common::headline_row(ui, palette, &headlines);

    ui.add_space(space::BASE);

    card(
        ui,
        palette,
        "Where your memory has gone",
        "Linux deliberately fills spare memory with cached files. That is a good thing - it is handed straight back when a program asks for it.",
        |ui| {
            let cache = memory.cached.saturating_sub(memory.shared);
            let other = memory
                .total
                .saturating_sub(memory.used)
                .saturating_sub(memory.buffers)
                .saturating_sub(cache)
                .saturating_sub(memory.shared)
                .saturating_sub(memory.slab);

            widgets::stacked_bar(
                ui,
                palette,
                memory.total,
                &[
                    Segment {
                        label: "Programs".to_owned(),
                        bytes: memory.used,
                        colour: palette.memory,
                        hint: "Memory your running applications are actually holding.".to_owned(),
                    },
                    Segment {
                        label: "File cache".to_owned(),
                        bytes: cache,
                        colour: palette.network,
                        hint: "Copies of recently read files, kept so they do not need re-reading from disk.".to_owned(),
                    },
                    Segment {
                        label: "Shared memory".to_owned(),
                        bytes: memory.shared,
                        colour: palette.gpu,
                        hint: "Memory shared between programs, including anything stored in /dev/shm.".to_owned(),
                    },
                    Segment {
                        label: "Disk buffers".to_owned(),
                        bytes: memory.buffers,
                        colour: palette.storage,
                        hint: "Small staging area for data on its way to or from a drive.".to_owned(),
                    },
                    Segment {
                        label: "Kernel".to_owned(),
                        bytes: memory.slab,
                        colour: palette.alternate,
                        hint: "The operating system's own bookkeeping structures.".to_owned(),
                    },
                    Segment {
                        label: "Completely free".to_owned(),
                        bytes: other,
                        colour: palette.fill(palette.text_faint, 70),
                        hint: "Memory that is not being used for anything at all.".to_owned(),
                    },
                ],
                18.0,
            );

            ui.add_space(space::BASE);

            let used = ctx.history.get_or_empty("memory.used");
            let cached = ctx.history.get_or_empty("memory.cached");
            widgets::graph(
                ui,
                palette,
                "memory_usage",
                170.0,
                GraphUnit::Bytes,
                &[
                    GraphLine::new("Used by programs", &used, palette.memory),
                    GraphLine::new("Held as cache", &cached, palette.network).outline(),
                ],
                ctx.window(),
            );
            ui.add_space(space::SNUG);
            widgets::graph_legend(
                ui,
                palette,
                &[
                    ("Used by programs", palette.memory, format::bytes(memory.used)),
                    ("Held as cache", palette.network, format::bytes(memory.reclaimable())),
                ],
            );
        },
    );

    ui.add_space(space::BASE);

    card_row(
        ui,
        palette,
        &mut [
            CardSpec::new(
                "Swap space",
                "Disk space used as overflow when memory runs out. It is hundreds of times slower than RAM.",
                &mut |ui| {
                    swap_card(ui, ctx);
                },
            ),
            CardSpec::new(
                "Memory traffic",
                "Data moving between memory and your drives",
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
            CardSpec::new("Installed memory", "The physical sticks in your machine", &mut |ui| {
                hardware_card(ui, ctx);
            }),
            CardSpec::new("Fine detail", "For when you want the whole picture", &mut |ui| {
                detail_card(ui, ctx);
            }),
        ],
    );

    ui.add_space(space::BASE);
    common::process_table(ui, ctx, ProcessRanking::Memory, 10);
    ui.add_space(space::BASE);
}

fn swap_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let memory = &ctx.snapshot.memory;

    if memory.swap_total == 0 {
        widgets::notice(
            ui,
            palette,
            "i",
            "No swap space is configured. If you run out of memory, the system will close programs rather than slow down.",
            palette.network,
        );
        return;
    }

    let used = memory.swap_used_percent();
    widgets::bar(ui, palette, used / 100.0, palette.severity(used), 10.0);
    ui.add_space(space::SNUG);

    widgets::metric_row(
        ui,
        palette,
        "In use",
        &format::usage_summary(memory.swap_used, memory.swap_total),
        "Some swap being used is perfectly normal - the kernel moves pages it has not touched in a long time.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Still in memory too",
        &format::bytes(memory.swap_cached),
        "Pages that are on disk but also still cached in RAM, so reading them back is free.",
    );

    if memory.zswap_original > 0 {
        widgets::metric_row(
            ui,
            palette,
            "Compressed in memory",
            &format!(
                "{} squeezed into {}",
                format::bytes(memory.zswap_original),
                format::bytes(memory.zswap_compressed)
            ),
            "Compressed swap keeps pages in RAM instead of writing them to disk, which is far faster.",
        );
    }

    divider(ui, palette);

    let swapping = memory.swap_outs_per_sec > 1.0 || memory.swap_ins_per_sec > 1.0;
    widgets::metric_row_coloured(
        ui,
        palette,
        "Activity right now",
        if swapping { "Actively swapping - expect stutter" } else { "Quiet" },
        if swapping { palette.danger } else { palette.good },
        "Steady swapping during a game is the usual cause of sudden frame drops.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Pages moved to disk",
        &format::per_sec(memory.swap_outs_per_sec),
        "",
    );
    widgets::metric_row(
        ui,
        palette,
        "Pages pulled back",
        &format::per_sec(memory.swap_ins_per_sec),
        "",
    );
}

fn activity_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let memory = &ctx.snapshot.memory;

    let faults = ctx.history.get_or_empty("memory.major_faults");
    widgets::graph(
        ui,
        palette,
        "memory_faults",
        96.0,
        GraphUnit::Rate,
        &[GraphLine::new("Reads from disk", &faults, palette.storage)],
        ctx.window(),
    );
    ui.add_space(space::SNUG);

    widgets::metric_row_coloured(
        ui,
        palette,
        "Memory reads that hit the disk",
        &format::per_sec(memory.major_faults_per_sec),
        if memory.major_faults_per_sec > 100.0 { palette.warn } else { palette.text },
        "When a program needs data that is not in memory, it has to wait for a drive. Sustained high numbers mean you are short of RAM.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Read from drives",
        &format::bytes_per_sec(memory.page_ins_per_sec),
        "Data loaded from storage into memory.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Written to drives",
        &format::bytes_per_sec(memory.page_outs_per_sec),
        "Data written from memory out to storage.",
    );

    divider(ui, palette);

    widgets::metric_row(
        ui,
        palette,
        "Waiting to be written",
        &format::bytes(memory.dirty),
        "Changes that exist only in memory so far. A large figure means a lot would be lost in a power cut.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Being written now",
        &format::bytes(memory.writeback),
        "Data on its way out to a drive this instant.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Mapped from files",
        &format::bytes(memory.mapped),
        "Program code and game assets loaded straight from files into memory.",
    );
}

fn hardware_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let memory = &ctx.snapshot.memory;

    if memory.modules.is_empty() {
        widgets::notice(
            ui,
            palette,
            "i",
            "Details of the individual memory sticks are not readable. On most systems this information requires administrator access.",
            palette.network,
        );
    } else {
        for module in &memory.modules {
            widgets::metric_row(
                ui,
                palette,
                &module.locator,
                &format!(
                    "{}{}{}",
                    format::bytes(module.size_bytes),
                    module.kind.as_ref().map_or(String::new(), |k| format!(" {k}")),
                    module.speed_mts.map_or(String::new(), |s| format!(" at {s} MT/s"))
                ),
                &module.manufacturer.as_ref().map_or(String::new(), |m| format!("Made by {m}")),
            );
        }
        divider(ui, palette);
    }

    widgets::metric_row(
        ui,
        palette,
        "Total installed",
        &format::bytes(memory.total),
        "All usable physical memory. Slightly less than the advertised size, because the firmware reserves a little.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Populated slots",
        &if memory.modules.is_empty() {
            format::UNAVAILABLE.to_owned()
        } else {
            memory.modules.len().to_string()
        },
        "",
    );
}

fn detail_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let memory = &ctx.snapshot.memory;

    widgets::metric_row(
        ui,
        palette,
        "Completely untouched",
        &format::bytes(memory.free),
        "Memory not used for anything, not even cache. A small number here is healthy.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Kernel structures",
        &format::bytes(memory.slab),
        "The operating system's own data, such as the file and directory caches.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Of which reclaimable",
        &format::bytes(memory.slab_reclaimable),
        "The part of the kernel's data that can be dropped if memory gets tight.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Promised to programs",
        &format::bytes(memory.committed),
        "The total every running program has asked for. This can exceed your installed memory, because programs rarely use everything they request.",
    );
    widgets::metric_row(
        ui,
        palette,
        "Promise limit",
        &format::bytes(memory.commit_limit),
        "The most the kernel is willing to promise, based on your memory plus swap.",
    );

    if memory.hugepages_total > 0 {
        divider(ui, palette);
        widgets::metric_row(
            ui,
            palette,
            "Large pages reserved",
            &format!(
                "{} of {} free",
                format::count(memory.hugepages_free),
                format::count(memory.hugepages_total)
            ),
            "Large memory pages, used by databases and some emulators for speed.",
        );
    }

    ui.add_space(space::SNUG);
    ui.label(
    RichText::new(
        "Tip: a high \"in use\" figure is only a problem if \"free for new programs\" is also low.",
    )
    .size(11.0)
    .color(palette.text_faint),
);
}
