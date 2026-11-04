//! Application shell: window chrome, navigation, settings and the frame loop.

use std::time::Duration;

use egui::{Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, StrokeKind, vec2};

use crate::{
    metrics::{Snapshot, collector::Collector, history::History},
    ui::{
        format,
        theme::{self, Accent, Palette, Theme, radius, space},
        widgets,
    },
};

/// The five top-level views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tab {
    Overview,
    Gpu,
    Cpu,
    Memory,
    Storage,
}

impl Tab {
    pub const ALL: [Self; 5] = [Self::Overview, Self::Gpu, Self::Cpu, Self::Memory, Self::Storage];

    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Gpu => "Graphics",
            Self::Cpu => "Processor",
            Self::Memory => "Memory",
            Self::Storage => "Storage",
        }
    }

    /// One-line description shown under the page title.
    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Overview => "How the whole machine is doing, right now",
            Self::Gpu => "Graphics card load, video memory, clocks and power",
            Self::Cpu => "Processor load per thread, clock speeds and temperature",
            Self::Memory => "What is using your RAM, and how much is really free",
            Self::Storage => "Drive activity, speeds, capacity and health",
        }
    }

    pub fn accent(self) -> Accent {
        match self {
            Self::Overview => Accent::Neutral,
            Self::Gpu => Accent::Gpu,
            Self::Cpu => Accent::Cpu,
            Self::Memory => Accent::Memory,
            Self::Storage => Accent::Storage,
        }
    }
}

/// User preferences, persisted between runs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub theme: Theme,
    /// Milliseconds between samples.
    pub refresh_ms: u64,
    /// Seconds of history kept in every graph.
    pub history_secs: f64,
    pub ui_scale: f32,
    pub tab: Tab,
    /// Show the per-thread heat grid on the processor tab.
    pub show_core_grid: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Midnight,
            refresh_ms: 1_000,
            history_secs: 60.0,
            ui_scale: 1.0,
            tab: Tab::Overview,
            show_core_grid: true,
        }
    }
}

/// Selectable refresh rates, with an explanation of the trade-off.
const REFRESH_CHOICES: [(u64, &str, &str); 5] = [
    (250, "4 times a second", "Smoothest graphs, slightly more CPU use"),
    (500, "Twice a second", "Responsive, still very light"),
    (1_000, "Once a second", "Recommended - matches most monitoring tools"),
    (2_000, "Every 2 seconds", "Relaxed, almost no overhead"),
    (5_000, "Every 5 seconds", "Long-term watching"),
];

/// Selectable history windows.
const HISTORY_CHOICES: [(f64, &str); 4] = [
    (30.0, "Last 30 seconds"),
    (60.0, "Last minute"),
    (300.0, "Last 5 minutes"),
    (900.0, "Last 15 minutes"),
];

pub struct PerfView {
    settings: Settings,
    collector: Collector,
    history: History,
    latest: Option<Snapshot>,
    /// Kept so graphs can show a frozen view while paused.
    frames_without_data: u32,
    /// The page currently named in the window title.
    shown_tab: Option<Tab>,
    show_settings: bool,
    about_open: bool,
}

impl std::fmt::Debug for PerfView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerfView").field("settings", &self.settings).finish_non_exhaustive()
    }
}

impl PerfView {
    pub fn new(cc: &eframe::CreationContext<'_>, overrides: Option<Settings>) -> Self {
        let settings = overrides.unwrap_or_else(|| {
            cc.storage
                .and_then(|storage| eframe::get_value::<Settings>(storage, eframe::APP_KEY))
                .unwrap_or_default()
        });

        theme::apply(&cc.egui_ctx, settings.theme, settings.ui_scale);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let collector =
            Collector::spawn(Duration::from_millis(settings.refresh_ms), cc.egui_ctx.clone());
        let history = History::new(settings.history_secs, 1000.0 / settings.refresh_ms as f64);

        Self {
            settings,
            collector,
            history,
            latest: None,
            frames_without_data: 0,
            shown_tab: None,
            show_settings: false,
            about_open: false,
        }
    }

