//! Reusable visual components.
//!
//! Everything the user sees is built from these so that a bar on the CPU tab
//! behaves and reads exactly like a bar on the storage tab.

use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};
use egui_plot::{Line, Plot, PlotPoints};

use super::{
    format,
    theme::{self, Accent, Palette, radius, space},
};

/// A heading with an optional explanatory subtitle. The subtitle is where we
/// explain what a metric actually means, in plain English.
pub fn section_header(ui: &mut Ui, palette: &Palette, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).size(15.0).strong().color(palette.text));
        if !subtitle.is_empty() {
            ui.add_space(3.0);
            ui.label(RichText::new(subtitle).size(11.5).color(palette.text_faint));
        }
    });
}

/// A small coloured pill, e.g. "Hot - check cooling".
pub fn badge(ui: &mut Ui, palette: &Palette, text: &str, colour: Color32) -> Response {
    let font = FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, colour);
    let padding = vec2(10.0, 5.0);
    let (rect, response) = ui.allocate_exact_size(galley.size() + padding * 2.0, Sense::hover());

    ui.painter().rect_filled(rect, radius::PILL, palette.fill(colour, 34));
    ui.painter().galley(rect.min + padding, galley, colour);
    response
}

/// A compact label/value pair. `hint` is shown on hover and should explain the
/// metric in full, so nothing is ever a mystery.
///
/// Every row is exactly [`theme::ROW_HEIGHT`] tall. That is what lets the
/// figures in cards sitting side by side line up with each other.
pub fn metric_row(ui: &mut Ui, palette: &Palette, label: &str, value: &str, hint: &str) {
    metric_row_coloured(ui, palette, label, value, palette.text, hint);
}

/// Same as [`metric_row`] but the value is tinted, for anything judgemental.
pub fn metric_row_coloured(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    value: &str,
    colour: Color32,
    hint: &str,
) {
    let strong = colour != palette.text;
    let response = ui
        .allocate_ui_with_layout(
            vec2(ui.available_width(), theme::ROW_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_height(theme::ROW_HEIGHT);
                ui.label(RichText::new(label).size(12.5).color(palette.text_muted));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let text = RichText::new(value).size(12.5).color(colour);
                    ui.label(if strong { text.strong() } else { text });
                });
            },
        )
        .response;

    if !hint.is_empty() {
        response.on_hover_text(hint);
    }
}

/// A plain horizontal capacity bar.
pub fn bar(ui: &mut Ui, palette: &Palette, fraction: f64, colour: Color32, height: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let corner = CornerRadius::same((height / 2.0) as u8);

    ui.painter().rect_filled(rect, corner, palette.fill(palette.text_faint, 38));
    let filled = (fraction.clamp(0.0, 1.0) as f32) * rect.width();
    if filled > 0.5 {
        let fill_rect = Rect::from_min_size(rect.min, vec2(filled.max(height), rect.height()));
        ui.painter().rect_filled(fill_rect, corner, colour);
    }
}

/// One slice of a [`stacked_bar`].
#[derive(Debug, Clone)]
pub struct Segment {
    /// What this slice is, in plain words.
    pub label: String,
    pub bytes: u64,
    pub colour: Color32,
    /// Shown on hover to explain why this memory counts the way it does.
    pub hint: String,
}

/// A stacked capacity bar plus a legend. This is how memory and disk usage are
/// shown, because a single "used" number hides what is really going on.
pub fn stacked_bar(ui: &mut Ui, palette: &Palette, total: u64, segments: &[Segment], height: f32) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let corner = CornerRadius::same((height / 2.0).min(6.0) as u8);

    ui.painter().rect_filled(rect, corner, palette.fill(palette.text_faint, 30));

    if total > 0 {
        let mut cursor = rect.min.x;
        for segment in segments {
            let span = (segment.bytes as f64 / total as f64) as f32 * rect.width();
            if span < 0.75 {
                continue;
            }
            let slice = Rect::from_min_size(pos2(cursor, rect.min.y), vec2(span, rect.height()));
            ui.painter().rect_filled(slice, corner, segment.colour);
            cursor += span;
        }
    }
    ui.painter().rect_stroke(
        rect,
        corner,
        Stroke::new(1.0, palette.fill(palette.outline, 180)),
        StrokeKind::Inside,
    );

    response.on_hover_text(
        segments
            .iter()
            .map(|s| format!("{}: {}", s.label, format::bytes(s.bytes)))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    ui.add_space(space::SNUG);
    // Legend, wrapped so it never overflows a narrow card.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::BASE, space::TIGHT);
        for segment in segments {
            let response = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                    ui.painter().rect_filled(dot, CornerRadius::same(2), segment.colour);
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            segment.label,
                            format::bytes(segment.bytes)
                        ))
                        .size(11.0)
                        .color(palette.text_muted),
                    );
                })
                .response;
            if !segment.hint.is_empty() {
                response.on_hover_text(&segment.hint);
            }
        }
    });
}

