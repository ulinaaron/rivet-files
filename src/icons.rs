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

/// Icon name → where it exists (dir index, root index, extension index).
type IconIndex = HashMap<String, Vec<(u16, u8, u8)>>;

#[derive(Clone, Debug)]
struct Theme {
    roots: Vec<PathBuf>,
    dirs: Vec<ThemeDir>,
    inherits: Vec<String>,
    /// icon name -> where it exists (dir index, root index, extension index).
    /// Built on first use by listing each theme folder once, so lookups never
    /// probe the disk file by file.
    index: Option<IconIndex>,
}

pub struct IconTheme {
    pub name: String,
    chain: Vec<Theme>,
    found: HashMap<(String, u32), Option<PathBuf>>,
    textures: HashMap<(String, u32), Option<egui::TextureHandle>>,
    /// When set, Rivet's own palette-coloured SVG icons answer first.
    builtin: Option<crate::builtin_icons::BuiltinColors>,
    builtin_tex: HashMap<(&'static str, u32), egui::TextureHandle>,
    /// Single-colour versions of icons, for drawing on the highlight colour.
    masks: HashMap<(String, u32), Option<egui::TextureHandle>>,
    /// Terminal mode: Nerd Font glyphs answer before anything else.
    nerd: Option<(std::sync::Arc<crate::nerd::NerdFont>, crate::nerd::NerdColors)>,
    nerd_tex: HashMap<(String, u32), Option<egui::TextureHandle>>,
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
    // The sandbox's /usr is the runtime, not the desktop's icon theme.
    for d in crate::places::host_data_dirs() {
        let p = d.join("icons");
        if p.is_dir() && !v.contains(&p) {
            v.push(p);
        }
    }
    v.push(PathBuf::from("/usr/share/pixmaps"));
    let host_pixmaps = PathBuf::from("/run/host/usr/share/pixmaps");
    if crate::places::in_flatpak() && host_pixmaps.is_dir() {
        v.push(host_pixmaps);
    }
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
        if in_section
            && let Some((k, v)) = line.split_once('=')
                && k.trim() == key {
                    return Some(v.trim().trim_matches('"').to_string());
                }
    }
    None
}

