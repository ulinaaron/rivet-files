//! Terminal mode's icons: glyphs from an installed Nerd Font, drawn as vector icons.
//!
//! Every icon Rivet asks for goes through the built-in icon names (see `builtin_icons::lookup`),
//! so one table maps them all to Nerd Font glyphs. Each glyph's outline is read from the font
//! file and turned into an SVG, then drawn by the same renderer as Rivet's own icons — so it
//! works anywhere an icon goes (buttons, rows, the file grid) and can be tinted like one.

use eframe::egui::Color32;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The glyph for a built-in icon key (Font Awesome glyphs, which every Nerd Font carries).
pub fn glyph_for(key: &str) -> Option<char> {
    let c = match key {
        "folder" => '\u{f07b}',
        "folder-home" => '\u{f015}',
        "folder-desktop" => '\u{f108}',
        "folder-documents" => '\u{f0f6}',
        "folder-download" => '\u{f019}',
        "folder-music" => '\u{f001}',
        "folder-pictures" => '\u{f03e}',
        "folder-videos" => '\u{f008}',
        "folder-templates" => '\u{f0c5}',
        "folder-public" => '\u{f0c0}',
        "network" => '\u{f0ac}',
        "recent" => '\u{f017}',
        "trash" | "trash-full" => '\u{f1f8}',
        "usb" => '\u{f287}',
        "emblem-link" => '\u{f0c1}',
        "file-unknown" => '\u{f016}',
        "file-archive" => '\u{f1c6}',
        "file-exec" => '\u{f013}',
        "file-pdf" => '\u{f1c1}',
        "file-sheet" => '\u{f1c3}',
        "file-text" => '\u{f0f6}',
        "file-code" => '\u{f1c9}',
        "drive" => '\u{f0a0}',
        "file-image" => '\u{f1c5}',
        "file-audio" => '\u{f1c7}',
        "file-video" => '\u{f1c8}',
        "file-font" => '\u{f031}',
        "file-slides" => '\u{f1c4}',
        "file-doc" => '\u{f1c2}',
        "s:back" => '\u{f060}',
        "s:forward" => '\u{f061}',
        "s:up" => '\u{f062}',
        "s:reload" => '\u{f021}',
        "s:home" => '\u{f015}',
        "s:computer" => '\u{f108}',
        "s:drive" => '\u{f0a0}',
        "s:edit" => '\u{f040}',
        "s:search" => '\u{f002}',
        "s:grid" => '\u{f009}',
        "s:list" => '\u{f03a}',
        "s:compact" => '\u{f00a}',
        "s:folder-new" => '\u{f067}',
        "s:folder" => '\u{f07b}',
        "s:add" => '\u{f067}',
        "s:terminal" => '\u{f120}',
        "s:eye" => '\u{f06e}',
        "s:trash" => '\u{f1f8}',
        "s:info" => '\u{f05a}',
        "s:star" => '\u{f005}',
        "s:sidebar" => '\u{f0db}',
        "s:tree" => '\u{f0e8}',
        "s:eject" => '\u{f052}',
        "s:min" => '\u{f068}',
        "s:max" => '\u{f2d0}',
        "s:restore" => '\u{f2d2}',
        "s:close" => '\u{f00d}',
        "s:menu" => '\u{f0c9}',
        "s:network" => '\u{f0ac}',
        "s:recent" => '\u{f017}',
        _ => return None,
    };
    Some(c)
}

/// Colours for file kinds, like a terminal's `ls` colours.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct NerdColors {
    pub folder: Color32,
    pub image: Color32,
    pub video: Color32,
    pub audio: Color32,
    pub archive: Color32,
    pub code: Color32,
    pub doc: Color32,
    pub plain: Color32,
}

impl NerdColors {
    /// Kind colours turned from the accent's hue, so they suit the desktop palette.
    pub fn from_theme(accent: Color32, fg: Color32, dark: bool) -> Self {
        let turn = |deg: f32| {
            let c = crate::builtin_icons::hue(accent, deg);
            if dark { c } else { crate::palette::mix(c, Color32::BLACK, 0.3) }
        };
        Self {
            folder: if dark { accent } else { crate::palette::mix(accent, Color32::BLACK, 0.2) },
            image: turn(70.0),
            video: turn(110.0),
            audio: turn(160.0),
            archive: turn(200.0),
            code: turn(250.0),
            doc: turn(-40.0),
            plain: crate::palette::mix(fg, if dark { Color32::BLACK } else { Color32::WHITE }, 0.2),
        }
    }

    pub fn for_key(&self, key: &str) -> Color32 {
        match key {
            k if k.starts_with("folder") => self.folder,
            "file-image" => self.image,
            "file-video" => self.video,
            "file-audio" => self.audio,
            "file-archive" => self.archive,
            "file-code" | "file-exec" => self.code,
            "file-pdf" | "file-doc" | "file-sheet" | "file-slides" => self.doc,
            _ => self.plain,
        }
    }
}

