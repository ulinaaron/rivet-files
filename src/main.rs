//! Rivet Files — a Nemo-style file manager in Rust (egui), with no GTK or Qt.
//!
//! Usage: rivet [FOLDER] [--no-decorations | --decorations] [--screenshot FILE.png]

mod app;
mod fsops;
mod icons;
mod launch;
mod mime;
mod model;
mod palette;
mod places;
mod settings;
mod thumbs;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result {
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
            "-h" | "--help" => {
                println!("Usage: rivet [FOLDER] [--no-decorations | --decorations] [--view icons|list|compact] [--tree] [--hide-menubar] [--screenshot FILE.png]");
                return Ok(());
            }
            _ if a.starts_with("--screenshot=") => screenshot = Some(PathBuf::from(&a["--screenshot=".len()..])),
            _ => {
                let p = a.strip_prefix("file://").map(|s| PathBuf::from(places::percent_decode(s))).unwrap_or_else(|| PathBuf::from(&a));
                start = Some(p);
            }
        }
    }

    // Checks use the default look so they are comparable from run to run.
    let mut settings = if screenshot.is_some() { settings::Settings::default() } else { settings::Settings::load() };
    if let Some(d) = decorations {
        settings.decorations = d;
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
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let options = eframe::NativeOptions { viewport, persist_window: false, ..Default::default() };

    eframe::run_native(
        "Rivet",
        options,
        Box::new(move |cc| Ok(Box::new(app::RivetApp::new(cc, settings, start, screenshot)))),
    )
}
