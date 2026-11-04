//! The Storage page: drive activity, speed, capacity and health.

use egui::{RichText, Ui};

use super::common::{self, Headline, ProcessRanking};
use crate::{
    app::{TabContext, card, divider},
    metrics::DiskMetrics,
    ui::{
        format,
        theme::space,
        widgets::{self, GraphLine, GraphUnit, Segment},
    },
};

pub fn show(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;
    let snapshot = ctx.snapshot;

    let busiest = snapshot.disks.iter().map(|d| d.busy_percent).fold(0.0, f64::max);
    let total_capacity: u64 = snapshot.volumes.iter().map(|v| v.total_bytes).sum();
    let total_used: u64 = snapshot.volumes.iter().map(|v| v.used_bytes).sum();
    let capacity_percent =
        if total_capacity == 0 { 0.0 } else { (total_used as f64 / total_capacity as f64) * 100.0 };

    let headlines = vec![
        Headline {
            label: "Reading",
            value: format::bytes_per_sec(snapshot.total_disk_read()),
            support: "Loaded from your drives right now".to_owned(),
            gauge: None,
            colour: palette.storage,
            hint: "Data being loaded from your drives, across every device.",
        },
        Headline {
            label: "Writing",
            value: format::bytes_per_sec(snapshot.total_disk_write()),
            support: "Saved to your drives right now".to_owned(),
            gauge: None,
            colour: palette.alternate,
            hint: "Data being saved to your drives, across every device.",
        },
        Headline {
            label: "Busiest drive",
            value: snapshot
                .disks
                .iter()
                .max_by(|a, b| a.busy_percent.total_cmp(&b.busy_percent))
                .map_or_else(|| "No drives found".to_owned(), short_disk_name),
            support: format!("{} drives fitted", snapshot.disks.len()),
            gauge: Some((busiest, "occupied")),
            colour: palette.severity(busiest),
            hint: "The ring shows how much of the time the hardest-working drive has a request in progress. Sustained 100% means storage is your bottleneck.",
        },
        Headline {
            label: "Space",
            value: format!("{} free", format::bytes(total_capacity.saturating_sub(total_used))),
            support: format!("of {} across every filesystem", format::bytes(total_capacity)),
            gauge: Some((capacity_percent, "used")),
            colour: palette.severity(capacity_percent),
            hint: "Combined capacity across every mounted filesystem.",
        },
    ];
    common::headline_row(ui, palette, &headlines);

    ui.add_space(space::BASE);

    card(
        ui,
        palette,
        "Drive activity over time",
        "Everything being read from and written to your drives, combined",
        |ui| {
            let read = ctx.history.get_or_empty("disk.total.read");
            let write = ctx.history.get_or_empty("disk.total.write");
            widgets::graph(
                ui,
                palette,
                "storage_total",
                170.0,
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
                    (
                        "Reading now",
                        palette.storage,
                        format::bytes_per_sec(snapshot.total_disk_read()),
                    ),
                    (
                        "Writing now",
                        palette.alternate,
                        format::bytes_per_sec(snapshot.total_disk_write()),
                    ),
                    ("Peak read", palette.text_muted, format::bytes_per_sec(read.max())),
                    ("Peak write", palette.text_muted, format::bytes_per_sec(write.max())),
                ],
            );
        },
    );

    ui.add_space(space::BASE);
    volumes_card(ui, ctx);

    for disk in &snapshot.disks {
        ui.add_space(space::BASE);
        disk_card(ui, ctx, disk);
    }

    ui.add_space(space::BASE);
    common::process_table(ui, ctx, ProcessRanking::DiskActivity, 8);
    ui.add_space(space::BASE);
}