/// A Nerd Font loaded for its glyphs.
pub struct NerdFont {
    pub path: PathBuf,
    data: Arc<Vec<u8>>,
}

/// Collects an outline as SVG path data (y flipped to point down) and its bounds.
struct Pen {
    d: String,
    min: (f32, f32),
    max: (f32, f32),
}

impl Pen {
    fn pt(&mut self, x: f32, y: f32) {
        self.min = (self.min.0.min(x), self.min.1.min(-y));
        self.max = (self.max.0.max(x), self.max.1.max(-y));
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pt(x, y);
        self.d.push_str(&format!("M{x:.1} {:.1}", -y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pt(x, y);
        self.d.push_str(&format!("L{x:.1} {:.1}", -y));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.pt(x, y);
        self.d.push_str(&format!("Q{cx:.1} {:.1} {x:.1} {:.1}", -cy, -y));
    }
    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        self.pt(x, y);
        self.d.push_str(&format!("C{c0x:.1} {:.1} {c1x:.1} {:.1} {x:.1} {:.1}", -c0y, -c1y, -y));
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

impl NerdFont {
    pub fn load(path: &Path) -> Option<Self> {
        let data = std::fs::read(path).ok()?;
        let f = Self { path: path.to_path_buf(), data: Arc::new(data) };
        f.has('\u{f07b}').then_some(f)
    }

    fn font(&self) -> Option<FontRef<'_>> {
        FontRef::new(&self.data).or_else(|_| FontRef::from_index(&self.data, 0)).ok()
    }

    pub fn has(&self, ch: char) -> bool {
        self.font().and_then(|f| f.charmap().map(ch)).is_some()
    }

    /// The glyph as a square SVG icon filled with `fill`, centred with a little margin.
    pub fn svg(&self, ch: char, fill: Color32) -> Option<String> {
        let font = self.font()?;
        let gid = font.charmap().map(ch)?;
        let glyph = font.outline_glyphs().get(gid)?;
        let mut pen = Pen { d: String::new(), min: (f32::MAX, f32::MAX), max: (f32::MIN, f32::MIN) };
        glyph.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen).ok()?;
        if pen.d.is_empty() {
            return None;
        }
        let (w, h) = (pen.max.0 - pen.min.0, pen.max.1 - pen.min.1);
        let side = w.max(h) * 1.12;
        let (x, y) = (pen.min.0 - (side - w) / 2.0, pen.min.1 - (side - h) / 2.0);
        Some(format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x:.1} {y:.1} {side:.1} {side:.1}"><path fill="#{:02x}{:02x}{:02x}" d="{}"/></svg>"##,
            fill.r(),
            fill.g(),
            fill.b(),
            pen.d
        ))
    }
}

/// An installed font with Nerd Font glyphs: `preferred` if it has them, else the first one
/// fontconfig knows (mono variants first).
pub fn find(preferred: Option<&Path>) -> Option<NerdFont> {
    if let Some(f) = preferred.and_then(NerdFont::load) {
        return Some(f);
    }
    let out = std::process::Command::new("fc-list")
        .args([":charset=f07b f1c9 f060", "--format", "%{file}\n"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut files: Vec<&str> = text.lines().filter(|l| l.ends_with(".ttf") || l.ends_with(".otf")).collect();
    // Prefer a regular mono face: its glyphs are designed to sit in one cell.
    files.sort_by_key(|f| {
        let n = f.to_lowercase();
        (!n.contains("mono"), !n.contains("regular"), n.len())
    });
    files.into_iter().find_map(|f| NerdFont::load(Path::new(f)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mapped glyph exists in an installed Nerd Font (skipped when there is none).
    #[test]
    fn glyphs_exist_in_nerd_font() {
        let Some(font) = find(None) else { return };
        let keys = [
            "folder", "folder-home", "folder-desktop", "folder-documents", "folder-download", "folder-music", "folder-pictures", "folder-videos",
            "folder-templates", "folder-public", "network", "recent", "trash", "usb", "emblem-link", "file-unknown", "file-archive", "file-exec",
            "file-pdf", "file-sheet", "file-text", "file-code", "drive", "file-image", "file-audio", "file-video", "file-font", "file-slides",
            "file-doc", "s:back", "s:forward", "s:up", "s:reload", "s:home", "s:computer", "s:edit", "s:search", "s:grid", "s:list",
            "s:compact", "s:add", "s:terminal", "s:eye", "s:trash", "s:info", "s:star", "s:sidebar", "s:tree", "s:eject", "s:min", "s:max",
            "s:restore", "s:close", "s:menu", "s:network", "s:recent",
        ];
        for k in keys {
            let ch = glyph_for(k).unwrap();
            assert!(font.svg(ch, Color32::WHITE).is_some(), "{k} ({:x}) missing in {}", ch as u32, font.path.display());
        }
    }
}
