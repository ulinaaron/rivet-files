//! Live desktop colours. Theme generators such as matugen, pywal-style tools,
//! Noctalia or DankMaterialShell write GTK colour files (`@define-color name value;`).
//! Rivet reads the same files (following `@import`s), and polls them so a new
//! wallpaper/scheme restyles the window within a second. A Rivet-specific file,
//! ~/.config/rivet/colors.css, takes priority so matugen can target Rivet directly.

use eframe::egui::Color32;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::places;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Palette {
    pub accent: Option<Color32>,
    pub accent_fg: Option<Color32>,
    pub window_bg: Option<Color32>,
    pub window_fg: Option<Color32>,
    pub view_bg: Option<Color32>,
    pub view_fg: Option<Color32>,
    pub headerbar_bg: Option<Color32>,
    pub sidebar_bg: Option<Color32>,
    pub popover_bg: Option<Color32>,
    pub destructive: Option<Color32>,
    pub source: PathBuf,
}

impl Palette {
    pub fn is_dark(&self) -> bool {
        let c = self.window_bg.or(self.view_bg).unwrap_or(Color32::BLACK);
        luminance(c) < 0.5
    }
}

pub fn luminance(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// The Rivet colour file matugen (or any tool) can write to.
pub fn rivet_colors_file() -> PathBuf {
    places::config_home().join("rivet/colors.css")
}

/// Files that may hold desktop colours, most specific first.
fn candidates() -> Vec<PathBuf> {
    let c = places::config_home();
    vec![
        rivet_colors_file(),
        c.join("gtk-4.0/gtk.css"),
        c.join("gtk-4.0/colors.css"),
        c.join("gtk-3.0/gtk.css"),
        c.join("gtk-3.0/colors.css"),
    ]
}

/// Other files whose change means "the desktop theme changed" (icon theme, dark mode).
fn settings_files() -> Vec<PathBuf> {
    let c = places::config_home();
    vec![c.join("gtk-3.0/settings.ini"), c.join("gtk-4.0/settings.ini"), c.join("kdeglobals")]
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(j) => rest = &rest[i + 2 + j + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Collect `@define-color` entries from a CSS file and everything it imports.
/// A colour file that ships inside an installed GTK theme holds that theme's fixed
/// defaults; generated colours (matugen, Noctalia, ...) live in the config folder.
fn is_theme_default(path: &Path) -> bool {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = real.to_string_lossy();
    s.contains("/themes/") || s.starts_with("/usr/")
}

fn collect(path: &Path, defs: &mut HashMap<String, String>, theme_defs: &mut HashMap<String, String>, visited: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 || visited.iter().any(|v| v == path) {
        return;
    }
    visited.push(path.to_path_buf());
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let text = strip_comments(&text);
    let dir = std::fs::canonicalize(path).ok().and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .or_else(|| path.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    for stmt in text.split(';') {
        let stmt = stmt.trim();
        if let Some(rest) = stmt.strip_prefix("@import") {
            let r = rest.trim();
            let inner = r.strip_prefix("url(").map(|x| x.trim_end_matches(')')).unwrap_or(r);
            let file = inner.trim().trim_matches(|c| c == '"' || c == '\'');
            let file = file.strip_prefix("file://").unwrap_or(file);
            let p = if let Some(h) = file.strip_prefix("~/") { places::home().join(h) } else { dir.join(file) };
            // Imports are applied first so later definitions in this file override them.
            collect(&p, defs, theme_defs, visited, depth + 1);
        } else if let Some(rest) = stmt.rfind("@define-color").map(|i| &stmt[i + 13..]) {
            let rest = rest.trim();
            if let Some((name, value)) = rest.split_once(char::is_whitespace) {
                let target = if is_theme_default(path) { &mut *theme_defs } else { &mut *defs };
                target.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }
}

fn parse_color(v: &str, defs: &HashMap<String, String>, depth: usize) -> Option<Color32> {
    let v = v.trim();
    if depth > 10 {
        return None;
    }
    if let Some(name) = v.strip_prefix('@') {
        return defs.get(name).and_then(|x| parse_color(x, defs, depth + 1));
    }
    if let Some(h) = v.strip_prefix('#') {
        let h: String = h.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        let n = |s: &str| u8::from_str_radix(s, 16).ok();
        return match h.len() {
            3 | 4 => {
                let d: Vec<u8> = h.chars().map(|c| c.to_digit(16).unwrap() as u8 * 17).collect();
                Some(Color32::from_rgb(d[0], d[1], d[2]))
            }
            6 | 8 => Some(Color32::from_rgb(n(&h[0..2])?, n(&h[2..4])?, n(&h[4..6])?)),
            _ => None,
        };
    }
    let lower = v.to_lowercase();
    if lower.starts_with("rgb") {
        let inner = &v[v.find('(')? + 1..v.rfind(')')?];
        let parts: Vec<f32> = inner
            .split(|c| c == ',' || c == ' ' || c == '/')
            .filter(|s| !s.is_empty())
            .take(3)
            .filter_map(|s| {
                let s = s.trim();
                if let Some(p) = s.strip_suffix('%') { p.parse::<f32>().ok().map(|x| x * 2.55) } else { s.parse().ok() }
            })
            .collect();
        if parts.len() == 3 {
            return Some(Color32::from_rgb(parts[0] as u8, parts[1] as u8, parts[2] as u8));
        }
        return None;
    }
    // alpha(@x, 0.5), shade(@x, 1.1), mix(@a, @b, t): use the first colour argument.
    if let (Some(open), Some(close)) = (v.find('('), v.rfind(')')) {
        let inner = &v[open + 1..close];
        let first = inner.split(',').next().unwrap_or("");
        return parse_color(first, defs, depth + 1);
    }
    match lower.as_str() {
        "white" => Some(Color32::WHITE),
        "black" => Some(Color32::BLACK),
        _ => None,
    }
}

fn build(defs: &HashMap<String, String>, source: &Path) -> Option<Palette> {
    let get = |names: &[&str]| names.iter().find_map(|n| defs.get(*n).and_then(|v| parse_color(v, defs, 0)));
    let p = Palette {
        accent: get(&["accent_bg_color", "accent_color", "theme_selected_bg_color", "primary"]),
        accent_fg: get(&["accent_fg_color", "theme_selected_fg_color", "on_primary"]),
        window_bg: get(&["window_bg_color", "theme_bg_color", "surface_dim", "background"]),
        window_fg: get(&["window_fg_color", "theme_fg_color", "on_surface", "foreground"]),
        view_bg: get(&["view_bg_color", "theme_base_color", "surface"]),
        view_fg: get(&["view_fg_color", "theme_text_color", "on_surface"]),
        headerbar_bg: get(&["headerbar_bg_color"]),
        sidebar_bg: get(&["sidebar_bg_color"]),
        popover_bg: get(&["popover_bg_color", "card_bg_color", "dialog_bg_color"]),
        destructive: get(&["destructive_bg_color", "error_color", "error"]),
        source: source.to_path_buf(),
    };
    if p.accent.is_some() || p.window_bg.is_some() || p.view_bg.is_some() { Some(p) } else { None }
}

// ---------------------------------------------------------------- matugen

fn expand_home(p: &str) -> PathBuf {
    if let Some(r) = p.strip_prefix("~/") { places::home().join(r) } else { PathBuf::from(p) }
}

/// (template, output) pairs from matugen's config.toml.
fn matugen_pairs() -> Vec<(PathBuf, PathBuf)> {
    let text = std::fs::read_to_string(matugen_config()).unwrap_or_default();
    let mut v = Vec::new();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let flush = |i: &mut Option<PathBuf>, o: &mut Option<PathBuf>, v: &mut Vec<(PathBuf, PathBuf)>| {
        if let (Some(a), Some(b)) = (i.take(), o.take()) {
            v.push((a, b));
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut input, &mut output, &mut v);
            continue;
        }
        let Some((k, val)) = line.split_once('=') else { continue };
        let val = val.trim().trim_matches(|c| c == '\'' || c == '"');
        match k.trim() {
            "input_path" => input = Some(expand_home(val)),
            "output_path" => output = Some(expand_home(val)),
            _ => {}
        }
    }
    flush(&mut input, &mut output, &mut v);
    v
}

pub fn matugen_config() -> PathBuf {
    places::config_home().join("matugen/config.toml")
}

enum Tok {
    Lit(String),
    Color { name: String, variant: String, fmt: String },
    Other,
}

fn tokenize(t: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut rest = t;
    loop {
        let a = rest.find("{{");
        let b = rest.find("{%");
        let start = match (a, b) {
            (Some(x), Some(y)) => x.min(y),
            (Some(x), None) | (None, Some(x)) => x,
            (None, None) => {
                out.push(Tok::Lit(rest.to_string()));
                return out;
            }
        };
        out.push(Tok::Lit(rest[..start].to_string()));
        let close = if rest[start..].starts_with("{{") { "}}" } else { "%}" };
        let Some(end) = rest[start + 2..].find(close) else {
            return out;
        };
        let inner = rest[start + 2..start + 2 + end].trim();
        rest = &rest[start + 2 + end + 2..];
        let parts: Vec<&str> = inner.split('.').map(|x| x.trim()).collect();
        if close == "}}" && !inner.contains('|') && parts.len() == 4 && parts[0] == "colors" {
            out.push(Tok::Color { name: parts[1].into(), variant: parts[2].into(), fmt: parts[3].into() });
        } else {
            out.push(Tok::Other);
        }
    }
}

/// Learn colour values by lining a matugen template up with the file it produced.
fn learn(template: &str, output: &str, found: &mut HashMap<(String, String), Color32>) {
    let toks = tokenize(template);
    let mut cur = 0usize;
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Lit(l) => {
                if l.is_empty() {
                    i += 1;
                    continue;
                }
                if output[cur..].starts_with(l.as_str()) {
                    cur += l.len();
                } else {
                    return; // template and output no longer line up
                }
            }
            Tok::Color { name, variant, fmt } => {
                let rest = &output[cur..];
                let take = match fmt.as_str() {
                    "hex" => rest.strip_prefix('#').map(|r| 1 + r.chars().take_while(|c| c.is_ascii_hexdigit()).count().min(8)),
                    "hex_stripped" => Some(rest.chars().take_while(|c| c.is_ascii_hexdigit()).count().min(6)),
                    "rgb" | "rgba" => rest.find(')').map(|e| e + 1),
                    _ => None,
                };
                let Some(n) = take else { return };
                let raw = &rest[..n];
                let val = if fmt == "hex_stripped" { format!("#{raw}") } else { raw.to_string() };
                if let Some(c) = parse_color(&val, &HashMap::new(), 0) {
                    found.entry((name.clone(), variant.clone())).or_insert(c);
                }
                cur += n;
            }
            Tok::Other => {
                // Unknown length: skip to the next literal text.
                let next = toks[i + 1..].iter().find_map(|t| match t {
                    Tok::Lit(l) if !l.is_empty() => Some(Some(l.as_str())),
                    Tok::Lit(_) => None,
                    _ => Some(None),
                });
                match next {
                    Some(Some(l)) => match output[cur..].find(l) {
                        Some(off) => cur += off,
                        None => return,
                    },
                    _ => return,
                }
            }
        }
        i += 1;
    }
}

fn prefers_dark() -> bool {
    let c = places::config_home();
    for f in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
        if let Ok(t) = std::fs::read_to_string(c.join(f)) {
            for l in t.lines() {
                if let Some(v) = l.trim().strip_prefix("gtk-application-prefer-dark-theme") {
                    let v = v.trim_start_matches([' ', '=']).trim();
                    return v == "1" || v == "true";
                }
            }
        }
    }
    true
}

/// The palette matugen produced most recently, learned from its own output files,
/// with the time of the newest output. Every file involved is added to `watched`.
fn matugen_palette(watched: &mut Vec<PathBuf>) -> Option<(Palette, SystemTime)> {
    let cfg = matugen_config();
    if !watched.contains(&cfg) {
        watched.push(cfg);
    }
    let mut pairs: Vec<(PathBuf, PathBuf, SystemTime)> = matugen_pairs()
        .into_iter()
        .filter_map(|(i, o)| {
            if !watched.contains(&o) {
                watched.push(o.clone());
            }
            let t = std::fs::metadata(&o).and_then(|m| m.modified()).ok()?;
            Some((i, o, t))
        })
        .collect();
    pairs.sort_by(|a, b| b.2.cmp(&a.2));
    let newest = pairs.first()?.2;
    let mut found: HashMap<(String, String), Color32> = HashMap::new();
    for (i, o, _) in &pairs {
        let (Ok(t), Ok(out)) = (std::fs::read_to_string(i), std::fs::read_to_string(o)) else { continue };
        learn(&t, &out, &mut found);
    }
    let mode = if prefers_dark() { "dark" } else { "light" };
    let get = |names: &[&str]| {
        names.iter().find_map(|n| {
            found.get(&(n.to_string(), "default".into())).or_else(|| found.get(&(n.to_string(), mode.into()))).copied()
        })
    };
    let p = Palette {
        accent: get(&["primary"]),
        accent_fg: get(&["on_primary"]),
        window_bg: get(&["surface_container", "background", "surface", "surface_dim"]),
        window_fg: get(&["on_surface", "on_background"]),
        view_bg: get(&["surface", "background", "surface_dim"]),
        view_fg: get(&["on_surface", "on_background"]),
        headerbar_bg: get(&["surface_container", "surface_container_low"]),
        sidebar_bg: get(&["surface_container_low", "surface_container"]),
        popover_bg: get(&["surface_container_high", "surface_container"]),
        destructive: get(&["error"]),
        source: pairs.first().map(|p| p.1.clone()).unwrap_or_default(),
    };
    if p.accent.is_some() && (p.window_bg.is_some() || p.view_bg.is_some()) { Some((p, newest)) } else { None }
}

/// Read the current desktop palette. Returns it plus every file worth watching.
pub fn load() -> (Option<Palette>, Vec<PathBuf>) {
    let mut watched: Vec<PathBuf> = Vec::new();
    // Priority: Rivet's own file, then generated colours in the config folder (first
    // candidate wins), and only then the fixed defaults of an installed GTK theme.
    let mut user_layers: Vec<(PathBuf, HashMap<String, String>)> = Vec::new();
    let mut theme_layers: Vec<(PathBuf, HashMap<String, String>)> = Vec::new();
    for c in candidates() {
        let mut defs = HashMap::new();
        let mut theme_defs = HashMap::new();
        let mut visited = Vec::new();
        collect(&c, &mut defs, &mut theme_defs, &mut visited, 0);
        for v in visited {
            if !watched.contains(&v) {
                watched.push(v);
            }
        }
        user_layers.push((c.clone(), defs));
        theme_layers.push((c, theme_defs));
    }
    let mut merged: HashMap<String, String> = HashMap::new();
    for (_, d) in theme_layers.iter().rev() {
        merged.extend(d.clone());
    }
    let mut source = theme_layers.iter().find(|(_, d)| !d.is_empty()).map(|(p, _)| p.clone());
    for (p, d) in user_layers.iter().rev() {
        if !d.is_empty() {
            merged.extend(d.clone());
            source = Some(p.clone());
        }
    }
    let gtk = source.as_ref().and_then(|src| build(&merged, src));
    let rivet_file = user_layers.first().is_some_and(|(_, d)| !d.is_empty());
    // When were the generated GTK colours last written (theme defaults don't count)?
    let gtk_time = watched
        .iter()
        .filter(|p| !is_theme_default(p) && p.extension().is_some_and(|e| e == "css"))
        .filter_map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .max();
    let gtk_is_generated = user_layers.iter().any(|(_, d)| !d.is_empty());
    let mat = matugen_palette(&mut watched);
    let found = match (gtk, mat) {
        (Some(g), _) if rivet_file => Some(g),
        (Some(g), Some((m, mt))) => {
            if gtk_is_generated && gtk_time.is_some_and(|gt| gt > mt) { Some(g) } else { Some(m) }
        }
        (None, Some((m, _))) => Some(m),
        (g, None) => g,
    };
    for c in candidates().into_iter().chain(settings_files()) {
        if !watched.contains(&c) {
            watched.push(c);
        }
    }
    (found, watched)
}

/// Polls the colour and settings files; `changed()` is true once per change.
pub struct Watcher {
    files: Vec<PathBuf>,
    sig: Vec<Option<(SystemTime, u64, Option<PathBuf>)>>,
    last: Instant,
}

fn stamp(p: &Path) -> Option<(SystemTime, u64, Option<PathBuf>)> {
    // Follow symlinks for content, but also notice a symlink being re-pointed.
    let link = std::fs::read_link(p).ok();
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len(), link))
}