    fn palette(&self) -> Palette {
        Palette::for_theme(self.settings.theme)
    }

    fn apply_refresh_change(&mut self, ctx: &egui::Context) {
        self.collector.set_interval(Duration::from_millis(self.settings.refresh_ms));
        self.history.resize(self.settings.history_secs, 1000.0 / self.settings.refresh_ms as f64);
        ctx.request_repaint();
    }

    /// Live value shown next to each entry in the navigation rail.
    fn nav_value(&self, tab: Tab) -> Option<String> {
        let snapshot = self.latest.as_ref()?;
        Some(match tab {
            Tab::Overview => format::percent_whole(snapshot.cpu.usage_percent),
            Tab::Cpu => format::percent_whole(snapshot.cpu.usage_percent),
            Tab::Memory => format::percent_whole(snapshot.memory.used_percent()),
            Tab::Gpu => snapshot
                .primary_gpu()
                .and_then(|g| g.utilization_percent)
                .map(format::percent_whole)?,
            Tab::Storage => {
                format::bytes_per_sec(snapshot.total_disk_read() + snapshot.total_disk_write())
            }
        })
    }

    fn nav_fraction(&self, tab: Tab) -> f64 {
        let Some(snapshot) = self.latest.as_ref() else { return 0.0 };
        match tab {
            Tab::Overview | Tab::Cpu => snapshot.cpu.usage_percent / 100.0,
            Tab::Memory => snapshot.memory.used_percent() / 100.0,
            Tab::Gpu => {
                snapshot.primary_gpu().and_then(|g| g.utilization_percent).unwrap_or(0.0) / 100.0
            }
            Tab::Storage => {
                snapshot.disks.iter().map(|d| d.busy_percent).fold(0.0, f64::max) / 100.0
            }
        }
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();

        ui.add_space(space::BASE);
        ui.horizontal(|ui| {
            ui.add_space(space::BASE);
            ui.vertical(|ui| {
                ui.label(RichText::new("perfview").size(18.0).strong().color(palette.text));
                ui.label(
                    RichText::new("Live system performance").size(10.5).color(palette.text_faint),
                );
            });
        });
        ui.add_space(space::LOOSE);

        for tab in Tab::ALL {
            let selected = self.settings.tab == tab;
            let accent = palette.accent(tab.accent());
            let accent = if tab == Tab::Overview { palette.text } else { accent };

            let height = 46.0;
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());

            if selected {
                ui.painter().rect_filled(rect, radius::CONTROL, palette.fill(accent, 26));
                // Accent bar marking the active page.
                ui.painter().rect_filled(
                    Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(3.0, height - 16.0)),
                    CornerRadius::same(2),
                    accent,
                );
            } else if response.hovered() {
                ui.painter().rect_filled(rect, radius::CONTROL, palette.surface_hover);
            }

            let text_colour = if selected { palette.text } else { palette.text_muted };
            ui.painter().text(
                rect.min + vec2(16.0, 10.0),
                egui::Align2::LEFT_TOP,
                tab.title(),
                egui::FontId::proportional(13.5),
                text_colour,
            );

            // Live readout on the right of the entry.
            if let Some(value) = self.nav_value(tab) {
                ui.painter().text(
                    egui::pos2(rect.max.x - 12.0, rect.min.y + 11.0),
                    egui::Align2::RIGHT_TOP,
                    value,
                    egui::FontId::proportional(12.0),
                    if selected { accent } else { palette.text_faint },
                );
            }

