//! Entry point: parse the command line, open the window, hand over to the app.

use clap::Parser;
use perfview::{PerfView, Settings, Tab, ui::theme::Theme};

/// A modern, human-readable live system performance viewer for Linux.
#[derive(Debug, Parser)]
#[command(name = "perfview", version, about, long_about = None)]
struct Cli {
    /// Page to open on startup.
    #[arg(long, value_enum, default_value_t = StartPage::Overview)]
    page: StartPage,

    /// Milliseconds between readings. Lower is smoother, higher is lighter.
    #[arg(long, value_name = "MILLISECONDS", default_value_t = 1000, value_parser = clap::value_parser!(u64).range(100..=60_000))]
    refresh: u64,

    /// Seconds of history kept in every graph.
    #[arg(long, value_name = "SECONDS", default_value_t = 60.0)]
    history: f64,

    /// Colour theme to start with.
    #[arg(long, value_enum, default_value_t = StartTheme::Midnight)]
    theme: StartTheme,

    /// Interface scale, where 1.0 is 100%.
    #[arg(long, value_name = "FACTOR", default_value_t = 1.0)]
    scale: f32,

    /// Window size on startup, as `WIDTHxHEIGHT`, e.g. `1600x1000`.
    #[arg(long, value_name = "WxH", value_parser = parse_size)]
    size: Option<(f32, f32)>,

    /// Ignore saved preferences and use the values given on the command line.
    #[arg(long)]
    fresh: bool,
}

/// Parse a `WIDTHxHEIGHT` window size.
fn parse_size(raw: &str) -> Result<(f32, f32), String> {
    let (width, height) = raw
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("expected WIDTHxHEIGHT, for example 1600x1000, but got `{raw}`"))?;
    let parse = |value: &str, name: &str| {
        value
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("`{value}` is not a valid {name}"))
            .and_then(|v| {
                (v >= 640.0).then_some(v).ok_or_else(|| format!("{name} must be at least 640"))
            })
    };
    Ok((parse(width, "width")?, parse(height, "height")?))
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum StartPage {
    Overview,
    Gpu,
    Cpu,
    Memory,
    Storage,
}

impl From<StartPage> for Tab {
    fn from(page: StartPage) -> Self {
        match page {
            StartPage::Overview => Self::Overview,
            StartPage::Gpu => Self::Gpu,
            StartPage::Cpu => Self::Cpu,
            StartPage::Memory => Self::Memory,
            StartPage::Storage => Self::Storage,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum StartTheme {
    Midnight,
    Daylight,
}

impl From<StartTheme> for Theme {
    fn from(theme: StartTheme) -> Self {
        match theme {
            StartTheme::Midnight => Self::Midnight,
            StartTheme::Daylight => Self::Daylight,
        }
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "perfview=info,warn".into()),
        )
        .init();

    let cli = Cli::parse();
    let overrides = cli.fresh.then(|| Settings {
        theme: cli.theme.into(),
        refresh_ms: cli.refresh,
        history_secs: cli.history.clamp(10.0, 3_600.0),
        ui_scale: cli.scale.clamp(0.6, 2.5),
        tab: cli.page.into(),
        show_core_grid: true,
    });

    let (width, height) = cli.size.unwrap_or((1_320.0, 860.0));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("perfview")
            .with_app_id("dev.crxigrxberts.perfview")
            .with_inner_size([width, height])
            .with_min_inner_size([980.0, 640.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "perfview",
        options,
        Box::new(move |cc| Ok(Box::new(PerfView::new(cc, overrides)))),
    )
}

/// Generate the window icon at runtime so the binary stays dependency-free.
///
/// It is a rounded "activity" glyph: three ascending bars in the CPU accent
/// colour on the midnight background.
fn load_icon() -> egui::IconData {
    const SIZE: usize = 64;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    // Bars as (x start, x end, y start) in pixels.
    const BARS: [(usize, usize, usize); 3] = [(12, 22, 40), (27, 37, 26), (42, 52, 14)];
    const ACCENTS: [[u8; 3]; 3] = [[0x38, 0xBD, 0xF8], [0xA7, 0x8B, 0xFA], [0x4A, 0xDE, 0x80]];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let index = (y * SIZE + x) * 4;
            // Rounded background.
            let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
            let inside = dx.abs().max(dy.abs()) < 30.0
                && (dx.abs() - 18.0).max(0.0).hypot((dy.abs() - 18.0).max(0.0)) < 12.0;
            if inside {
                rgba[index..index + 4].copy_from_slice(&[0x0F, 0x13, 0x1C, 0xFF]);
            }

            for (bar, accent) in BARS.iter().zip(ACCENTS) {
                if x >= bar.0 && x < bar.1 && y >= bar.2 && y < 50 {
                    rgba[index..index + 4]
                        .copy_from_slice(&[accent[0], accent[1], accent[2], 0xFF]);
                }
            }
        }
    }

    egui::IconData { rgba, width: SIZE as u32, height: SIZE as u32 }
}