impl Watcher {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let sig = files.iter().map(|f| stamp(f)).collect();
        Watcher { files, sig, last: Instant::now() }
    }

    pub fn changed(&mut self) -> bool {
        if self.last.elapsed().as_millis() < 900 {
            return false;
        }
        self.last = Instant::now();
        let sig: Vec<_> = self.files.iter().map(|f| stamp(f)).collect();
        if sig != self.sig {
            self.sig = sig;
            return true;
        }
        false
    }

    pub fn set_files(&mut self, files: Vec<PathBuf>) {
        if files != self.files {
            self.sig = files.iter().map(|f| stamp(f)).collect();
            self.files = files;
        }
    }
}

/// A matugen template for Rivet's own colour file.
pub const MATUGEN_TEMPLATE: &str = "/* Rivet Files colours — generated by matugen */
@define-color accent_bg_color {{colors.primary.default.hex}};
@define-color accent_fg_color {{colors.on_primary.default.hex}};
@define-color window_bg_color {{colors.surface_container.default.hex}};
@define-color window_fg_color {{colors.on_surface.default.hex}};
@define-color headerbar_bg_color {{colors.surface_container.default.hex}};
@define-color sidebar_bg_color {{colors.surface_container_low.default.hex}};
@define-color view_bg_color {{colors.surface.default.hex}};
@define-color view_fg_color {{colors.on_surface.default.hex}};
@define-color popover_bg_color {{colors.surface_container_high.default.hex}};
@define-color destructive_bg_color {{colors.error.default.hex}};
";