/// Work out which icon theme the desktop is using, without linking any toolkit.
pub fn detect_system_theme() -> String {
    if let Ok(t) = std::env::var("RIVET_ICON_THEME")
        && !t.is_empty() {
            return t;
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

    if desktop.contains("kde")
        && let Some(t) = read_ini_value(&config.join("kdeglobals"), "Icons", "Theme") {
            return t;
        }
    if (desktop.contains("cinnamon") || desktop.contains("x-cinnamon"))
        && let Some(t) = try_gsettings("org.cinnamon.desktop.interface") {
            return t;
        }
    for f in ["gtk-3.0/settings.ini", "gtk-4.0/settings.ini"] {
        if let Some(t) = read_ini_value(&config.join(f), "Settings", "gtk-icon-theme-name")
            && !t.is_empty() {
                return t;
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
                    if let Some(n) = p.file_name().and_then(|n| n.to_str())
                        && !names.iter().any(|x: &String| x == n) {
                            names.push(n.to_string());
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
    Some(Theme { roots, dirs, inherits, index: None })
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
        Self { name: name.to_string(), chain, found: HashMap::new(), textures: HashMap::new(), builtin: None, builtin_tex: HashMap::new(), masks: HashMap::new(), nerd: None, nerd_tex: HashMap::new() }
    }

    fn build_index(theme: &mut Theme) {
        let mut index: HashMap<String, Vec<(u16, u8, u8)>> = HashMap::new();
        for (di, d) in theme.dirs.iter().enumerate() {
            for (ri, r) in theme.roots.iter().enumerate() {
                let Ok(rd) = std::fs::read_dir(r.join(&d.path)) else { continue };
                for e in rd.flatten() {
                    let file = e.file_name();
                    let file = file.to_string_lossy();
                    let Some((stem, ext)) = file.rsplit_once('.') else { continue };
                    let Some(ei) = EXTS.iter().position(|x| *x == ext) else { continue };
                    index.entry(stem.to_string()).or_default().push((di as u16, ri as u8, ei as u8));
                }
            }
        }
        theme.index = Some(index);
    }

    fn lookup_in(theme: &mut Theme, name: &str, size: u32) -> Option<PathBuf> {
        if theme.index.is_none() {
            Self::build_index(theme);
        }
        let hits = theme.index.as_ref()?.get(name)?;
        let path = |&(di, ri, ei): &(u16, u8, u8)| {
            theme.roots[ri as usize].join(&theme.dirs[di as usize].path).join(format!("{name}.{}", EXTS[ei as usize]))
        };
        // An exact size match first (in the theme's own directory order), else the closest size.
        if let Some(h) = hits.iter().filter(|h| dir_matches(&theme.dirs[h.0 as usize], size)).min_by_key(|h| (h.0, h.1, h.2)) {
            return Some(path(h));
        }
        hits.iter().min_by_key(|h| (dir_distance(&theme.dirs[h.0 as usize], size), h.0, h.1, h.2)).map(path)
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
            // First lookup ever: index every theme in the chain at once, in parallel.
            if self.chain.iter().any(|t| t.index.is_none()) {
                std::thread::scope(|s| {
                    for t in self.chain.iter_mut().filter(|t| t.index.is_none()) {
                        s.spawn(move || Self::build_index(t));
                    }
                });
            }
            let mut hit = None;
            for t in &mut self.chain {
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

    /// Switch the built-in icon set on (with these colours) or off.
    pub fn set_builtin(&mut self, colors: Option<crate::builtin_icons::BuiltinColors>) {
        if self.builtin != colors {
            self.builtin = colors;
            self.builtin_tex.clear(); // recolour on next draw
            self.masks.clear();
        }
    }

    /// Switch terminal mode's Nerd Font glyphs on (with these colours) or off.
    pub fn set_nerd(&mut self, nerd: Option<(std::sync::Arc<crate::nerd::NerdFont>, crate::nerd::NerdColors)>) {
        let same = match (&self.nerd, &nerd) {
            (None, None) => true,
            (Some((a, ca)), Some((b, cb))) => a.path == b.path && ca == cb,
            _ => false,
        };
        if !same {
            self.nerd = nerd;
            self.nerd_tex.clear();
        }
    }

    /// The Nerd Font glyph for the first name that has one: in its kind's colour, or white
    /// (to be tinted) for symbolic icons and masks.
    fn nerd_texture(&mut self, ctx: &egui::Context, names: &[&str], px: u32, mask: bool) -> Option<Option<egui::TextureHandle>> {
        let (font, colors) = self.nerd.as_ref()?;
        for n in names {
            let Some((key, symbolic)) = crate::builtin_icons::lookup(n) else { continue };
            let Some(ch) = crate::nerd::glyph_for(key) else { continue };
            let k = (format!("{}{key}", if mask { "m:" } else { "" }), px);
            if let Some(t) = self.nerd_tex.get(&k) {
                return Some(t.clone());
            }
            let fill = if symbolic || mask { egui::Color32::WHITE } else { colors.for_key(key) };
            let t = font
                .svg(ch, fill)
                .and_then(|svg| render_svg(svg.as_bytes(), px))
                .map(|img| ctx.load_texture(format!("nerd:{}:{px}", k.0), img, egui::TextureOptions::LINEAR));
            self.nerd_tex.insert(k, t.clone());
            return Some(t);
        }
        None
    }

    /// Get a texture for the first available icon name, rendered at `px` pixels.
    pub fn texture(&mut self, ctx: &egui::Context, names: &[&str], px: u32) -> Option<egui::TextureHandle> {
        if let Some(t) = self.nerd_texture(ctx, names, px, false) {
            return t;
        }
        if let Some(colors) = self.builtin {
            for n in names {
                if let Some((key, _)) = crate::builtin_icons::lookup(n) {
                    if let Some(t) = self.builtin_tex.get(&(key, px)) {
                        return Some(t.clone());
                    }
                    let svg = crate::builtin_icons::svg(key, &colors);
                    if let Some(img) = render_svg(svg.as_bytes(), px) {
                        let t = ctx.load_texture(format!("builtin:{key}:{px}"), img, egui::TextureOptions::LINEAR);
                        self.builtin_tex.insert((key, px), t.clone());
                        return Some(t);
                    }
                }
            }
        }
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

    /// A single-colour (white, tint it) version of the first available icon: its shape,
    /// with a little shading kept so inner details such as document lines stay visible.
    pub fn mask(&mut self, ctx: &egui::Context, names: &[&str], px: u32) -> Option<egui::TextureHandle> {
        if let Some(t) = self.nerd_texture(ctx, names, px, true) {
            return t;
        }
        if let Some(colors) = self.builtin {
            for n in names {
                if let Some((key, _)) = crate::builtin_icons::lookup(n) {
                    let k = (format!("b:{key}"), px);
                    if let Some(t) = self.masks.get(&k) {
                        return t.clone();
                    }
                    let img = render_svg(crate::builtin_icons::svg(key, &colors).as_bytes(), px);
                    let t = img.map(|i| ctx.load_texture(format!("mask:{}:{px}", k.0), to_mask(i), egui::TextureOptions::LINEAR));
                    self.masks.insert(k, t.clone());
                    return t;
                }
            }
        }
        let (name, path) = self.find(names, nominal_size(px))?;
        self.mask_path(ctx, &name, &path, px)
    }

    pub fn mask_from_path(&mut self, ctx: &egui::Context, path: &Path, px: u32) -> Option<egui::TextureHandle> {
        let key = path.to_string_lossy().to_string();
        self.mask_path(ctx, &key, path, px)
    }

    fn mask_path(&mut self, ctx: &egui::Context, key: &str, path: &Path, px: u32) -> Option<egui::TextureHandle> {
        let k = (key.to_string(), px);
        if let Some(t) = self.masks.get(&k) {
            return t.clone();
        }
        let t = load_image_file(path, px).map(|i| ctx.load_texture(format!("mask:{key}:{px}"), to_mask(i), egui::TextureOptions::LINEAR));
        self.masks.insert(k, t.clone());
        t
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

/// Turn a full-colour icon into a white silhouette (premultiplied). Brightness is kept
/// as a gentle opacity variation (relative to the icon's brightest pixel), so the shape
/// reads as one solid colour while darker details still show through.
fn to_mask(mut img: egui::ColorImage) -> egui::ColorImage {
    let lum = |p: &egui::Color32| {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        if a == 0 { 0.0 } else { (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0 }
    };
    let max = img.pixels.iter().map(lum).fold(0.0f32, f32::max).max(0.05);
    for p in img.pixels.iter_mut() {
        let a = p.a() as f32 / 255.0;
        let l = (lum(p) / max).clamp(0.0, 1.0);
        let out = (a * (0.45 + 0.55 * l) * 255.0).round() as u8;
        *p = egui::Color32::from_rgba_premultiplied(out, out, out, out);
    }
    img
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

pub(crate) fn render_svg(data: &[u8], px: u32) -> Option<egui::ColorImage> {
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