            // Micro-bar underneath giving an at-a-glance load reading.
            let track = Rect::from_min_size(
                egui::pos2(rect.min.x + 16.0, rect.max.y - 13.0),
                vec2(rect.width() - 28.0, 3.0),
            );
            ui.painter().rect_filled(
                track,
                CornerRadius::same(2),
                palette.fill(palette.text_faint, 30),
            );
            let fraction = self.nav_fraction(tab).clamp(0.0, 1.0) as f32;
            if fraction > 0.004 {
                ui.painter().rect_filled(
                    Rect::from_min_size(track.min, vec2(track.width() * fraction, track.height())),
                    CornerRadius::same(2),
                    accent,
                );
            }

            if response.clicked() {
                self.settings.tab = tab;
            }
            response.on_hover_text(tab.subtitle());
            ui.add_space(2.0);
        }

        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            ui.add_space(space::BASE);
            if let Some(snapshot) = &self.latest {
                ui.label(
                    RichText::new(format!(
                        "Collector overhead {:.1} ms",
                        snapshot.collection_time.as_secs_f64() * 1000.0
                    ))
                    .size(10.0)
                    .color(palette.text_faint),
                )
                .on_hover_text(
                    "How long it took to gather one full reading. Anything under a few milliseconds is negligible.",
                );
            }
            ui.add_space(space::TIGHT);
            if ui
                .add(egui::Button::new(RichText::new("About perfview").size(11.0)).frame(false))
                .clicked()
            {
                self.about_open = true;
            }
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let paused = self.collector.is_paused();

        ui.horizontal(|ui| {
            ui.add_space(space::TIGHT);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(self.settings.tab.title())
                        .size(18.0)
                        .strong()
                        .color(palette.text),
                );
                ui.label(
                    RichText::new(self.settings.tab.subtitle())
                        .size(11.5)
                        .color(palette.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(space::TIGHT);

                if ui
                    .button(RichText::new(self.settings.theme.toggled().label()).size(12.0))
                    .on_hover_text("Switch between the dark and light themes")
                    .clicked()
                {
                    self.settings.theme = self.settings.theme.toggled();
                    theme::apply(ui.ctx(), self.settings.theme, self.settings.ui_scale);
                }

                if ui
                    .button(RichText::new("Settings").size(12.0))
                    .on_hover_text("Refresh rate, graph history and display scale")
                    .clicked()
                {
                    self.show_settings = !self.show_settings;
                }

                let (label, hint) = if paused {
                    ("Resume", "Start collecting new readings again")
                } else {
                    ("Freeze", "Stop collecting so you can study the current numbers")
                };
                if ui.button(RichText::new(label).size(12.0)).on_hover_text(hint).clicked() {
                    self.collector.set_paused(!paused);
                }

                ui.add_space(space::SNUG);
                self.live_indicator(ui, &palette, paused);
            });
        });
    }

    /// A small dot and word showing whether sampling is running.
    ///
    /// Deliberately understated. A bright badge with a clock in it is the
    /// loudest thing in the header, which is wrong: it is also the least
    /// important, and it makes the header read like a desktop status bar.
    fn live_indicator(&self, ui: &mut egui::Ui, palette: &Palette, paused: bool) {
        let (colour, text, hint) = if paused {
            (
                palette.warn,
                "Frozen",
                "Sampling is paused. These figures are from the moment you froze it.",
            )
        } else {
            (palette.good, "Live", "Sampling is running.")
        };

        let response = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (dot, _) = ui.allocate_exact_size(vec2(7.0, 7.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.5, colour);
                ui.label(RichText::new(text).size(11.5).color(palette.text_muted));
            })
            .response;

        let detail = self.latest.as_ref().map_or_else(String::new, |snapshot| {
            format!("\nLast reading at {}", snapshot.taken_at.format("%H:%M:%S"))
        });
        response.on_hover_text(format!("{hint}{detail}"));
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let Some(snapshot) = &self.latest else { return };

        ui.horizontal(|ui| {
            ui.add_space(space::SNUG);
            let facts = [
                format!("{} · {}", snapshot.system.hostname, snapshot.system.distribution),
                format!("Linux kernel {}", snapshot.system.kernel_version),
                format!("Up for {}", format::duration_long(snapshot.system.uptime_secs)),
                format!(
                    "{} processes · {} threads",
                    format::count(snapshot.cpu.process_count as u64),
                    format::count(snapshot.cpu.thread_count as u64)
                ),
            ];
            for (index, fact) in facts.iter().enumerate() {
                if index > 0 {
                    ui.label(RichText::new("•").size(10.5).color(palette.text_faint));
                }
                ui.label(RichText::new(fact).size(10.5).color(palette.text_faint));
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(space::SNUG);
                let interval = REFRESH_CHOICES
                    .iter()
                    .find(|(ms, _, _)| *ms == self.settings.refresh_ms)
                    .map_or("Custom", |(_, label, _)| label);
                ui.label(
                    RichText::new(format!(
                        "Last reading {} · refreshing {} · press 1-5 to change page",
                        snapshot.taken_at.format("%H:%M:%S"),
                        interval.to_lowercase()
                    ))
                    .size(10.5)
                    .color(palette.text_faint),
                );
            });
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let palette = self.palette();
        let mut open = true;
        let mut refresh_changed = false;

        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(330.0)
            .anchor(egui::Align2::RIGHT_TOP, vec2(-16.0, 64.0))
            .show(ctx, |ui| {
                widgets::section_header(
                    ui,
                    &palette,
                    "How often to take a reading",
                    "Faster updates make graphs smoother but use a little more power.",
                );
                ui.add_space(space::SNUG);
                for (ms, label, hint) in REFRESH_CHOICES {
                    if ui
                        .radio_value(&mut self.settings.refresh_ms, ms, label)
                        .on_hover_text(hint)
                        .changed()
                    {
                        refresh_changed = true;
                    }
                }

                ui.add_space(space::BASE);
                widgets::section_header(
                    ui,
                    &palette,
                    "How much history to show",
                    "The time span covered by every graph.",
                );
                ui.add_space(space::SNUG);
                for (secs, label) in HISTORY_CHOICES {
                    if ui.radio_value(&mut self.settings.history_secs, secs, label).changed() {
                        refresh_changed = true;
                    }
                }

                ui.add_space(space::BASE);
                widgets::section_header(
                    ui,
                    &palette,
                    "Display size",
                    "Scale the whole interface up or down.",
                );
                ui.add_space(space::SNUG);
                let mut scale = self.settings.ui_scale;
                if ui
                    .add(
                        egui::Slider::new(&mut scale, 0.8..=1.6)
                            .step_by(0.05)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    )
                    .changed()
                {
                    self.settings.ui_scale = scale;
                    theme::apply(ctx, self.settings.theme, scale);
                }

                ui.add_space(space::BASE);
                ui.checkbox(&mut self.settings.show_core_grid, "Show the per-thread heat grid")
                    .on_hover_text("A tile per logical processor on the Processor page.");

                ui.add_space(space::BASE);
                if ui.button("Clear all graph history").clicked() {
                    self.history.clear();
                }
            });

        if refresh_changed {
            self.apply_refresh_change(ctx);
        }
        self.show_settings = open;
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let palette = self.palette();
        let mut open = true;

        egui::Window::new("About perfview")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("perfview").size(22.0).strong().color(palette.text));
                ui.label(
                    RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                        .size(12.0)
                        .color(palette.text_muted),
                );
                ui.add_space(space::BASE);
                ui.label(
                    RichText::new(
                        "A live system performance viewer for Linux.\n\nReadings are labelled in full and converted to familiar units. Hover any value for a description of what it measures.",
                    )
                    .size(12.5)
                    .color(palette.text_muted),
                );
                ui.add_space(space::BASE);
                ui.hyperlink_to("Source code on GitHub", env!("CARGO_PKG_REPOSITORY"));
                ui.add_space(space::SNUG);
                ui.label(
                    RichText::new("Released under the MIT licence.")
                        .size(11.0)
                        .color(palette.text_faint),
                );
            });

        self.about_open = open;
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            for (index, tab) in Tab::ALL.iter().enumerate() {
                let key = match index {
                    0 => egui::Key::Num1,
                    1 => egui::Key::Num2,
                    2 => egui::Key::Num3,
                    3 => egui::Key::Num4,
                    _ => egui::Key::Num5,
                };
                if input.key_pressed(key) {
                    self.settings.tab = *tab;
                }
            }
            if input.key_pressed(egui::Key::Space) {
                self.collector.set_paused(!self.collector.is_paused());
            }
        });
    }
}

