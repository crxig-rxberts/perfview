//! Pieces shared by more than one tab.

use egui::{Color32, Rect, RichText, Sense, Ui, Vec2, vec2};
use egui_extras::{Column, TableBuilder};

use crate::{
    app::TabContext,
    metrics::ProcessMetrics,
    ui::{
        format,
        theme::{self, Palette, space},
        widgets,
    },
};

/// Which column a process table is ranked by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessRanking {
    Cpu,
    Memory,
    DiskActivity,
}

impl ProcessRanking {
    fn heading(self) -> (&'static str, &'static str) {
        match self {
            Self::Cpu => (
                "Busiest applications",
                "Ranked by how much processor time each one is using right now",
            ),
            Self::Memory => {
                ("Biggest memory users", "Ranked by the physical RAM each one is actually holding")
            }
            Self::DiskActivity => (
                "Busiest applications on disk",
                "Ranked by how much data each one is reading and writing right now",
            ),
        }
    }

    fn sort(self, processes: &mut [ProcessMetrics]) {
        match self {
            Self::Cpu => processes.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent)),
            Self::Memory => processes.sort_by_key(|p| std::cmp::Reverse(p.memory_bytes)),
            Self::DiskActivity => processes.sort_by(|a, b| {
                (b.disk_read_bytes_per_sec + b.disk_write_bytes_per_sec)
                    .total_cmp(&(a.disk_read_bytes_per_sec + a.disk_write_bytes_per_sec))
            }),
        }
    }
}

/// A sortable table of the top processes.
pub fn process_table(ui: &mut Ui, ctx: &TabContext<'_>, ranking: ProcessRanking, rows: usize) {
    let palette = ctx.palette;
    let (title, subtitle) = ranking.heading();

    crate::app::card(ui, palette, title, subtitle, |ui| {
        let mut processes = ctx.snapshot.processes.clone();
        ranking.sort(&mut processes);
        processes.truncate(rows);

        if processes.is_empty() {
            ui.label(
                RichText::new("No processes to show yet.").size(12.0).color(palette.text_muted),
            );
            return;
        }

        let cores = ctx.snapshot.cpu.logical_cores;
        let total_memory = ctx.snapshot.memory.total;

        TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(200.0))
            .column(Column::exact(112.0))
            .column(Column::exact(126.0))
            .column(Column::exact(112.0))
            .column(Column::exact(104.0))
            .header(26.0, |mut header| {
                for (label, hint) in [
                    (
                        "Application",
                        "The program's executable name. Hover a row for the full command.",
                    ),
                    ("Processor", "Share of the whole processor, across every core."),
                    ("Memory", "Physical RAM held by this program."),
                    ("Disk", "Combined read and write throughput right now."),
                    ("Running for", "How long the program has been open."),
                ] {
                    header.col(|ui| {
                        ui.label(
                            RichText::new(label).size(11.0).strong().color(palette.text_faint),
                        )
                        .on_hover_text(hint);
                    });
                }
            })
            .body(|mut body| {
                for process in &processes {
                    body.row(theme::ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            let response = ui
                                .label(RichText::new(&process.name).size(12.0).color(palette.text));
                            response.on_hover_text(format!(
                                "{}\nProcess ID {} · {} · {} threads\n{}",
                                process.name,
                                process.pid,
                                process.status,
                                process.thread_count,
                                if process.command.is_empty() {
                                    "(no command line available)".to_owned()
                                } else {
                                    process.command.clone()
                                }
                            ));
                        });

                        let share = process.cpu_percent_of_machine(cores);
                        row.col(|ui| {
                            value_with_bar(
                                ui,
                                palette,
                                &format::percent(share),
                                share,
                                palette.cpu,
                            );
                        });

                        let memory_share = if total_memory == 0 {
                            0.0
                        } else {
                            (process.memory_bytes as f64 / total_memory as f64) * 100.0
                        };
                        row.col(|ui| {
                            value_with_bar(
                                ui,
                                palette,
                                &format::bytes(process.memory_bytes),
                                memory_share,
                                palette.memory,
                            );
                        });

                        row.col(|ui| {
                            let total =
                                process.disk_read_bytes_per_sec + process.disk_write_bytes_per_sec;
                            ui.label(
                                RichText::new(if total < 1.0 {
                                    "—".to_owned()
                                } else {
                                    format::bytes_per_sec(total)
                                })
                                .size(12.0)
                                .color(if total < 1.0 {
                                    palette.text_faint
                                } else {
                                    palette.text
                                }),
                            )
                            .on_hover_text(format!(
                                "Reading {}\nWriting {}",
                                format::bytes_per_sec(process.disk_read_bytes_per_sec),
                                format::bytes_per_sec(process.disk_write_bytes_per_sec)
                            ));
                        });

                        row.col(|ui| {
                            ui.label(
                                RichText::new(format::duration_clock(process.run_time_secs))
                                    .size(12.0)
                                    .color(palette.text_muted),
                            );
                        });
                    });
                }
            });
    });
}

