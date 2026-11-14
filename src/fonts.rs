//! UI fonts: the desktop's font (from GTK settings / gsettings), any installed family
//! (found with fontconfig's command-line tools — no library linked), and a size.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::places;

/// Default UI text size in points (egui units) at a font scale of 1.0.
pub const BASE_SIZE: f32 = 13.5;

static SCALE_BITS: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0f32

/// Current text scale (chosen size ÷ base size), used by every hand-drawn label.
pub fn scale() -> f32 {
    f32::from_bits(SCALE_BITS.load(Ordering::Relaxed))
}

pub fn set_scale(s: f32) {
    SCALE_BITS.store(s.clamp(0.6, 2.0).to_bits(), Ordering::Relaxed);
}

/// A proportional font id that follows the chosen text size.
pub fn ui_font(size: f32) -> eframe::egui::FontId {
    eframe::egui::FontId::proportional(size * scale())
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The desktop's UI font as (family, size in points), e.g. ("Cantarell", 10.0).
pub fn desktop_font() -> Option<(String, f32)> {
    let parse = |v: &str| -> Option<(String, f32)> {
        let v = v.trim().trim_matches(|c| c == '\'' || c == '"');
        let (fam, size) = v.rsplit_once(' ')?;
        Some((fam.trim().trim_end_matches(',').to_string(), size.parse().ok()?))
    };
    let c = places::config_home();
    for f in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
        if let Ok(t) = std::fs::read_to_string(c.join(f)) {
            for l in t.lines() {
                if let Some(v) = l.trim().strip_prefix("gtk-font-name") {
                    if let Some(r) = parse(v.trim_start_matches([' ', '='])) {
                        return Some(r);
                    }
                }
            }
        }
    }
    for schema in ["org.cinnamon.desktop.interface", "org.gnome.desktop.interface"] {
        if let Some(v) = run("gsettings", &["get", schema, "font-name"]) {
            if let Some(r) = parse(&v) {
                return Some(r);
            }
        }
    }
    None
}

/// `desktop_font`, looked up once and refreshed every few seconds (theme tools can change it).
pub fn desktop_font_cached() -> Option<(String, f32)> {
    use std::sync::Mutex;
    use std::time::Instant;
    static CACHE: Mutex<Option<(Instant, Option<(String, f32)>)>> = Mutex::new(None);
    let mut g = CACHE.lock().unwrap();
    if let Some((t, v)) = g.as_ref() {
        if t.elapsed().as_secs() < 5 {
            return v.clone();
        }
    }
    let v = desktop_font();
    *g = Some((Instant::now(), v.clone()));
    v
}

/// Points (as GTK uses them) to egui units: 1pt = 4/3 px at 96 dpi.
pub fn points_to_ui(pt: f32) -> f32 {
    (pt * 4.0 / 3.0).clamp(9.0, 28.0)
}

/// Installed font families that have a regular style, sorted.
pub fn families() -> Vec<String> {
    let text = run("fc-list", &["--format", "%{family[0]}\n", ":style=Regular"]).unwrap_or_default();
    let mut v: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    v.sort_by_key(|a| a.to_lowercase());
    v.dedup();
    v
}

/// The file for a family's regular face.
pub fn file_for(family: &str) -> Option<PathBuf> {
    let pattern = format!("{family}:style=Regular");
    let out = run("fc-match", &["-f", "%{family[0]}|%{file}", &pattern])?;
    let (got, file) = out.split_once('|')?;
    // fc-match always answers; only accept it when it really found this family.
    if !got.eq_ignore_ascii_case(family) && family != "sans-serif" {
        return None;
    }
    let p = PathBuf::from(file.trim());
    if p.is_file() { Some(p) } else { None }
}

/// Installed monospace families (for the monospace font picker), sorted.
pub fn mono_families() -> Vec<String> {
    let text = run("fc-list", &["--format", "%{family[0]}\n", ":spacing=mono:style=Regular"]).unwrap_or_default();
    let mut v: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    v.sort_by_key(|a| a.to_lowercase());
    v.dedup();
    v
}

/// The monospace font file: the chosen family, else the system's "monospace".
pub fn resolve_mono(choice: &str) -> Option<(String, PathBuf)> {
    if !choice.is_empty() {
        if let Some(p) = file_for(choice) {
            return Some((choice.to_string(), p));
        }
    }
    let out = run("fc-match", &["-f", "%{family[0]}|%{file}", "monospace:style=Regular"])?;
    let (fam, file) = out.split_once('|')?;
    let p = PathBuf::from(file.trim());
    p.is_file().then(|| (fam.to_string(), p))
}

/// Fallback when fontconfig's tools are missing: look for a few common files.
fn common_sans() -> Option<PathBuf> {
    let candidates = ["NotoSans-Regular.ttf", "Cantarell-Regular.otf", "Ubuntu-R.ttf", "DejaVuSans.ttf", "LiberationSans-Regular.ttf"];
    for c in candidates {
        let mut stack = vec![(PathBuf::from("/usr/share/fonts"), 0), (places::data_home().join("fonts"), 0)];
        while let Some((d, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && depth < 3 {
                    stack.push((p, depth + 1));
                } else if e.file_name().to_string_lossy() == c {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Resolve the font file to use: the chosen family, else the desktop's, else a common sans.
pub fn resolve(choice: &str) -> Option<(String, PathBuf)> {
    if !choice.is_empty() {
        if let Some(p) = file_for(choice) {
            return Some((choice.to_string(), p));
        }
    }
    if let Some((fam, _)) = desktop_font() {
        if let Some(p) = file_for(&fam) {
            return Some((fam, p));
        }
    }
    if let Some(p) = file_for("sans-serif").or_else(common_sans) {
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Sans".into());
        return Some((name, p));
    }
    None
}