fn volumes_card(ui: &mut Ui, ctx: &TabContext<'_>) {
    let palette = ctx.palette;

    card(
        ui,
        palette,
        "Space on your drives",
        "Each mounted filesystem, and how much room is left",
        |ui| {
            if ctx.snapshot.volumes.is_empty() {
                ui.label(
                    RichText::new("No filesystems found.").size(12.0).color(palette.text_muted),
                );
                return;
            }

            for volume in &ctx.snapshot.volumes {
                let used = volume.used_percent();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(volume.friendly_name())
                            .size(13.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} · {}{}",
                            volume.mount_point,
                            volume.file_system,
                            if volume.is_read_only { " · read only" } else { "" }
                        ))
                        .size(11.0)
                        .color(palette.text_faint),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        widgets::badge(
                            ui,
                            palette,
                            format::capacity_verdict(used),
                            palette.severity(used),
                        );
                    });
                });
                ui.add_space(space::TIGHT);

                widgets::stacked_bar(
                    ui,
                    palette,
                    volume.total_bytes,
                    &[
                        Segment {
                            label: "Used".to_owned(),
                            bytes: volume.used_bytes,
                            colour: palette.severity(used),
                            hint: format!(
                                "{} of files and programs.",
                                format::bytes(volume.used_bytes)
                            ),
                        },
                        Segment {
                            label: "Free".to_owned(),
                            bytes: volume.available_bytes,
                            colour: palette.fill(palette.text_faint, 70),
                            hint: format!(
                                "{} still available.",
                                format::bytes(volume.available_bytes)
                            ),
                        },
                    ],
                    14.0,
                );

                if volume.is_nearly_full() {
                    ui.add_space(space::TIGHT);
                    widgets::notice(
                        ui,
                        palette,
                        "!",
                        &format!(
                            "{} is {} full. Filesystems slow down noticeably once they pass 90%.",
                            volume.friendly_name(),
                            format::percent_whole(used)
                        ),
                        palette.warn,
                    );
                }

                divider(ui, palette);
            }
        },
    );
}