impl eframe::App for PerfView {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    /// We persist our own [`Settings`] explicitly. Letting egui also restore
    /// its widget memory means a stale scroll position from a previous run
    /// reappears on startup, which looks like a bug to the user.
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if let Some(snapshot) = self.collector.poll() {
            self.history.ingest(&snapshot);
            self.latest = Some(snapshot);
            self.frames_without_data = 0;
        } else {
            self.frames_without_data = self.frames_without_data.saturating_add(1);
        }

        self.handle_shortcuts(&ctx);

        // Name the current page in the window title, so it is identifiable in
        // a taskbar, window switcher or screenshot.
        if self.shown_tab != Some(self.settings.tab) {
            self.shown_tab = Some(self.settings.tab);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "perfview — {}",
                self.settings.tab.title()
            )));
        }

        let palette = self.palette();

        egui::Panel::left("navigation")
            .exact_size(198.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(palette.chrome)
                    .inner_margin(egui::Margin::symmetric(10, 0)),
            )
            .show(ui, |ui| self.navigation(ui));

        egui::Panel::top("title_bar")
            .exact_size(62.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(palette.chrome)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| {
                self.top_bar(ui);
            });

        egui::Panel::bottom("status_bar")
            .exact_size(26.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new().fill(palette.chrome).inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.base).inner_margin(egui::Margin::same(14)))
            .show(ui, |ui| {
                let Some(snapshot) = self.latest.clone() else {
                    waiting_state(ui, &palette);
                    return;
                };

                egui::ScrollArea::vertical()
                    // Each page remembers where you left it, and lands at the
                    // top the first time you open it.
                    .id_salt(self.settings.tab.title())
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let tab_ctx = TabContext {
                            snapshot: &snapshot,
                            history: &self.history,
                            palette: &palette,
                            settings: &self.settings,
                            gpu_notice: self.collector.gpu_notice.as_deref(),
                            has_gpu: self.collector.has_gpu,
                        };
                        match self.settings.tab {
                            Tab::Overview => crate::ui::tabs::overview::show(ui, &tab_ctx),
                            Tab::Gpu => crate::ui::tabs::gpu::show(ui, &tab_ctx),
                            Tab::Cpu => crate::ui::tabs::cpu::show(ui, &tab_ctx),
                            Tab::Memory => crate::ui::tabs::memory::show(ui, &tab_ctx),
                            Tab::Storage => crate::ui::tabs::storage::show(ui, &tab_ctx),
                        }
                    });
            });

        self.settings_window(&ctx);
        self.about_window(&ctx);

        // Repaint shortly before the next sample is due, so the clock in the
        // header stays live without spinning the GPU.
        ctx.request_repaint_after(Duration::from_millis(self.settings.refresh_ms.min(500)));
    }
}