/// A number with a thin proportion bar behind it, so rows can be compared by
/// eye without reading every figure.
fn value_with_bar(ui: &mut Ui, palette: &Palette, text: &str, percent: f64, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ui.available_height().min(18.0)),
        egui::Sense::hover(),
    );
    let fraction = (percent / 100.0).clamp(0.0, 1.0) as f32;
    if fraction > 0.004 {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * fraction, rect.height())),
            egui::CornerRadius::same(3),
            palette.fill(colour, 46),
        );
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(5.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(12.0),
        palette.text,
    );
}

/// A headline figure with an optional gauge, used across the top of most tabs.
#[derive(Debug)]
pub struct Headline<'a> {
    pub label: &'a str,
    /// The large primary figure.
    pub value: String,
    pub support: String,
    /// Percentage to draw in the ring, with a one-word caption for it.
    pub gauge: Option<(f64, &'a str)>,
    pub colour: Color32,
    pub hint: &'a str,
}

/// Diameter of a headline gauge.
const GAUGE_SIZE: f32 = 84.0;
/// Width reserved for the gauge in *every* column, gauge or not.
const GAUGE_SLOT: f32 = GAUGE_SIZE + 14.0;
/// Fixed heights for the three text rows, so they line up across the row.
const LABEL_HEIGHT: f32 = 15.0;
const VALUE_HEIGHT: f32 = 32.0;
const SUPPORT_HEIGHT: f32 = 32.0;

/// Render a row of headline figures.
///
/// Every column reserves the same gauge slot and the same three text rows, so
/// the labels, figures and supporting lines all sit on exactly the same
/// baselines right across the card. When a gauge is present it carries the
/// percentage, so the figure beside it says something different rather than
/// repeating the same number twice.
pub fn headline_row(ui: &mut Ui, palette: &Palette, headlines: &[Headline<'_>]) {
    crate::ui::theme::card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.columns(headlines.len(), |columns| {
            for (column, headline) in columns.iter_mut().zip(headlines) {
                headline_cell(column, palette, headline);
            }
        });
    });
}

fn headline_cell(ui: &mut Ui, palette: &Palette, headline: &Headline<'_>) {
    let response = ui
        .horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // Always reserve the slot, so text starts at the same x in every
            // column whether or not this one has a gauge.
            let (slot, _) = ui.allocate_exact_size(vec2(GAUGE_SLOT, GAUGE_SIZE), Sense::hover());
            if let Some((percent, caption)) = headline.gauge {
                let square = Rect::from_min_size(slot.min, Vec2::splat(GAUGE_SIZE));
                widgets::ring_gauge_at(
                    ui.painter(),
                    palette,
                    square,
                    percent,
                    headline.colour,
                    &format::percent_whole(percent),
                    caption,
                );
            }

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(space::TIGHT);

                fixed_row(ui, LABEL_HEIGHT, |ui| {
                    ui.label(
                        RichText::new(headline.label.to_uppercase())
                            .size(10.5)
                            .strong()
                            .color(palette.text_faint),
                    );
                });

                // A missing reading is a statement, not a result, so it is
                // never given the accent colour.
                let is_missing = headline.value == format::UNAVAILABLE;
                fixed_row(ui, VALUE_HEIGHT, |ui| {
                    ui.label(
                        RichText::new(&headline.value)
                            .size(if headline.gauge.is_some() { 20.0 } else { 26.0 })
                            .strong()
                            .color(if is_missing { palette.text_faint } else { headline.colour }),
                    );
                });

                fixed_row(ui, SUPPORT_HEIGHT, |ui| {
                    ui.label(RichText::new(&headline.support).size(11.5).color(palette.text_muted));
                });
            });
        })
        .response;

    if !headline.hint.is_empty() {
        response.on_hover_text(headline.hint);
    }
}

/// Lay out one line of a headline in a slot of exactly `height` points.
fn fixed_row(ui: &mut Ui, height: f32, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_height(height);
            add(ui);
        },
    );
}

/// Explain a temperature with a colour and a verdict badge.
pub fn temperature_line(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    celsius: Option<f64>,
    kind: format::ThermalKind,
    hint: &str,
) {
    match celsius {
        Some(value) => widgets::metric_row_coloured(
            ui,
            palette,
            label,
            &format!("{} · {}", format::celsius(value), format::temperature_verdict(value, kind)),
            palette.thermal(value, kind),
            hint,
        ),
        None => widgets::metric_row(ui, palette, label, format::UNAVAILABLE, hint),
    }
}
