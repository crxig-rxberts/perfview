//! Visual design system: one palette, applied consistently everywhere.
//!
//! Each hardware domain owns a colour (CPU is sky blue, GPU is green, memory is
//! violet, storage is amber, network is indigo). Once learned, the colour alone
//! tells the user what they are looking at, on every tab and every graph.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

/// Which overall look to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    /// Deep navy, designed to sit behind a game on a second monitor.
    #[default]
    Midnight,
    /// High-contrast light theme for bright rooms and screenshots.
    Daylight,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Self::Midnight => "Midnight",
            Self::Daylight => "Daylight",
        }
    }

    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Midnight => Self::Daylight,
            Self::Daylight => Self::Midnight,
        }
    }
}

/// Every colour in the application.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Window background, behind everything.
    pub base: Color32,
    /// Side rail and top bar.
    pub chrome: Color32,
    /// Card background.
    pub surface: Color32,
    /// Card background when hovered.
    pub surface_hover: Color32,
    /// Inset wells inside cards, e.g. graph backgrounds.
    pub well: Color32,
    /// Hairline borders.
    pub outline: Color32,

    pub text: Color32,
    /// Labels and units.
    pub text_muted: Color32,
    /// Captions and footnotes.
    pub text_faint: Color32,

    pub cpu: Color32,
    pub gpu: Color32,
    pub memory: Color32,
    pub storage: Color32,
    pub network: Color32,

    pub good: Color32,
    pub warn: Color32,
    pub danger: Color32,
    /// Secondary tint used for "read" vs "write" style pairs.
    pub alternate: Color32,
}

/// A hardware domain, used to pick an accent colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accent {
    Cpu,
    Gpu,
    Memory,
    Storage,
    Network,
    Neutral,
}

impl Palette {
    pub fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Midnight => Self {
                base: Color32::from_rgb(0x0A, 0x0D, 0x14),
                chrome: Color32::from_rgb(0x0F, 0x13, 0x1C),
                surface: Color32::from_rgb(0x15, 0x1A, 0x26),
                surface_hover: Color32::from_rgb(0x1B, 0x22, 0x30),
                well: Color32::from_rgb(0x0E, 0x12, 0x1B),
                outline: Color32::from_rgb(0x25, 0x2D, 0x3E),
                text: Color32::from_rgb(0xE6, 0xEB, 0xF5),
                text_muted: Color32::from_rgb(0x95, 0xA0, 0xB5),
                text_faint: Color32::from_rgb(0x66, 0x70, 0x85),
                cpu: Color32::from_rgb(0x38, 0xBD, 0xF8),
                gpu: Color32::from_rgb(0x4A, 0xDE, 0x80),
                memory: Color32::from_rgb(0xA7, 0x8B, 0xFA),
                storage: Color32::from_rgb(0xFB, 0xBF, 0x24),
                network: Color32::from_rgb(0x60, 0xA5, 0xFA),
                good: Color32::from_rgb(0x4A, 0xDE, 0x80),
                warn: Color32::from_rgb(0xFB, 0xBF, 0x24),
                danger: Color32::from_rgb(0xF8, 0x71, 0x71),
                alternate: Color32::from_rgb(0xF4, 0x72, 0xB6),
            },
            Theme::Daylight => Self {
                base: Color32::from_rgb(0xF4, 0xF6, 0xFA),
                chrome: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                surface_hover: Color32::from_rgb(0xF0, 0xF3, 0xF9),
                well: Color32::from_rgb(0xF7, 0xF9, 0xFC),
                outline: Color32::from_rgb(0xDD, 0xE3, 0xED),
                text: Color32::from_rgb(0x10, 0x16, 0x24),
                text_muted: Color32::from_rgb(0x55, 0x60, 0x75),
                text_faint: Color32::from_rgb(0x8A, 0x94, 0xA8),
                cpu: Color32::from_rgb(0x02, 0x84, 0xC7),
                gpu: Color32::from_rgb(0x15, 0x9E, 0x4B),
                memory: Color32::from_rgb(0x7C, 0x3A, 0xED),
                storage: Color32::from_rgb(0xB4, 0x53, 0x09),
                network: Color32::from_rgb(0x25, 0x63, 0xEB),
                good: Color32::from_rgb(0x15, 0x9E, 0x4B),
                warn: Color32::from_rgb(0xB4, 0x53, 0x09),
                danger: Color32::from_rgb(0xDC, 0x26, 0x26),
                alternate: Color32::from_rgb(0xDB, 0x27, 0x77),
            },
        }
    }

    pub fn accent(&self, accent: Accent) -> Color32 {
        match accent {
            Accent::Cpu => self.cpu,
            Accent::Gpu => self.gpu,
            Accent::Memory => self.memory,
            Accent::Storage => self.storage,
            Accent::Network => self.network,
            Accent::Neutral => self.text_muted,
        }
    }

    /// Green below 60%, amber to 85%, red beyond. Used for every meter so the
    /// user can read severity at a glance without reading the number.
    pub fn severity(&self, percent: f64) -> Color32 {
        match percent {
            p if p < 60.0 => self.good,
            p if p < 85.0 => self.warn,
            _ => self.danger,
        }
    }

    /// Severity for a temperature reading against its domain's limits.
    pub fn thermal(&self, celsius: f64, kind: crate::ui::format::ThermalKind) -> Color32 {
        use crate::ui::format::ThermalKind;
        let (warm, hot) = match kind {
            ThermalKind::Cpu => (75.0, 90.0),
            ThermalKind::Gpu => (75.0, 85.0),
            ThermalKind::Drive => (55.0, 70.0),
        };
        match celsius {
            c if c < warm => self.good,
            c if c < hot => self.warn,
            _ => self.danger,
        }
    }

    /// A translucent version of a colour, for graph fills.
    pub fn fill(&self, colour: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha)
    }
}

