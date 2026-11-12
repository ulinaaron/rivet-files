//! Preferences saved as simple key=value lines in ~/.config/rivet/settings.

use crate::places;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewMode {
    Icons,
    List,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortKey {
    Name,
    Size,
    Type,
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub view: ViewMode,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub folders_first: bool,
    pub show_hidden: bool,
    pub zoom: f32, // icon size in points for the icon view
    pub list_zoom: f32,
    pub decorations: bool,
    pub icon_theme: String, // empty = follow the system
    pub theme: ThemeChoice,
    pub show_sidebar: bool,
    pub sidebar_tree: bool,
    pub show_statusbar: bool,
    pub show_menubar: bool,
    pub show_thumbnails: bool,
    pub single_click: bool,
    pub confirm_trash: bool,
    pub accent: [u8; 3],
    /// Use the desktop's generated colours (matugen, Noctalia, GTK theme) when available.
    pub follow_desktop_colors: bool,
    /// Accent currently supplied by the desktop palette (not saved).
    pub live_accent: Option<[u8; 3]>,
    pub window_w: f32,
    pub window_h: f32,
    pub sidebar_width: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            view: ViewMode::Icons,
            sort: SortKey::Name,
            sort_desc: false,
            folders_first: true,
            show_hidden: false,
            zoom: 64.0,
            list_zoom: 22.0,
            decorations: true,
            icon_theme: String::new(),
            theme: ThemeChoice::System,
            show_sidebar: true,
            sidebar_tree: false,
            show_statusbar: true,
            show_menubar: true,
            show_thumbnails: true,
            single_click: false,
            confirm_trash: false,
            accent: [0x35, 0x84, 0xe4],
            follow_desktop_colors: true,
            live_accent: None,
            window_w: 1000.0,
            window_h: 640.0,
            sidebar_width: 190.0,
        }
    }
}

fn path() -> std::path::PathBuf {
    places::config_home().join("rivet/settings")
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(path()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            let b = v == "true";
            let f = v.parse::<f32>().ok();
            match k.trim() {
                "view" => s.view = match v { "list" => ViewMode::List, "compact" => ViewMode::Compact, _ => ViewMode::Icons },
                "sort" => s.sort = match v { "size" => SortKey::Size, "type" => SortKey::Type, "modified" => SortKey::Modified, _ => SortKey::Name },
                "sort_desc" => s.sort_desc = b,
                "folders_first" => s.folders_first = b,
                "show_hidden" => s.show_hidden = b,
                "zoom" => s.zoom = f.unwrap_or(s.zoom).clamp(24.0, 256.0),
                "list_zoom" => s.list_zoom = f.unwrap_or(s.list_zoom).clamp(16.0, 64.0),
                "decorations" => s.decorations = b,
                "icon_theme" => s.icon_theme = v.to_string(),
                "theme" => s.theme = match v { "light" => ThemeChoice::Light, "dark" => ThemeChoice::Dark, _ => ThemeChoice::System },
                "show_sidebar" => s.show_sidebar = b,
                "sidebar_tree" => s.sidebar_tree = b,
                "show_statusbar" => s.show_statusbar = b,
                "show_menubar" => s.show_menubar = b,
                "show_thumbnails" => s.show_thumbnails = b,
                "single_click" => s.single_click = b,
                "confirm_trash" => s.confirm_trash = b,
                "follow_desktop_colors" => s.follow_desktop_colors = b,
                "accent" => {
                    let h = v.trim_start_matches('#');
                    if h.len() == 6 {
                        if let Ok(n) = u32::from_str_radix(h, 16) {
                            s.accent = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
                        }
                    }
                }
                "window_w" => s.window_w = f.unwrap_or(s.window_w).max(400.0),
                "window_h" => s.window_h = f.unwrap_or(s.window_h).max(300.0),
                "sidebar_width" => s.sidebar_width = f.unwrap_or(s.sidebar_width).clamp(120.0, 500.0),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let view = match self.view { ViewMode::Icons => "icons", ViewMode::List => "list", ViewMode::Compact => "compact" };
        let sort = match self.sort { SortKey::Name => "name", SortKey::Size => "size", SortKey::Type => "type", SortKey::Modified => "modified" };
        let theme = match self.theme { ThemeChoice::System => "system", ThemeChoice::Light => "light", ThemeChoice::Dark => "dark" };
        let text = format!(
            "view={view}\nsort={sort}\nsort_desc={}\nfolders_first={}\nshow_hidden={}\nzoom={}\nlist_zoom={}\ndecorations={}\nicon_theme={}\ntheme={theme}\nshow_sidebar={}\nsidebar_tree={}\nshow_statusbar={}\nshow_menubar={}\nshow_thumbnails={}\nsingle_click={}\nconfirm_trash={}\nfollow_desktop_colors={}\naccent=#{:02x}{:02x}{:02x}\nwindow_w={}\nwindow_h={}\nsidebar_width={}\n",
            self.sort_desc, self.folders_first, self.show_hidden, self.zoom, self.list_zoom, self.decorations,
            self.icon_theme, self.show_sidebar, self.sidebar_tree, self.show_statusbar, self.show_menubar, self.show_thumbnails,
            self.single_click, self.confirm_trash, self.follow_desktop_colors, self.accent[0], self.accent[1], self.accent[2],
            self.window_w, self.window_h, self.sidebar_width
        );
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, text);
    }
}