/// Everything a tab needs to render itself.
pub struct TabContext<'a> {
    pub snapshot: &'a Snapshot,
    pub history: &'a History,
    pub palette: &'a Palette,
    pub settings: &'a Settings,
    pub gpu_notice: Option<&'a str>,
    pub has_gpu: bool,
}

impl std::fmt::Debug for TabContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabContext").finish_non_exhaustive()
    }
}

impl TabContext<'_> {
    /// Shorthand for the graph time window.
    pub fn window(&self) -> f64 {
        self.settings.history_secs
    }
}

fn waiting_state(ui: &mut egui::Ui, palette: &Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space(120.0);
        ui.label(RichText::new("Taking the first reading…").size(16.0).color(palette.text));
        ui.add_space(space::SNUG);
        ui.label(
            RichText::new(
                "Load figures need two samples to compare, so the first set of numbers appears in a moment.",
            )
            .size(12.0)
            .color(palette.text_muted),
        );
        ui.add_space(space::LOOSE);
        ui.spinner();
    });
}

/// Draw a card with a title, subtitle and body. Used by every tab.
pub fn card(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: &str,
    subtitle: &str,
    body: impl FnOnce(&mut egui::Ui),
) {
    theme::card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::section_header(ui, palette, title, subtitle);
        ui.add_space(space::BASE);
        body(ui);
    });
}