/// Corner radii, kept consistent so the UI feels like one object.
pub mod radius {
    use egui::CornerRadius;

    pub const CARD: CornerRadius = CornerRadius::same(12);
    pub const CONTROL: CornerRadius = CornerRadius::same(8);
    pub const PILL: CornerRadius = CornerRadius::same(255);
}

/// Spacing scale, in points.
pub mod space {
    pub const TIGHT: f32 = 5.0;
    pub const SNUG: f32 = 10.0;
    pub const BASE: f32 = 14.0;
    pub const LOOSE: f32 = 22.0;
}

/// Inner padding of a card, in points. Shared so that a card's content height
/// can be worked back from its outer height.
pub const CARD_PADDING: f32 = 18.0;

/// Minimum height of a label/value row. Fixed so that rows line up across
/// cards sitting side by side.
pub const ROW_HEIGHT: f32 = 26.0;

/// Apply the palette to egui's global style.
pub fn apply(ctx: &egui::Context, theme: Theme, ui_scale: f32) {
    let palette = Palette::for_theme(theme);

    ctx.set_theme(match theme {
        Theme::Midnight => egui::ThemePreference::Dark,
        Theme::Daylight => egui::ThemePreference::Light,
    });

    let text_styles: std::collections::BTreeMap<_, _> = [
        (TextStyle::Heading, FontId::new(21.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();

    ctx.all_styles_mut(|style| {
        style.text_styles.clone_from(&text_styles);

        let mut visuals = if theme == Theme::Midnight { Visuals::dark() } else { Visuals::light() };
        visuals.panel_fill = palette.base;
        visuals.window_fill = palette.surface;
        visuals.extreme_bg_color = palette.well;
        visuals.faint_bg_color = palette.surface_hover;
        visuals.override_text_color = Some(palette.text);
        visuals.window_corner_radius = radius::CARD;
        visuals.menu_corner_radius = radius::CONTROL;
        visuals.window_stroke = Stroke::new(1.0, palette.outline);
        visuals.selection.bg_fill = palette.fill(palette.cpu, 60);
        visuals.selection.stroke = Stroke::new(1.0, palette.cpu);
        visuals.hyperlink_color = palette.cpu;

        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = radius::CONTROL;
            widget.bg_stroke = Stroke::new(1.0, palette.outline);
            widget.fg_stroke = Stroke::new(1.0, palette.text);
        }
        visuals.widgets.noninteractive.bg_fill = palette.surface;
        visuals.widgets.noninteractive.weak_bg_fill = palette.surface;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_muted);
        visuals.widgets.inactive.bg_fill = palette.surface;
        visuals.widgets.inactive.weak_bg_fill = palette.surface;
        visuals.widgets.hovered.bg_fill = palette.surface_hover;
        visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
        visuals.widgets.active.bg_fill = palette.fill(palette.cpu, 48);
        visuals.widgets.active.weak_bg_fill = palette.fill(palette.cpu, 48);

        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(space::SNUG, space::SNUG);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.menu_margin = egui::Margin::same(10);
        style.spacing.scroll.bar_width = 9.0;
        style.spacing.scroll.floating = true;
        style.spacing.interact_size.y = 28.0;
    });

    ctx.set_pixels_per_point(ui_scale);
}

/// A rounded card frame with a hairline border, used for every panel.
pub fn card(palette: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(radius::CARD)
        .inner_margin(egui::Margin::same(CARD_PADDING as i8))
}

/// An inset well, used behind graphs and tables.
pub fn well(palette: &Palette, corner: CornerRadius) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.well)
        .corner_radius(corner)
        .inner_margin(egui::Margin::symmetric(10, 10))
}
