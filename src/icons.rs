//! Freedesktop icon theme lookup (no GTK): reads the user's icon theme name from
//! the usual settings files, follows `Inherits=` chains down to `hicolor`, and
//! renders SVG (resvg) or PNG (image) icons into egui textures.

use eframe::egui;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct ThemeDir {
    path: String,
    size: u32,
    scale: u32,
    min: u32,
    max: u32,
    threshold: u32,
    kind: DirKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DirKind {
    Fixed,
    Scalable,
    Threshold,
}

#[derive(Clone, Debug)]
struct Theme {
    roots: Vec<PathBuf>,
    dirs: Vec<ThemeDir>,
    inherits: Vec<String>,
}

pub struct IconTheme {
    pub name: String,
    chain: Vec<Theme>,
    found: HashMap<(String, u32), Option<PathBuf>>,
    textures: HashMap<(String, u32), Option<egui::TextureHandle>>,
}

pub fn icon_base_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        v.push(home.join(".icons"));
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        v.push(data_home.join("icons"));
    }
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    for d in data_dirs.split(':') {
        if !d.is_empty() {
            let p = PathBuf::from(d).join("icons");
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v.push(PathBuf::from("/usr/share/pixmaps"));
    v
}

fn read_ini_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.trim_matches(|c| c == '[' || c == ']') == section;
            continue;
        }
        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