/// A circular gauge: the single most glanceable way to show a percentage.
pub fn ring_gauge(
    ui: &mut Ui,
    palette: &Palette,
    percent: f64,
    colour: Color32,
    centre_value: &str,
    caption: &str,
    diameter: f32,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    ring_gauge_at(ui.painter(), palette, rect, percent, colour, centre_value, caption);
    response.on_hover_text(format!("{} of maximum", format::percent(percent)));
}

/// Paint a ring gauge into an already-reserved square.
///
/// Headline rows reserve the same slot in every column, whether or not there
/// is a gauge to put in it, so the text beside them always starts in the same
/// place.
pub fn ring_gauge_at(
    painter: &egui::Painter,
    palette: &Palette,
    rect: Rect,
    percent: f64,
    colour: Color32,
    centre_value: &str,
    caption: &str,
) {
    let diameter = rect.width().min(rect.height());
    let centre = rect.center();
    let outer = diameter / 2.0 - 3.0;
    let thickness = (diameter * 0.11).clamp(6.0, 14.0);

    // The gauge sweeps 270 degrees, leaving a gap at the bottom so the user can
    // tell "empty" from "the ring hasn't rendered".
    const START: f32 = std::f32::consts::PI * 0.75;
    const SWEEP: f32 = std::f32::consts::PI * 1.5;

    // Rounded caps are drawn by hand: egui's polyline has none, and without
    // them a low reading renders as an ugly sliver rather than a neat dot.
    let arc = |from: f32, to: f32, colour: Color32| {
        let at = |angle: f32| centre + vec2(angle.cos() * outer, angle.sin() * outer);
        let steps = (((to - from).abs() / SWEEP * 96.0).ceil() as usize).max(2);
        let points: Vec<_> =
            (0..=steps).map(|step| at(from + (to - from) * (step as f32 / steps as f32))).collect();

        painter.circle_filled(at(from), thickness / 2.0, colour);
        painter.circle_filled(at(to), thickness / 2.0, colour);
        painter.add(egui::Shape::line(points, Stroke::new(thickness, colour)));
    };

    arc(START, START + SWEEP, palette.fill(palette.text_faint, 34));
    let fraction = (percent / 100.0).clamp(0.0, 1.0) as f32;
    if fraction > 0.0005 {
        arc(START, START + SWEEP * fraction, colour);
    }

    painter.text(
        centre - vec2(0.0, 5.0),
        egui::Align2::CENTER_CENTER,
        centre_value,
        FontId::proportional((diameter * 0.2).clamp(14.0, 26.0)),
        palette.text,
    );
    painter.text(
        centre + vec2(0.0, diameter * 0.17),
        egui::Align2::CENTER_CENTER,
        caption,
        FontId::proportional(10.5),
        palette.text_faint,
    );
}

/// How a [`graph`] should label its Y axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphUnit {
    /// Fixed 0-100 axis.
    Percent,
    /// Auto-scaled bytes per second.
    BytesPerSec,
    /// Auto-scaled bytes.
    Bytes,
    Celsius,
    Watts,
    Gigahertz,
    Megahertz,
    /// Unit-less count per second.
    Rate,
    Plain,
}

