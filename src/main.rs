//! Rivet Files — a Nemo-style file manager in Rust (egui), with no GTK or Qt.
//!
//! Usage: rivet [FOLDER] [--no-decorations | --decorations] [--screenshot FILE.png]

mod app;
mod bench;
mod builtin_icons;
mod fonts;
mod fsops;
mod icons;
mod launch;
mod desktop;
mod mime;
mod model;
mod palette;
mod places;
mod settings;
mod thumbs;

use eframe::egui;
use std::path::PathBuf;

/// When the process started (for RIVET_FRAME_STATS).
pub static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn main() -> eframe::Result {
    STARTED.get_or_init(std::time::Instant::now);
    let mut screenshot: Option<PathBuf> = None;
    let mut start: Option<PathBuf> = None;
    let mut decorations: Option<bool> = None;
    let mut view: Option<String> = None;
    let mut tree = false;
    let mut hide_menubar = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => screenshot = args.next().map(PathBuf::from),
            "--no-decorations" | "--csd" => decorations = Some(false),
            "--decorations" => decorations = Some(true),
            "--view" => view = args.next(),
            "--tree" => tree = true,
            "--hide-menubar" => hide_menubar = true,
            "--home" => start = Some(places::home()),
            "--bench" => {
                bench::run(args.by_ref().map(PathBuf::from).collect());
                return Ok(());
            }
            "--install-desktop-entry" => {
                match desktop::install() {
                    Ok(p) => println!("Installed {}", p.display()),
                    Err(e) => eprintln!("Could not install the desktop entry: {e}"),
                }
                return Ok(());
            }
            "-V" | "--version" => {
                println!("Rivet Files {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-h" | "--help" => {
                println!("Usage: rivet [FOLDER] [--no-decorations | --decorations] [--view icons|list|compact] [--tree] [--hide-menubar] [--home] [--install-desktop-entry] [--screenshot FILE.png]");
                return Ok(());
            }
            _ if a.starts_with("--screenshot=") => screenshot = Some(PathBuf::from(&a["--screenshot=".len()..])),
            _ => {
                let p = if let Some(s) = a.strip_prefix("file://") {
                    PathBuf::from(places::percent_decode(s))
                } else if a == "~" {
                    places::home()
                } else if let Some(r) = a.strip_prefix("~/") {
                    places::home().join(r)
                } else {
                    PathBuf::from(&a)
                };
                start = Some(p);
            }
        }
    }

    // Checks use the default look so they are comparable from run to run.
    let mut settings = if screenshot.is_some() { settings::Settings::default() } else { settings::Settings::load() };
    // Screenshots for checks show a neutral sample folder, never the person's own files,
    // unless a folder was named on purpose.
    let mut demo: Option<PathBuf> = None;
    if screenshot.is_some() && start.is_none() && std::env::var_os("RIVET_REAL_HOME").is_none() {
        demo = places::make_demo_dir();
        start = demo.clone();
    }
    if let Some(d) = decorations {
        settings.decorations = d;
    }
    // Test hook for checks: try a toolbar layout without touching saved settings.
    if let Ok(t) = std::env::var("RIVET_TOOLBAR") {
        settings.toolbar = settings::parse_toolbar(&t);
    }
    if std::env::var("RIVET_ICONS").as_deref() == Ok("builtin") {
        settings.builtin_icons = true;
    }
    if let Ok(f) = std::env::var("RIVET_FONT") {
        settings.ui_font = f;
    }
    if let Some(sz) = std::env::var("RIVET_FONT_SIZE").ok().and_then(|v| v.parse().ok()) {
        settings.ui_font_size = sz;
    }
    if let Ok(c) = std::env::var("RIVET_ICON_COLOR") {
        settings.icon_color = settings::IconColor::from_id(&c);
    }
    if hide_menubar {
        settings.show_menubar = false;
    }
    if tree {
        settings.show_sidebar = true;
        settings.sidebar_tree = true;
    }
    match view.as_deref() {
        Some("list") => settings.view = settings::ViewMode::List,
        Some("compact") => settings.view = settings::ViewMode::Compact,
        Some("icons") => settings.view = settings::ViewMode::Icons,
        _ => {}
    }
    if screenshot.is_some() {
        // Checks always use a predictable size and the default look.
        settings.window_w = 1100.0;
        settings.window_h = 700.0;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Rivet")
        .with_app_id("rivet-files")
        .with_inner_size([settings.window_w, settings.window_h])
        .with_min_inner_size([520.0, 340.0])
        .with_decorations(settings.decorations);
    if let Some(img) = icons::render_svg(desktop::ICON_SVG.as_bytes(), 128) {
        let rgba = img.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
        viewport = viewport.with_icon(egui::IconData { rgba, width: 128, height: 128 });
    }
    if screenshot.is_some() {
        // A fixed size (min = max) so tiling window managers float the window at
        // exactly this size instead of stretching it to fill a tall tile.
        let size = [settings.window_w, settings.window_h];
        viewport = viewport.with_active(false).with_min_inner_size(size).with_max_inner_size(size).with_resizable(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let options = eframe::NativeOptions { viewport, persist_window: false, ..Default::default() };

    eframe::run_native(
        "Rivet",
        options,
        Box::new(move |cc| Ok(Box::new(app::RivetApp::new(cc, settings, start, screenshot, demo)))),
    )
}