/// Work out which icon theme the desktop is using, without linking any toolkit.
pub fn detect_system_theme() -> String {
    if let Ok(t) = std::env::var("RIVET_ICON_THEME") {
        if !t.is_empty() {
            return t;
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();

    // Ask the desktop's settings store through its command-line tool (read only).
    let try_gsettings = |schema: &str| -> Option<String> {
        let out = std::process::Command::new("gsettings")
            .args(["get", schema, "icon-theme"])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().trim_matches('\'').to_string();
        if s.is_empty() { None } else { Some(s) }
    };

    if desktop.contains("kde") {
        if let Some(t) = read_ini_value(&config.join("kdeglobals"), "Icons", "Theme") {
            return t;
        }
    }
    if desktop.contains("cinnamon") || desktop.contains("x-cinnamon") {
        if let Some(t) = try_gsettings("org.cinnamon.desktop.interface") {
            return t;
        }
    }
    for f in ["gtk-3.0/settings.ini", "gtk-4.0/settings.ini"] {
        if let Some(t) = read_ini_value(&config.join(f), "Settings", "gtk-icon-theme-name") {
            if !t.is_empty() {
                return t;
            }
        }
    }
    if let Some(t) = read_ini_value(&config.join("kdeglobals"), "Icons", "Theme") {
        return t;
    }
    if let Some(t) = try_gsettings("org.gnome.desktop.interface") {
        return t;
    }
    "hicolor".to_string()
}

/// All installed icon themes that have folder/mime icons (for the preferences list).
pub fn installed_themes() -> Vec<String> {
    let mut names = Vec::new();
    for base in icon_base_dirs() {
        if let Ok(rd) = std::fs::read_dir(&base) {
            for e in rd.flatten() {
                let p = e.path();
                let idx = p.join("index.theme");
                if idx.is_file() {
                    let dirs = read_ini_value(&idx, "Icon Theme", "Directories").unwrap_or_default();
                    if dirs.is_empty() {
                        continue; // cursor-only theme
                    }
                    if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                        if !names.iter().any(|x: &String| x == n) {
                            names.push(n.to_string());
                        }
                    }
                }
            }
        }
    }
    names.sort_by_key(|a| a.to_lowercase());
    names
}

fn load_theme(name: &str) -> Option<Theme> {
    let roots: Vec<PathBuf> = icon_base_dirs()
        .into_iter()
        .map(|b| b.join(name))
        .filter(|p| p.is_dir())
        .collect();
    let index = roots.iter().map(|r| r.join("index.theme")).find(|p| p.is_file())?;
    let text = std::fs::read_to_string(&index).ok()?;

    // Parse all sections once.
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut cur = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            cur = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            sections
                .entry(cur.clone())
                .or_default()
                .insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let head = sections.get("Icon Theme").cloned().unwrap_or_default();
    let mut dir_names: Vec<String> = Vec::new();
    for key in ["Directories", "ScaledDirectories"] {
        if let Some(d) = head.get(key) {
            for n in d.split(',') {
                let n = n.trim();
                if !n.is_empty() && !dir_names.iter().any(|x| x == n) {
                    dir_names.push(n.to_string());
                }
            }
        }
    }
    let mut dirs = Vec::new();
    for dn in dir_names {
        let Some(sec) = sections.get(&dn) else { continue };
        let num = |k: &str| sec.get(k).and_then(|v| v.parse::<u32>().ok());
        let size = num("Size").unwrap_or(48);
        let kind = match sec.get("Type").map(|s| s.as_str()) {
            Some("Fixed") => DirKind::Fixed,
            Some("Scalable") => DirKind::Scalable,
            _ => DirKind::Threshold,
        };
        dirs.push(ThemeDir {
            path: dn,
            size,
            scale: num("Scale").unwrap_or(1),
            min: num("MinSize").unwrap_or(size),
            max: num("MaxSize").unwrap_or(size),
            threshold: num("Threshold").unwrap_or(2),
            kind,
        });
    }
    let inherits = head
        .get("Inherits")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    Some(Theme { roots, dirs, inherits })
}

fn dir_matches(d: &ThemeDir, size: u32) -> bool {
    if d.scale != 1 {
        return false;
    }
    match d.kind {
        DirKind::Fixed => d.size == size,
        DirKind::Scalable => d.min <= size && size <= d.max,
        DirKind::Threshold => d.size.saturating_sub(d.threshold) <= size && size <= d.size + d.threshold,
    }
}

fn dir_distance(d: &ThemeDir, size: u32) -> u32 {
    let s = size as i64;
    let v = match d.kind {
        DirKind::Fixed => (d.size as i64 - s).abs(),
        DirKind::Scalable => {
            if s < d.min as i64 {
                d.min as i64 - s
            } else if s > d.max as i64 {
                s - d.max as i64
            } else {
                0
            }
        }
        DirKind::Threshold => {
            let lo = d.size as i64 - d.threshold as i64;
            let hi = d.size as i64 + d.threshold as i64;
            if s < lo {
                lo - s
            } else if s > hi {
                s - hi
            } else {
                0
            }
        }
    };
    // Prefer bigger icons (downscaling looks better than upscaling) and scale 1.
    let penalty = if (d.size as i64) < s && d.kind != DirKind::Scalable { 1000 } else { 0 };
    (v + penalty + if d.scale != 1 { 5000 } else { 0 }) as u32
}

const EXTS: [&str; 3] = ["svg", "png", "xpm"];

impl IconTheme {
    pub fn new(name: &str) -> Self {
        let mut chain = Vec::new();
        let mut queue = vec![name.to_string()];
        let mut seen: Vec<String> = Vec::new();
        while let Some(n) = queue.first().cloned() {
            queue.remove(0);
            if seen.contains(&n) {
                continue;
            }
            seen.push(n.clone());
            if let Some(t) = load_theme(&n) {
                for i in &t.inherits {
                    queue.push(i.clone());
                }
                chain.push(t);
            }
        }
        // Always end with a few broadly-complete fallbacks.
        for fb in ["Adwaita", "AdwaitaLegacy", "breeze", "hicolor"] {
            if !seen.iter().any(|s| s == fb) {
                seen.push(fb.to_string());
                if let Some(t) = load_theme(fb) {
                    chain.push(t);
                }
            }
        }
        Self { name: name.to_string(), chain, found: HashMap::new(), textures: HashMap::new() }
    }

    fn lookup_in(theme: &Theme, name: &str, size: u32) -> Option<PathBuf> {
        // exact match pass
        for d in theme.dirs.iter().filter(|d| dir_matches(d, size)) {
            for r in &theme.roots {
                for ext in EXTS {
                    let p = r.join(&d.path).join(format!("{name}.{ext}"));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
        // closest size pass
        let mut best: Option<(u32, PathBuf)> = None;
        for d in &theme.dirs {
            let dist = dir_distance(d, size);
            if best.as_ref().is_some_and(|(b, _)| *b <= dist) {
                continue;
            }
            for r in &theme.roots {
                for ext in EXTS {
                    let p = r.join(&d.path).join(format!("{name}.{ext}"));
                    if p.is_file() {
                        best = Some((dist, p));
                        break;
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Find the file for the first of `names` that exists in the theme chain.
    pub fn find(&mut self, names: &[&str], size: u32) -> Option<(String, PathBuf)> {
        for n in names {
            let key = (n.to_string(), size);
            if let Some(hit) = self.found.get(&key) {
                if let Some(p) = hit {
                    return Some((n.to_string(), p.clone()));
                }
                continue;
            }
            let mut hit = None;
            for t in &self.chain {
                if let Some(p) = Self::lookup_in(t, n, size) {
                    hit = Some(p);
                    break;
                }
            }
            if hit.is_none() {
                let px = PathBuf::from("/usr/share/pixmaps");
                for ext in EXTS {
                    let p = px.join(format!("{n}.{ext}"));
                    if p.is_file() {
                        hit = Some(p);
                        break;
                    }
                }
            }
            self.found.insert(key, hit.clone());
            if let Some(p) = hit {
                return Some((n.to_string(), p));
            }
        }
        None
    }

    /// Get a texture for the first available icon name, rendered at `px` pixels.
    pub fn texture(&mut self, ctx: &egui::Context, names: &[&str], px: u32) -> Option<egui::TextureHandle> {
        let lookup_size = nominal_size(px);
        let (name, path) = self.find(names, lookup_size)?;
        let key = (name.clone(), px);
        if let Some(t) = self.textures.get(&key) {
            return t.clone();
        }
        let symbolic = name.ends_with("-symbolic") || path.to_string_lossy().contains("/symbolic/");
        let tex = load_image_file(&path, px).map(|mut img| {
            if symbolic {
                // Symbolic icons are single-colour masks: make them white so tinting gives the exact text colour.
                for p in img.pixels.iter_mut() {
                    let a = p.a();
                    *p = egui::Color32::from_rgba_premultiplied(a, a, a, a);
                }
            }
            ctx.load_texture(format!("icon:{name}:{px}"), img, egui::TextureOptions::LINEAR)
        });
        self.textures.insert(key, tex.clone());
        tex
    }

    /// Load an icon from an absolute path (e.g. an `Icon=` value in a .desktop file).
    pub fn texture_from_path(&mut self, ctx: &egui::Context, path: &Path, px: u32) -> Option<egui::TextureHandle> {
        let key = (path.to_string_lossy().to_string(), px);
        if let Some(t) = self.textures.get(&key) {
            return t.clone();
        }
        let tex = load_image_file(path, px)
            .map(|img| ctx.load_texture(format!("iconp:{}:{px}", key.0), img, egui::TextureOptions::LINEAR));
        self.textures.insert(key, tex.clone());
        tex
    }
}

/// Snap a pixel size to the usual theme sizes so lookups are shared.
fn nominal_size(px: u32) -> u32 {
    for s in [16, 22, 24, 32, 48, 64, 96, 128, 256] {
        if px <= s {
            return s;
        }
    }
    256
}

pub fn load_image_file(path: &Path, px: u32) -> Option<egui::ColorImage> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let data = std::fs::read(path).ok()?;
    if ext == "svg" || ext == "svgz" {
        render_svg(&data, px)
    } else {
        let img = image::load_from_memory(&data).ok()?;
        let img = if img.width() != px || img.height() != px {
            img.resize(px, px, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let rgba = img.to_rgba8();
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        ))
    }
}

fn render_svg(data: &[u8], px: u32) -> Option<egui::ColorImage> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(data, &opt).ok()?;
    let size = tree.size();
    let px = px.max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = (px as f32 / size.width()).min(px as f32 / size.height());
    let dx = (px as f32 - size.width() * scale) / 2.0;
    let dy = (px as f32 - size.height() * scale) / 2.0;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], pixmap.data()))
}