impl GraphUnit {
    fn format(self, value: f64) -> String {
        match self {
            Self::Percent => format::percent_whole(value),
            Self::BytesPerSec => format::bytes_per_sec(value),
            Self::Bytes => format::bytes(value.max(0.0) as u64),
            Self::Celsius => format::celsius(value),
            Self::Watts => format::watts(value),
            Self::Gigahertz => format!("{value:.2} GHz"),
            Self::Megahertz => format!("{value:.0} MHz"),
            Self::Rate => format::per_sec(value),
            Self::Plain => format!("{value:.2}"),
        }
    }

    /// Some readings are meaningless on a zero-based axis. A graphics card
    /// that sits between 35 °C and 50 °C looks completely flat when the axis
    /// runs to zero, so temperatures get a fixed, familiar 0-100 °C scale
    /// instead of auto-ranging.
    fn fixed_range(self) -> Option<(f64, f64)> {
        match self {
            Self::Percent => Some((0.0, 100.0)),
            Self::Celsius => Some((0.0, 100.0)),
            _ => None,
        }
    }
}

/// One line on a [`graph`].
#[derive(Debug)]
pub struct GraphLine<'a> {
    pub name: &'a str,
    pub series: &'a crate::metrics::history::Series,
    pub colour: Color32,
    /// Whether to shade the area under the line.
    pub filled: bool,
}

impl<'a> GraphLine<'a> {
    pub fn new(
        name: &'a str,
        series: &'a crate::metrics::history::Series,
        colour: Color32,
    ) -> Self {
        Self { name, series, colour, filled: true }
    }

    #[must_use]
    pub fn outline(mut self) -> Self {
        self.filled = false;
        self
    }
}

/// A live area chart.
///
/// The X axis is always "seconds ago", counted back from now, because an
/// absolute timestamp is useless on a rolling graph.
pub fn graph(
    ui: &mut Ui,
    palette: &Palette,
    id: &str,
    height: f32,
    unit: GraphUnit,
    lines: &[GraphLine<'_>],
    window_secs: f64,
) {
    let now =
        lines.iter().filter_map(|l| l.series.x_range()).map(|(_, end)| end).fold(0.0, f64::max);

    let peak = lines.iter().map(|l| l.series.max()).fold(0.0, f64::max);
    // A flat-zero series would otherwise collapse the axis to nothing and
    // leave the graph unlabelled.
    let auto_max = if peak <= 1e-9 { 1.0 } else { peak * 1.18 };
    let (y_min, y_max) = unit.fixed_range().unwrap_or((0.0, auto_max));

    let plot = Plot::new(id)
        .height(height)
        .show_axes([false, true])
        .show_grid([false, true])
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .show_background(false)
        .include_y(y_min)
        .include_y(y_max)
        .include_x(-window_secs)
        .include_x(0.0)
        .y_axis_min_width(54.0)
        .y_axis_formatter(move |mark, _| unit.format(mark.value))
        .label_formatter(move |hover| {
            let (name, point) = match hover {
                egui_plot::HoverPosition::NearDataPoint { plot_name, position, .. } => {
                    (*plot_name, *position)
                }
                egui_plot::HoverPosition::Elsewhere { position } => ("", *position),
            };
            let ago = -point.x;
            let when = if ago < 1.0 { "now".to_owned() } else { format!("{ago:.0} s ago") };
            Some(if name.is_empty() {
                format!("{}\n{when}", unit.format(point.y))
            } else {
                format!("{name}\n{}\n{when}", unit.format(point.y))
            })
        });

    // An inset well behind the plot separates the data from the card.
    super::theme::well(palette, radius::CONTROL).show(ui, |ui| {
        ui.set_width(ui.available_width());
        plot.show(ui, |plot_ui| {
            for line in lines {
                // Shift X so that "now" sits at zero and history runs negative.
                let points: Vec<[f64; 2]> =
                    line.series.points().into_iter().map(|[x, y]| [x - now, y]).collect();
                if points.is_empty() {
                    continue;
                }

                let mut plotted =
                    Line::new(line.name, PlotPoints::from(points)).color(line.colour).width(1.8);
                if line.filled {
                    // `fill` shades down to the axis and handles concave
                    // shapes correctly, which a polygon does not.
                    plotted = plotted.fill(0.0).fill_alpha(0.18);
                }
                plot_ui.line(plotted);
            }
        });
    });
}

/// A legend row for a multi-line [`graph`].
pub fn graph_legend(ui: &mut Ui, palette: &Palette, entries: &[(&str, Color32, String)]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::BASE, space::TIGHT);
        for (label, colour, value) in entries {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (dot, _) = ui.allocate_exact_size(vec2(10.0, 3.0), Sense::hover());
                ui.painter().rect_filled(dot, CornerRadius::same(2), *colour);
                ui.label(RichText::new(*label).size(11.0).color(palette.text_muted));
                ui.label(RichText::new(value).size(11.0).strong().color(palette.text));
            });
        }
    });
}