fn disk_card(ui: &mut Ui, ctx: &TabContext<'_>, disk: &DiskMetrics) {
    let palette = ctx.palette;
    let key = |suffix: &str| format!("disk.{}.{suffix}", disk.name);

    card(
        ui,
        palette,
        &friendly_disk_name(disk),
        &format!(
            "{} · {} · device {}{}",
            disk.kind,
            format::bytes_decimal(disk.size_bytes),
            disk.name,
            if disk.volumes.is_empty() {
                String::new()
            } else {
                format!(" · holds {}", disk.volumes.join(", "))
            }
        ),
        |ui| {
            let read = ctx.history.get_or_empty(&key("read"));
            let write = ctx.history.get_or_empty(&key("write"));
            widgets::graph(
                ui,
                palette,
                &format!("disk_rate_{}", disk.name),
                130.0,
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
                    ("Reading", palette.storage, format::bytes_per_sec(disk.read_bytes_per_sec)),
                    ("Writing", palette.alternate, format::bytes_per_sec(disk.write_bytes_per_sec)),
                ],
            );

            divider(ui, palette);

            ui.columns(2, |columns| {
                let ui = &mut columns[0];
                widgets::metric_row_coloured(
                    ui,
                    palette,
                    "How busy",
                    &format!(
                        "{} of the time",
                        format::percent_whole(disk.busy_percent)
                    ),
                    palette.severity(disk.busy_percent),
                    "The share of time this drive had at least one request in progress. Near 100% means it cannot keep up.",
                );
                widgets::bar(
                    ui,
                    palette,
                    disk.busy_percent / 100.0,
                    palette.severity(disk.busy_percent),
                    5.0,
                );
                ui.add_space(space::TIGHT);
                widgets::metric_row(
                    ui,
                    palette,
                    "Requests per second",
                    &format::per_sec(disk.total_ops_per_sec()),
                    "Separate read and write operations the drive is handling. Known in the trade as IOPS.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Reads · writes",
                    &format!(
                        "{} · {}",
                        format::per_sec(disk.read_ops_per_sec),
                        format::per_sec(disk.write_ops_per_sec)
                    ),
                    "",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Waiting in the queue",
                    &format!("{:.2} requests", disk.queue_depth),
                    "How many requests are backed up on average. Consistently above a few means the drive is saturated.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "In progress right now",
                    &format!("{} requests", disk.in_flight),
                    "",
                );

                let ui = &mut columns[1];
                widgets::metric_row(
                    ui,
                    palette,
                    "Time to answer a read",
                    &format::milliseconds_opt(disk.read_latency_ms),
                    "Average wait for a read to complete. A fast solid state drive answers in well under a millisecond.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Time to answer a write",
                    &format::milliseconds_opt(disk.write_latency_ms),
                    "Average wait for a write to complete.",
                );
                common::temperature_line(
                    ui,
                    palette,
                    "Temperature",
                    disk.temperature_c,
                    format::ThermalKind::Drive,
                    "Fast drives throttle themselves when they get hot, which shows up as sudden loading pauses.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Read since boot",
                    &format::bytes_decimal(disk.total_read_bytes),
                    "Total data read from this drive since the machine started.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Written since boot",
                    &format::bytes_decimal(disk.total_write_bytes),
                    "Total data written to this drive since the machine started.",
                );
            });

            divider(ui, palette);

            ui.columns(2, |columns| {
                let ui = &mut columns[0];
                widgets::metric_row(
                    ui,
                    palette,
                    "Model",
                    disk.model.as_deref().unwrap_or(format::UNAVAILABLE),
                    "",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Firmware",
                    disk.firmware.as_deref().unwrap_or(format::UNAVAILABLE),
                    "The version of the drive's own software.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Type",
                    &disk.kind,
                    "",
                );

                let ui = &mut columns[1];
                widgets::metric_row(
                    ui,
                    palette,
                    "Capacity",
                    &format!(
                        "{} ({})",
                        format::bytes_decimal(disk.size_bytes),
                        format::bytes(disk.size_bytes)
                    ),
                    "Drive makers count in thousands; your computer counts in 1024s. Both numbers describe the same drive.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Block size",
                    &disk
                        .physical_block_size
                        .map_or_else(|| format::UNAVAILABLE.to_owned(), format::bytes),
                    "The smallest chunk the drive physically reads or writes.",
                );
                widgets::metric_row(
                    ui,
                    palette,
                    "Request ordering",
                    disk.scheduler.as_deref().map_or(format::UNAVAILABLE, describe_scheduler),
                    "How the kernel decides which request to send to the drive next.",
                );
            });
        },
    );
}

/// A name a person would recognise, falling back to the kernel device name.
fn friendly_disk_name(disk: &DiskMetrics) -> String {
    match &disk.model {
        Some(model) if !model.is_empty() => model.clone(),
        _ => format!("Drive {}", disk.name),
    }
}

/// The same name trimmed for a headline, where it has to fit on one line.
///
/// Firmware model strings usually end in a part number, e.g.
/// "Seagate FireCuda 510 SSD ZP1000GM30031". That last token means nothing to
/// anyone, so it is dropped.
fn short_disk_name(disk: &DiskMetrics) -> String {
    let full = friendly_disk_name(disk);
    let words: Vec<&str> = full.split_whitespace().collect();

    let is_part_number = |word: &str| {
        word.len() >= 6
            && word.chars().any(|c| c.is_ascii_digit())
            && word.chars().any(|c| c.is_ascii_uppercase())
            && !word.chars().all(|c| c.is_ascii_uppercase())
    };

    let trimmed: Vec<&str> = match words.split_last() {
        Some((last, rest)) if is_part_number(last) && !rest.is_empty() => rest.to_vec(),
        _ => words,
    };

    let name = trimmed.join(" ");
    if name.len() > 30 { format!("{}…", name[..29].trim_end()) } else { name }
}

fn describe_scheduler(raw: &str) -> &'static str {
    match raw {
        "none" => "Straight through - best for fast solid state drives",
        "mq-deadline" => "Deadline - keeps any one request from waiting too long",
        "bfq" => "Fair queueing - shares the drive evenly between programs",
        "kyber" => "Kyber - aims for low latency",
        _ => "Custom",
    }
}