/// One card in a [`card_row`].
///
/// The body is `FnMut` because it is laid out twice: once invisibly to measure
/// it, and once for real at the agreed height.
pub struct CardSpec<'a> {
    pub title: String,
    pub subtitle: String,
    pub body: &'a mut dyn FnMut(&mut egui::Ui),
}

impl std::fmt::Debug for CardSpec<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardSpec").field("title", &self.title).finish_non_exhaustive()
    }
}

impl<'a> CardSpec<'a> {
    pub fn new(
        title: impl Into<String>,
        subtitle: impl Into<String>,
        body: &'a mut dyn FnMut(&mut egui::Ui),
    ) -> Self {
        Self { title: title.into(), subtitle: subtitle.into(), body }
    }
}

/// Render a row of cards side by side, all the same height.
///
/// Ragged card bottoms look sloppy, and worse, they stop the rows of figures
/// inside them from lining up across the row. To avoid that, the whole row is
/// laid out once invisibly to find the tallest card, then drawn for real with
/// every card stretched to that height.
pub fn card_row(ui: &mut egui::Ui, palette: &Palette, cards: &mut [CardSpec<'_>]) {
    let count = cards.len();
    if count == 0 {
        return;
    }

    // Sizing pass. The separate id salt keeps these throwaway widgets from
    // clashing with the real ones drawn moments later in the same frame.
    let mut tallest: f32 = 0.0;
    {
        let mut probe = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("card_row_sizing")
                .max_rect(ui.available_rect_before_wrap())
                .sizing_pass()
                .invisible(),
        );
        probe.columns(count, |columns| {
            for (column, card) in columns.iter_mut().zip(cards.iter_mut()) {
                draw_card(column, palette, card, 0.0);
                tallest = tallest.max(column.min_rect().height());
            }
        });
    }

    ui.columns(count, |columns| {
        for (column, card) in columns.iter_mut().zip(cards.iter_mut()) {
            draw_card(column, palette, card, tallest);
        }
    });
}

fn draw_card(ui: &mut egui::Ui, palette: &Palette, card: &mut CardSpec<'_>, min_height: f32) {
    theme::card(palette).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if min_height > 0.0 {
            // The frame grows with its contents, so forcing the body taller is
            // what makes the card itself match its neighbours.
            ui.set_min_height(min_height - theme::CARD_PADDING * 2.0);
        }
        widgets::section_header(ui, palette, &card.title, &card.subtitle);
        ui.add_space(space::BASE);
        (card.body)(ui);
    });
}

/// A thin horizontal rule used to separate groups inside a card.
pub fn divider(ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(space::SNUG);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, palette.outline),
        StrokeKind::Middle,
    );
    ui.add_space(space::SNUG);
}

/// Lay out children in equal-width columns with consistent gutters.
pub fn columns(ui: &mut egui::Ui, count: usize, body: impl FnOnce(&mut [egui::Ui])) {
    ui.columns(count, body);
}

/// Colour helper used by tabs that need the accent for the current page.
pub fn accent_colour(palette: &Palette, accent: Accent) -> Color32 {
    palette.accent(accent)
}