/// The per-core heat grid on the CPU tab: one tile per logical processor,
/// shaded by how busy it is.
pub fn core_grid(
    ui: &mut Ui,
    palette: &Palette,
    cores: &[crate::metrics::cpu::CoreMetrics],
    columns: usize,
) {
    let columns = columns.max(1);
    let spacing = 6.0;
    let available = ui.available_width();
    let tile = ((available - spacing * (columns - 1) as f32) / columns as f32).max(28.0);

    for row in cores.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for core in row {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(tile, tile * 0.62), Sense::hover());
                let usage = core.usage_percent.clamp(0.0, 100.0);
                let colour = palette.severity(usage);

                ui.painter().rect_filled(rect, radius::CONTROL, palette.well);
                // Fill from the bottom up, like a tiny bar chart.
                let fill_height = rect.height() * (usage / 100.0) as f32;
                if fill_height > 0.5 {
                    let fill = Rect::from_min_max(
                        pos2(rect.min.x, rect.max.y - fill_height),
                        rect.max,
                    );
                    ui.painter().rect_filled(fill, radius::CONTROL, palette.fill(colour, 150));
                }
                ui.painter().rect_stroke(
                    rect,
                    radius::CONTROL,
                    Stroke::new(1.0, palette.outline),
                    StrokeKind::Inside,
                );
                ui.painter().text(
                    rect.center() - vec2(0.0, 5.0),
                    egui::Align2::CENTER_CENTER,
                    format::percent_whole(usage),
                    FontId::proportional(12.0),
                    palette.text,
                );
                // The label sits over the fill, so it needs enough contrast to
                // stay readable on a fully saturated tile.
                ui.painter().text(
                    rect.center() + vec2(0.0, 8.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Thread {}", core.id),
                    FontId::proportional(9.0),
                    if usage > 45.0 {
                        palette.fill(palette.text, 215)
                    } else {
                        palette.text_faint
                    },
                );

                response.on_hover_text(format!(
                    "Logical processor {}\n{}\nClock speed: {}\nApplications: {}\nKernel: {}\nWaiting on disk: {}",
                    core.id,
                    core.physical_core
                        .map_or_else(|| "Physical core unknown".to_owned(), |c| format!(
                            "Lives on physical core {c}"
                        )),
                    format::hertz_opt(core.frequency_hz),
                    format::percent(core.breakdown.user + core.breakdown.nice),
                    format::percent(core.breakdown.system),
                    format::percent(core.breakdown.iowait),
                ));
            }
        });
        ui.add_space(spacing);
    }
}

/// A full-width informational strip, used for GPU driver notices and the like.
pub fn notice(ui: &mut Ui, palette: &Palette, icon: &str, text: &str, colour: Color32) {
    egui::Frame::new()
        .fill(palette.fill(colour, 22))
        .stroke(Stroke::new(1.0, palette.fill(colour, 90)))
        .corner_radius(radius::CONTROL)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(14.0).color(colour));
                ui.label(RichText::new(text).size(12.0).color(palette.text));
            });
        });
}

/// Shown when a whole feature is unavailable, with advice on how to fix it.
pub fn empty_state(ui: &mut Ui, palette: &Palette, title: &str, body: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.label(RichText::new(title).size(16.0).strong().color(palette.text));
        ui.add_space(space::SNUG);
        ui.label(RichText::new(body).size(12.5).color(palette.text_muted));
        ui.add_space(48.0);
    });
}

/// Accent colour lookup shortcut used by the tabs.
pub fn accent_of(palette: &Palette, accent: Accent) -> Color32 {
    palette.accent(accent)
}
