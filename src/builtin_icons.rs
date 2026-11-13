//! Rivet's own icon set: small SVG templates recoloured from the desktop palette
//! (matugen / GTK colours / the chosen accent). All vector, so recolouring is just
//! string substitution + one rasterisation per size, cached by the icon theme.
//!
//! Two kinds:
//! * full-colour icons (folders, files, drives, places) — 48×48, filled from the palette;
//! * symbolic UI icons (toolbar, window buttons) — 24×24 white strokes, tinted at draw time.

use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuiltinColors {
    pub accent: Color32,
    pub bg: Color32,
    pub fg: Color32,
    pub dark: bool,
}

// ---------------------------------------------------------------- colour maths

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    crate::palette::mix(a, b, t)
}

fn to_hsv(c: Color32) -> (f32, f32, f32) {
    let (r, g, b) = (c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

fn from_hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(r), f(g), f(b))
}

/// A sibling colour: the accent's hue turned by `deg`, kept vivid enough to read.
fn hue(accent: Color32, deg: f32) -> Color32 {
    let (h, s, v) = to_hsv(accent);
    from_hsv(h + deg, s.clamp(0.42, 0.8), v.clamp(0.62, 0.86))
}

/// Material-style secondary: the accent's hue, much calmer.
pub fn derive_secondary(accent: Color32) -> Color32 {
    let (h, s, v) = to_hsv(accent);
    from_hsv(h, (s * 0.45).clamp(0.12, 0.4), v.clamp(0.55, 0.85))
}

/// Material-style tertiary: a neighbouring hue with similar strength.
pub fn derive_tertiary(accent: Color32) -> Color32 {
    let (h, s, v) = to_hsv(accent);
    from_hsv(h + 60.0, (s * 0.8).clamp(0.3, 0.7), v.clamp(0.6, 0.85))
}

fn darken(c: Color32, t: f32) -> Color32 {
    mix(c, Color32::BLACK, t)
}

// ---------------------------------------------------------------- lookup