/// Write the template next to matugen's other templates and register it in
/// matugen's config (only when the person presses the button in Preferences).
pub fn install_matugen_template() -> Result<String, String> {
    let mdir = places::config_home().join("matugen");
    let tdir = mdir.join("templates");
    std::fs::create_dir_all(&tdir).map_err(|e| e.to_string())?;
    let tfile = tdir.join("rivet-colors.css");
    std::fs::write(&tfile, MATUGEN_TEMPLATE).map_err(|e| e.to_string())?;
    let cfg = mdir.join("config.toml");
    let existing = std::fs::read_to_string(&cfg).unwrap_or_default();
    if existing.contains("[templates.rivet]") {
        return Ok("The Rivet template was refreshed; matugen already knows about it.".into());
    }
    let block = "\n[templates.rivet]\ninput_path = '~/.config/matugen/templates/rivet-colors.css'\noutput_path = '~/.config/rivet/colors.css'\n";
    let mut text = existing;
    text.push_str(block);
    std::fs::write(&cfg, text).map_err(|e| e.to_string())?;
    Ok("Added a Rivet template to matugen. The next time matugen runs, Rivet will recolour itself.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learns_from_matugen_output() {
        let t = "[colors]\nbackground={{colors.background.default.hex_stripped}}ff\n{{ image }}\ninput={{colors.primary.default.hex}};\nx={{colors.on_primary.dark.hex | set_alpha: 0.5}}\nerr={{colors.error.default.rgb}}\n";
        let o = "[colors]\nbackground=0b0f11ff\n/home/me/wall.png\ninput=#62c1ff;\nx=rgba(1,2,3,0.5)\nerr=rgb(200, 10, 20)\n";
        let mut f = HashMap::new();
        learn(t, o, &mut f);
        assert_eq!(f.get(&("background".into(), "default".into())), Some(&Color32::from_rgb(0x0b, 0x0f, 0x11)));
        assert_eq!(f.get(&("primary".into(), "default".into())), Some(&Color32::from_rgb(0x62, 0xc1, 0xff)));
        assert_eq!(f.get(&("error".into(), "default".into())), Some(&Color32::from_rgb(200, 10, 20)));
        assert!(!f.contains_key(&("on_primary".into(), "dark".into())));
    }
    #[test]
    fn parses_gtk_colors() {
        let dir = std::env::temp_dir().join(format!("rivet-pal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("gtk.css"), "/* c */ @import url(\"colors.css\");\n@define-color window_bg_color #101010;").unwrap();
        std::fs::write(dir.join("colors.css"), "@define-color accent_color #ff8800;\n@define-color accent_bg_color @accent_color;\n@define-color window_bg_color #ffffff;\n@define-color view_bg_color rgb(1, 2, 3);\n@define-color popover_bg_color alpha(@accent_color, 0.5);").unwrap();
        let mut defs = HashMap::new();
        let mut theme = HashMap::new();
        let mut visited = vec![];
        collect(&dir.join("gtk.css"), &mut defs, &mut theme, &mut visited, 0);
        let p = build(&defs, &dir).unwrap();
        assert_eq!(p.accent, Some(Color32::from_rgb(0xff, 0x88, 0)));
        assert_eq!(p.window_bg, Some(Color32::from_rgb(0x10, 0x10, 0x10)));
        assert_eq!(p.view_bg, Some(Color32::from_rgb(1, 2, 3)));
        assert_eq!(p.popover_bg, p.accent);
        assert!(p.is_dark());
        assert_eq!(visited.len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod live_probe {
    #[test]
    #[ignore]
    fn print_desktop_palette() {
        let (p, w) = super::load();
        println!("PALETTE {:?}\nWATCHED {:?}", p, w);
    }
}