/// Which built-in icon (if any) answers this freedesktop icon name.
/// Returns (key, symbolic).
pub fn lookup(name: &str) -> Option<(&'static str, bool)> {
    if let Some(base) = name.strip_suffix("-symbolic") {
        let k = match base {
            "go-previous" => "s:back",
            "go-next" => "s:forward",
            "go-up" => "s:up",
            "view-refresh" => "s:reload",
            "user-home" | "go-home" => "s:home",
            "computer" => "s:computer",
            "drive-harddisk" => "s:drive",
            "document-edit" | "edit" | "text-editor" => "s:edit",
            "edit-find" | "system-search" => "s:search",
            "view-grid" | "view-app-grid" => "s:grid",
            "view-list" => "s:list",
            "view-compact" | "view-more-horizontal" => "s:compact",
            "folder-new" => "s:folder-new",
            "tab-new" | "list-add" => "s:add",
            "utilities-terminal" | "terminal" => "s:terminal",
            "view-reveal" | "view-visible" | "eye-open" => "s:eye",
            "user-trash" => "s:trash",
            "document-properties" | "dialog-information" => "s:info",
            "bookmark-new" | "starred" | "user-bookmarks" => "s:star",
            "sidebar-show" | "view-sidebar" | "view-dual" | "view-left-pane" => "s:sidebar",
            "view-list-tree" => "s:tree",
            "media-eject" => "s:eject",
            "window-minimize" => "s:min",
            "window-maximize" => "s:max",
            "window-restore" => "s:restore",
            "window-close" => "s:close",
            "open-menu" | "application-menu" | "view-more" => "s:menu",
            "folder" => "s:folder",
            "network-workgroup" | "network-server" => "s:network",
            "document-open-recent" => "s:recent",
            _ => return None,
        };
        return Some((k, true));
    }
    let k = match name {
        "folder" | "inode-directory" | "folder-open" | "system-file-manager" | "org.gnome.Nautilus" => "folder",
        "user-home" => "folder-home",
        "user-desktop" => "folder-desktop",
        "folder-documents" => "folder-documents",
        "folder-download" => "folder-download",
        "folder-music" => "folder-music",
        "folder-pictures" => "folder-pictures",
        "folder-videos" => "folder-videos",
        "folder-templates" => "folder-templates",
        "folder-publicshare" => "folder-public",
        "folder-remote" | "network-workgroup" | "network-server" => "network",
        "document-open-recent" | "folder-recent" => "recent",
        "user-trash" => "trash",
        "user-trash-full" => "trash-full",
        "drive-removable-media" | "media-removable" => "usb",
        "emblem-symbolic-link" | "emblem-shared" => "emblem-link",
        "emblem-unreadable" | "unknown" | "application-x-generic" | "application-octet-stream" => "file-unknown",
        "package-x-generic" => "file-archive",
        "application-x-executable" | "application-x-sharedlib" | "application-x-pie-executable" => "file-exec",
        "application-pdf" => "file-pdf",
        "text-csv" => "file-sheet",
        "text-x-generic" | "text-plain" | "application-x-zerosize" | "text-x-readme" => "file-text",
        "text-html" | "text-css" | "application-json" | "application-xml" | "application-javascript" | "application-x-shellscript" => "file-code",
        n if n.starts_with("drive-harddisk") => "drive",
        n if n.starts_with("image-") => "file-image",
        n if n.starts_with("audio-") => "file-audio",
        n if n.starts_with("video-") => "file-video",
        n if n.starts_with("font-") || n.contains("font") => "file-font",
        n if n.contains("spreadsheet") || n.contains("ms-excel") => "file-sheet",
        n if n.contains("presentation") || n.contains("powerpoint") => "file-slides",
        n if n.contains("compressed") || n.contains("zip") || n.contains("-tar") || n.contains("archive") || n.contains("rar") || n.contains("7z") => "file-archive",
        n if n.contains("document") || n.contains("msword") || n.contains("opendocument.text") || n.contains("rtf") => "file-doc",
        n if n.starts_with("text-x-") || n.contains("script") => "file-code",
        n if n.starts_with("text-") => "file-text",
        _ => return None,
    };
    Some((k, false))
}

// ---------------------------------------------------------------- SVGs

const SVG48: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">"##;
const SVG24: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##;

fn folder(c: &BuiltinColors, emblem: &str) -> String {
    let front = mix(c.accent, c.bg, 0.08);
    let back = darken(c.accent, 0.28);
    let glyph = darken(c.accent, 0.55);
    let emblem = emblem.replace("G", &hex(glyph));
    format!(
        r##"{SVG48}<path d="M5 13a4 4 0 0 1 4-4h10.3a3 3 0 0 1 2.1.9L24 12.5h15a4 4 0 0 1 4 4V20H5z" fill="{b}"/><rect x="5" y="16" width="38" height="26" rx="4" fill="{f}"/><rect x="5" y="16" width="38" height="2.5" rx="1.2" fill="#fff" fill-opacity="0.22"/>{emblem}</svg>"##,
        b = hex(back),
        f = hex(front),
    )
}

const STROKE_G: &str = r##"fill="none" stroke="G" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round""##;

fn folder_emblem(key: &str) -> String {
    let s = STROKE_G;
    match key {
        "folder-home" => format!(r##"<path d="M18 30l6-5.5 6 5.5M20 28.5V35h8v-6.5" {s}/>"##),
        "folder-desktop" => format!(r##"<rect x="17.5" y="23.5" width="13" height="9" rx="1.5" {s}/><path d="M21.5 36h5M24 32.5V36" {s}/>"##),
        "folder-documents" => format!(r##"<path d="M20 23h5.5l3 3v10H20z M22.5 30h3.5M22.5 33h3.5" {s}/>"##),
        "folder-download" => format!(r##"<path d="M24 23v10M19.5 28.5L24 33l4.5-4.5M19 36h10" {s}/>"##),
        "folder-music" => format!(r##"<path d="M22 33.5V24.5l7-1.5v8.5" {s}/><circle cx="20.5" cy="33.5" r="1.8" fill="G"/><circle cx="27.5" cy="31.5" r="1.8" fill="G"/>"##),
        "folder-pictures" => format!(r##"<rect x="17.5" y="23.5" width="13" height="11.5" rx="1.5" {s}/><path d="M18 33.5l4-4 3 3 2-2 3 3" {s}/>"##),
        "folder-videos" => r##"<path d="M21 23.5v11l9-5.5z" fill="G"/>"##.to_string(),
        "folder-templates" => format!(r##"<path d="M18 35l6-11 6 11z" {s}/>"##),
        "folder-public" => format!(r##"<circle cx="20" cy="29.5" r="1.9" fill="G"/><circle cx="28" cy="25.5" r="1.9" fill="G"/><circle cx="28" cy="33.5" r="1.9" fill="G"/><path d="M21.7 28.6l4.6-2.3M21.7 30.4l4.6 2.3" {s}/>"##),
        _ => String::new(),
    }
}

fn file_page(c: &BuiltinColors, inner: &str) -> String {
    // On light backgrounds the page is tinted a little more and gets a thin outline,
    // so it never melts into a white window.
    let page = if c.dark { mix(Color32::from_rgb(0xee, 0xee, 0xee), c.accent, 0.10) } else { mix(Color32::from_rgb(0xf7, 0xf7, 0xf8), c.accent, 0.10) };
    let fold = darken(page, if c.dark { 0.16 } else { 0.22 });
    let line = mix(page, darken(c.accent, 0.5), 0.5);
    let inner = inner.replace("LINE", &hex(line));
    let edge = if c.dark { String::new() } else { format!(r##" stroke="{}" stroke-width="1.2""##, hex(mix(page, darken(c.accent, 0.55), 0.38))) };
    format!(
        r##"{SVG48}<path d="M12 4h16.5L38 13.5V41a3 3 0 0 1-3 3H12a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z" fill="{p}"{edge}/><path d="M28.5 4v7a2.5 2.5 0 0 0 2.5 2.5h7z" fill="{f}"/>{inner}</svg>"##,
        p = hex(page),
        f = hex(fold),
    )
}

fn badge(color: Color32, glyph: &str) -> String {
    format!(
        r##"<rect x="12.5" y="24" width="22" height="15" rx="3.5" fill="{c}"/><g fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{glyph}</g>"##,
        c = hex(color)
    )
}

fn full_colour(key: &str, c: &BuiltinColors) -> String {
    let lines = r##"<path d="M14 21h14M14 26h19M14 31h19M14 36h12" fill="none" stroke="LINE" stroke-width="2" stroke-linecap="round"/>"##;
    match key {
        "folder" => folder(c, ""),
        k if k.starts_with("folder-") => folder(c, &folder_emblem(k)),
        "file-text" => file_page(c, lines),
        "file-image" => file_page(c, &badge(hue(c.accent, 120.0), r##"<path d="M15.5 36.5l4.5-4.5 3.5 3.5 2.5-2.5 4 4"/><circle cx="27.5" cy="28.5" r="1.2" fill="#fff"/>"##)),
        "file-audio" => file_page(c, &badge(hue(c.accent, 60.0), r##"<path d="M22 35V28l6-1.5V33"/><circle cx="20.6" cy="35" r="1.4" fill="#fff"/><circle cx="26.6" cy="33" r="1.4" fill="#fff"/>"##)),
        "file-video" => file_page(c, &badge(hue(c.accent, -60.0), r##"<path d="M21 27.5v8l6.5-4z" fill="#fff"/>"##)),
        "file-archive" => file_page(c, &badge(hue(c.accent, 30.0), r##"<path d="M23.5 26v1.6M23.5 29.4V31M23.5 32.8v1.6"/><rect x="21.5" y="34.5" width="4" height="3" rx="1"/>"##)),
        "file-code" => file_page(c, &badge(c.accent, r##"<path d="M20 28.5l-3 3 3 3M27 28.5l3 3-3 3M24.6 27.5l-2.2 8"/>"##)),
        "file-pdf" => file_page(c, &badge(hue(c.accent, 180.0), r##"<path d="M17 28h13M17 31.5h13M17 35h8"/>"##)),
        "file-doc" => file_page(c, &badge(darken(c.accent, 0.1), r##"<path d="M17 28h13M17 31.5h13M17 35h8"/>"##)),
        "file-sheet" => file_page(c, &badge(hue(c.accent, 90.0), r##"<rect x="16.5" y="27" width="14" height="9" rx="1"/><path d="M16.5 31.5h14M21.5 27v9M26 27v9"/>"##)),
        "file-slides" => file_page(c, &badge(hue(c.accent, -30.0), r##"<path d="M18.5 36v-3M23.5 36v-7M28.5 36v-5"/>"##)),
        "file-font" => file_page(c, &badge(hue(c.accent, -90.0), r##"<path d="M19.5 36.5l4-9 4 9M21 33.3h5"/>"##)),
        "file-exec" => file_page(c, &badge(darken(c.accent, 0.25), r##"<path d="M17.5 28.5l3 2.5-3 2.5M23 35h7"/>"##)),
        "file-unknown" => file_page(c, &badge(mix(c.accent, Color32::GRAY, 0.6), r##"<path d="M21.5 29.5a2.2 2.2 0 1 1 3.2 2c-.8.4-1.2.9-1.2 1.8M23.5 36.2v.1"/>"##)),
        "drive" => {
            let body = mix(c.accent, c.bg, 0.25);
            format!(
                r##"{SVG48}<rect x="6" y="13" width="36" height="24" rx="5" fill="{b}"/><rect x="6" y="27" width="36" height="10" rx="4" fill="{d}"/><circle cx="35" cy="32" r="2" fill="{l}"/><path d="M11 32h14" stroke="{l}" stroke-width="2" stroke-linecap="round"/></svg>"##,
                b = hex(body),
                d = hex(darken(body, 0.25)),
                l = hex(mix(c.accent, Color32::WHITE, 0.55)),
            )
        }
        "usb" => {
            let body = mix(c.accent, c.bg, 0.15);
            format!(
                r##"{SVG48}<rect x="13" y="16" width="22" height="28" rx="4" fill="{b}"/><rect x="17" y="6" width="14" height="11" rx="1.5" fill="{d}"/><path d="M21 10v3M27 10v3" stroke="{b}" stroke-width="2" stroke-linecap="round"/></svg>"##,
                b = hex(body),
                d = hex(darken(body, 0.3)),
            )
        }
        "trash" | "trash-full" => {
            let body = mix(c.accent, c.bg, 0.2);
            let lid = darken(body, 0.25);
            let papers = if key == "trash-full" {
                format!(r##"<path d="M16 15l4-6 6 4 4-5 3 7z" fill="{}"/>"##, hex(mix(Color32::WHITE, c.accent, 0.1)))
            } else {
                String::new()
            };
            format!(
                r##"{SVG48}{papers}<path d="M12 15h24l-2.2 25.5a3 3 0 0 1-3 2.5H17.2a3 3 0 0 1-3-2.5z" fill="{b}"/><rect x="9" y="12" width="30" height="5" rx="2.5" fill="{l}"/><path d="M20 22v15M24 22v15M28 22v15" stroke="{l}" stroke-width="2" stroke-linecap="round"/></svg>"##,
                b = hex(body),
                l = hex(lid),
            )
        }
        "network" => {
            let body = mix(c.accent, c.bg, 0.1);
            let g = hex(darken(c.accent, 0.5));
            format!(
                r##"{SVG48}<circle cx="24" cy="24" r="18" fill="{b}"/><g fill="none" stroke="{g}" stroke-width="2" stroke-linecap="round"><circle cx="24" cy="24" r="17"/><ellipse cx="24" cy="24" rx="7.5" ry="17"/><path d="M7 24h34M10 15h28M10 33h28"/></g></svg>"##,
                b = hex(body),
            )
        }
        "recent" => {
            let body = mix(c.accent, c.bg, 0.1);
            let g = hex(darken(c.accent, 0.55));
            format!(r##"{SVG48}<circle cx="24" cy="24" r="18" fill="{b}"/><path d="M24 13v11l7 5" fill="none" stroke="{g}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/></svg>"##, b = hex(body))
        }
        "emblem-link" => format!(
            r##"{SVG48}<circle cx="24" cy="24" r="20" fill="{a}"/><path d="M17 31L31 17M21 17h10v10" fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
            a = hex(darken(c.accent, 0.1))
        ),
        _ => folder(c, ""),
    }
}

fn symbolic(key: &str) -> String {
    let body = match key {
        "s:back" => r##"<path d="M19 12H5M11 6l-6 6 6 6"/>"##,
        "s:forward" => r##"<path d="M5 12h14M13 6l6 6-6 6"/>"##,
        "s:up" => r##"<path d="M12 19V5M6 11l6-6 6 6"/>"##,
        "s:reload" => r##"<path d="M20 12a8 8 0 1 1-2.4-5.7M20 4v5h-5"/>"##,
        "s:home" => r##"<path d="M4 11l8-7 8 7M6 9.5V20h12V9.5M10 20v-5h4v5"/>"##,
        "s:computer" => r##"<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M9 20h6M12 16v4"/>"##,
        "s:drive" => r##"<rect x="3" y="6" width="18" height="12" rx="3"/><path d="M7 14h6"/><circle cx="17" cy="14" r=".6" fill="#fff"/>"##,
        "s:edit" => r##"<path d="M4 20l4.5-1L19 8.5 15.5 5 5 15.5z M13.5 7l3.5 3.5"/>"##,
        "s:search" => r##"<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15l5 5"/>"##,
        "s:grid" => r##"<g fill="#fff" stroke="none"><rect x="4" y="4" width="7" height="7" rx="1.6"/><rect x="13" y="4" width="7" height="7" rx="1.6"/><rect x="4" y="13" width="7" height="7" rx="1.6"/><rect x="13" y="13" width="7" height="7" rx="1.6"/></g>"##,
        "s:list" => r##"<path d="M10 6h10M10 12h10M10 18h10"/><g fill="#fff" stroke="none"><rect x="3.5" y="4.5" width="3" height="3" rx="1"/><rect x="3.5" y="10.5" width="3" height="3" rx="1"/><rect x="3.5" y="16.5" width="3" height="3" rx="1"/></g>"##,
        "s:compact" => r##"<path d="M4 6h4M4 12h4M4 18h4M10 6h4M10 12h4M10 18h4M16 6h4M16 12h4M16 18h4"/>"##,
        "s:folder-new" => r##"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z M12 11v6M9 14h6"/>"##,
        "s:folder" => r##"<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"##,
        "s:add" => r##"<path d="M12 5v14M5 12h14"/>"##,
        "s:terminal" => r##"<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M7 10l3 2.5L7 15M12.5 15.5h4.5"/>"##,
        "s:eye" => r##"<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>"##,
        "s:trash" => r##"<path d="M4 7h16M9.5 7V4.5h5V7M6 7l1 13h10l1-13M10 11v5M14 11v5"/>"##,
        "s:info" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.6v.2"/>"##,
        "s:star" => r##"<path d="M12 3.5l2.6 5.4 5.9.8-4.3 4.1 1 5.8L12 16.8l-5.2 2.8 1-5.8-4.3-4.1 5.9-.8z"/>"##,
        "s:sidebar" => r##"<rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M9.5 4v16"/>"##,
        "s:tree" => r##"<path d="M5 4v13a1 1 0 0 0 1 1h5M5 10h6"/><rect x="13" y="7.5" width="7" height="5" rx="1.2"/><rect x="13" y="15.5" width="7" height="5" rx="1.2"/>"##,
        "s:eject" => r##"<path d="M5 14.5h14L12 6z" fill="#fff"/><path d="M5 19h14"/>"##,
        "s:min" => r##"<path d="M6 12h12"/>"##,
        "s:max" => r##"<rect x="6" y="6" width="12" height="12" rx="1.5"/>"##,
        "s:restore" => r##"<rect x="5" y="9" width="10" height="10" rx="1.5"/><path d="M9 9V6.5A1.5 1.5 0 0 1 10.5 5H17.5A1.5 1.5 0 0 1 19 6.5v7a1.5 1.5 0 0 1-1.5 1.5H15"/>"##,
        "s:close" => r##"<path d="M6 6l12 12M18 6L6 18"/>"##,
        "s:menu" => r##"<path d="M4 7h16M4 12h16M4 17h16"/>"##,
        "s:network" => r##"<circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="3.8" ry="9"/><path d="M3 12h18"/>"##,
        "s:recent" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.5 2.5"/>"##,
        _ => "",
    };
    format!("{SVG24}{body}</svg>")
}

/// The finished SVG for a key, coloured for this palette.
pub fn svg(key: &str, c: &BuiltinColors) -> String {
    if key.starts_with("s:") { symbolic(key) } else { full_colour(key, c) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_renders() {
        let c = BuiltinColors { accent: Color32::from_rgb(0x62, 0xc1, 0xff), bg: Color32::from_rgb(0x0b, 0x0f, 0x11), fg: Color32::WHITE, dark: true };
        let names = [
            "folder", "user-home", "user-desktop", "folder-documents", "folder-download", "folder-music", "folder-pictures",
            "folder-videos", "folder-templates", "folder-publicshare", "folder-remote", "document-open-recent", "user-trash",
            "user-trash-full", "drive-harddisk", "drive-removable-media", "emblem-symbolic-link", "unknown", "image-png",
            "audio-x-generic", "video-mp4", "font-ttf", "application-pdf", "text-x-python", "text-plain", "application-zip",
            "x-office-spreadsheet", "x-office-presentation", "x-office-document", "application-x-executable",
            "go-previous-symbolic", "go-next-symbolic", "go-up-symbolic", "view-refresh-symbolic", "user-home-symbolic",
            "computer-symbolic", "document-edit-symbolic", "edit-find-symbolic", "view-grid-symbolic", "view-list-symbolic",
            "view-compact-symbolic", "folder-new-symbolic", "tab-new-symbolic", "utilities-terminal-symbolic",
            "view-reveal-symbolic", "user-trash-symbolic", "document-properties-symbolic", "bookmark-new-symbolic",
            "sidebar-show-symbolic", "view-list-tree-symbolic", "media-eject-symbolic", "window-minimize-symbolic",
            "window-maximize-symbolic", "window-restore-symbolic", "window-close-symbolic", "open-menu-symbolic",
        ];
        for n in names {
            let (k, _) = lookup(n).unwrap_or_else(|| panic!("no built-in icon for {n}"));
            let img = crate::icons::render_svg(svg(k, &c).as_bytes(), 32).unwrap_or_else(|| panic!("{n} did not render"));
            assert!(img.pixels.iter().any(|p| p.a() > 0), "{n} is empty");
        }
        assert_eq!(lookup("text-x-generic"), Some(("file-text", false)));
        assert_eq!(lookup("firefox"), None);
    }
}
