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

/// The Peek preview's default height limit.
pub const PEEK_PREVIEW_H: f32 = 280.0;

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
    /// Use Rivet's own SVG icons, recoloured from the desktop palette.
    pub builtin_icons: bool,
    /// UI font family; empty = the desktop's font.
    pub ui_font: String,
    /// The archive format Compress… last used ("zip", "tar.gz"…).
    pub compress_format: String,
    /// UI text size; 0 = follow the desktop's font size.
    pub ui_font_size: f32,
    /// Which palette colour the built-in icons use.
    pub icon_color: IconColor,
    pub theme: ThemeChoice,
    pub show_sidebar: bool,
    /// Sidebar icon size in points; 0 = no icons.
    pub sidebar_icon: f32,
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
    /// Put the sidebar on the right of the window instead of the left.
    pub sidebar_right: bool,
    /// Put the toolbar at the bottom of the window.
    pub toolbar_bottom: bool,
    /// Offer Convert / Resize in the right-click menu (needs ffmpeg).
    pub media_tools: bool,
    /// The Bulk Rename module (F2 / right-click with several items selected).
    pub bulk_rename: bool,
    /// Put the status bar at the top of the window (under the toolbar when both are there).
    pub statusbar_top: bool,
    /// The Peek panel: details and a large preview of the selected item.
    pub show_peek: bool,
    /// Put the Peek panel on the left (default: right).
    pub peek_left: bool,
    pub peek_width: f32,
    /// Tallest the Peek preview may be (drag the grip under it).
    pub peek_preview_h: f32,
    pub toolbar: Vec<ToolItem>,
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
            builtin_icons: false,
            ui_font: String::new(),
            compress_format: "zip".into(),
            ui_font_size: 0.0,
            icon_color: IconColor::Primary,
            theme: ThemeChoice::System,
            show_sidebar: true,
            sidebar_icon: 16.0,
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
            sidebar_right: false,
            toolbar_bottom: false,
            media_tools: true,
            bulk_rename: true,
            statusbar_top: false,
            show_peek: false,
            peek_left: false,
            peek_width: 300.0,
            peek_preview_h: PEEK_PREVIEW_H,
            toolbar: ToolItem::default_layout(),
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
                "builtin_icons" => s.builtin_icons = b,
                "ui_font" => s.ui_font = v.to_string(),
                "compress_format" => s.compress_format = v.to_string(),
                "ui_font_size" => s.ui_font_size = f.unwrap_or(0.0).clamp(0.0, 28.0),
                "icon_color" => s.icon_color = IconColor::from_id(v),
                "theme" => s.theme = match v { "light" => ThemeChoice::Light, "dark" => ThemeChoice::Dark, _ => ThemeChoice::System },
                "show_sidebar" => s.show_sidebar = b,
                "sidebar_icon" => s.sidebar_icon = match f.unwrap_or(16.0) { x if x <= 0.0 => 0.0, x if x < 18.0 => 16.0, x if x < 22.0 => 20.0, _ => 24.0 },
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
                "toolbar" => s.toolbar = parse_toolbar(v),
                "sidebar_right" => s.sidebar_right = b,
                "toolbar_bottom" => s.toolbar_bottom = b,
                "media_tools" => s.media_tools = b,
                "bulk_rename" => s.bulk_rename = b,
                "statusbar_top" => s.statusbar_top = b,
                "show_peek" => s.show_peek = b,
                "peek_left" => s.peek_left = b,
                "peek_preview_h" => s.peek_preview_h = f.unwrap_or(PEEK_PREVIEW_H).clamp(100.0, 1200.0),
                "peek_width" => s.peek_width = f.unwrap_or(s.peek_width).clamp(200.0, 700.0),
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
            "view={view}\nsort={sort}\nsort_desc={}\nfolders_first={}\nshow_hidden={}\nzoom={}\nlist_zoom={}\ndecorations={}\nicon_theme={}\nbuiltin_icons={}\nui_font={}\nui_font_size={}\nicon_color={}\ntheme={theme}\nshow_sidebar={}\nsidebar_icon={}\nsidebar_tree={}\nshow_statusbar={}\nshow_menubar={}\nshow_thumbnails={}\nsingle_click={}\nconfirm_trash={}\nfollow_desktop_colors={}\naccent=#{:02x}{:02x}{:02x}\nwindow_w={}\nwindow_h={}\nsidebar_width={}\ntoolbar={}\ncompress_format={}\nsidebar_right={}\nshow_peek={}\npeek_left={}\npeek_width={}\npeek_preview_h={}\ntoolbar_bottom={}\nstatusbar_top={}\nmedia_tools={}\nbulk_rename={}\n",
            self.sort_desc, self.folders_first, self.show_hidden, self.zoom, self.list_zoom, self.decorations,
            self.icon_theme, self.builtin_icons, self.ui_font, self.ui_font_size, self.icon_color.id(), self.show_sidebar, self.sidebar_icon, self.sidebar_tree, self.show_statusbar, self.show_menubar, self.show_thumbnails,
            self.single_click, self.confirm_trash, self.follow_desktop_colors, self.accent[0], self.accent[1], self.accent[2],
            self.window_w, self.window_h, self.sidebar_width, toolbar_string(&self.toolbar), self.compress_format,
            self.sidebar_right, self.show_peek, self.peek_left, self.peek_width, self.peek_preview_h, self.toolbar_bottom, self.statusbar_top, self.media_tools, self.bulk_rename
        );
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, text);
    }
}

/// Something that can be placed on the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolItem {
    Back,
    Forward,
    Up,
    Reload,
    Home,
    Computer,
    EditLocation,
    PathBar,
    Search,
    ViewIcons,
    ViewList,
    ViewCompact,
    NewFolder,
    NewTab,
    Terminal,
    ShowHidden,
    Trash,
    Properties,
    Bookmark,
    Sidebar,
    Peek,
    Separator,
    /// The hamburger menu, shown only while the menu bar is hidden.
    Menu,
}

impl ToolItem {
    pub const ALL: [ToolItem; 23] = [
        ToolItem::Back,
        ToolItem::Forward,
        ToolItem::Up,
        ToolItem::Reload,
        ToolItem::Home,
        ToolItem::Computer,
        ToolItem::EditLocation,
        ToolItem::PathBar,
        ToolItem::Search,
        ToolItem::ViewIcons,
        ToolItem::ViewList,
        ToolItem::ViewCompact,
        ToolItem::NewFolder,
        ToolItem::NewTab,
        ToolItem::Terminal,
        ToolItem::ShowHidden,
        ToolItem::Trash,
        ToolItem::Properties,
        ToolItem::Bookmark,
        ToolItem::Sidebar,
        ToolItem::Peek,
        ToolItem::Separator,
        ToolItem::Menu,
    ];

    /// The default layout (classic file-manager toolbar).
    pub fn default_layout() -> Vec<ToolItem> {
        use ToolItem::*;
        vec![Back, Forward, Up, Separator, Reload, Home, Computer, Separator, EditLocation, PathBar, Search, Separator, ViewIcons, ViewList, ViewCompact, Menu]
    }

    pub fn id(self) -> &'static str {
        use ToolItem::*;
        match self {
            Back => "back",
            Forward => "forward",
            Up => "up",
            Reload => "reload",
            Home => "home",
            Computer => "computer",
            EditLocation => "edit-location",
            PathBar => "path",
            Search => "search",
            ViewIcons => "view-icons",
            ViewList => "view-list",
            ViewCompact => "view-compact",
            NewFolder => "new-folder",
            NewTab => "new-tab",
            Terminal => "terminal",
            ShowHidden => "show-hidden",
            Trash => "trash",
            Properties => "properties",
            Bookmark => "bookmark",
            Sidebar => "sidebar",
            Peek => "peek",
            Separator => "separator",
            Menu => "menu",
        }
    }

    pub fn label(self) -> &'static str {
        use ToolItem::*;
        match self {
            Back => "Back",
            Forward => "Forward",
            Up => "Parent folder",
            Reload => "Reload",
            Home => "Home",
            Computer => "File System",
            EditLocation => "Type a location",
            PathBar => "Location bar",
            Search => "Search",
            ViewIcons => "Icon view",
            ViewList => "List view",
            ViewCompact => "Compact view",
            NewFolder => "New folder",
            NewTab => "New tab",
            Terminal => "Open in terminal",
            ShowHidden => "Show hidden files",
            Trash => "Trash",
            Properties => "Properties",
            Bookmark => "Bookmark this folder",
            Sidebar => "Show/hide sidebar",
            Peek => "Show/hide Peek panel",
            Separator => "Gap",
            Menu => "Menu (when the menu bar is hidden)",
        }
    }

    /// Icon names to try, plus a text fallback.
    pub fn icons(self) -> (&'static [&'static str], &'static str) {
        use ToolItem::*;
        match self {
            Back => (&["go-previous-symbolic", "go-previous"], "◀"),
            Forward => (&["go-next-symbolic", "go-next"], "▶"),
            Up => (&["go-up-symbolic", "go-up"], "▲"),
            Reload => (&["view-refresh-symbolic", "view-refresh"], "⟳"),
            Home => (&["user-home-symbolic", "go-home-symbolic", "user-home"], "⌂"),
            Computer => (&["computer-symbolic", "drive-harddisk-symbolic", "computer"], "/"),
            EditLocation => (&["document-edit-symbolic", "edit-symbolic", "text-editor-symbolic"], "✎"),
            PathBar => (&["folder-symbolic", "folder"], "…"),
            Search => (&["edit-find-symbolic", "system-search-symbolic", "edit-find"], "🔍"),
            ViewIcons => (&["view-grid-symbolic", "view-app-grid-symbolic"], "▦"),
            ViewList => (&["view-list-symbolic", "view-list"], "☰"),
            ViewCompact => (&["view-compact-symbolic", "view-more-horizontal-symbolic"], "▥"),
            NewFolder => (&["folder-new-symbolic", "folder-new"], "+"),
            NewTab => (&["tab-new-symbolic", "tab-new", "list-add-symbolic"], "+"),
            Terminal => (&["utilities-terminal-symbolic", "terminal-symbolic", "utilities-terminal"], ">_"),
            ShowHidden => (&["view-reveal-symbolic", "view-visible-symbolic", "eye-open-symbolic"], "👁"),
            Trash => (&["user-trash-symbolic", "user-trash"], "🗑"),
            Properties => (&["document-properties-symbolic", "dialog-information-symbolic", "document-properties"], "i"),
            Bookmark => (&["bookmark-new-symbolic", "starred-symbolic", "bookmark-new"], "★"),
            Sidebar => (&["sidebar-show-symbolic", "view-sidebar-symbolic", "view-dual-symbolic"], "◧"),
            Peek => (&["view-reveal-symbolic", "view-visible-symbolic", "document-properties-symbolic"], "i"),
            Separator => (&[], "|"),
            Menu => (&[], "≡"),
        }
    }

    pub fn from_id(s: &str) -> Option<ToolItem> {
        ToolItem::ALL.into_iter().find(|t| t.id() == s)
    }

    /// Approximate width on the toolbar, used to size the location bar.
    pub fn width(self) -> f32 {
        match self {
            ToolItem::Separator => 12.0,
            ToolItem::PathBar => 0.0,
            _ => 39.0,
        }
    }
}

pub fn parse_toolbar(v: &str) -> Vec<ToolItem> {
    let mut out: Vec<ToolItem> = Vec::new();
    for t in v.split(',').filter_map(|x| ToolItem::from_id(x.trim())) {
        if t == ToolItem::Separator || !out.contains(&t) {
            out.push(t);
        }
    }
    if !out.contains(&ToolItem::PathBar) {
        out.push(ToolItem::PathBar);
    }
    // Older layouts had the menu button fixed at the far right.
    if !out.contains(&ToolItem::Menu) {
        out.push(ToolItem::Menu);
    }
    out
}

pub fn toolbar_string(v: &[ToolItem]) -> String {
    v.iter().map(|t| t.id()).collect::<Vec<_>>().join(",")
}

/// The palette colour Rivet's own icons are painted with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconColor {
    Primary,
    Secondary,
    Tertiary,
}

impl IconColor {
    pub const ALL: [IconColor; 3] = [IconColor::Primary, IconColor::Secondary, IconColor::Tertiary];
    pub fn id(self) -> &'static str {
        match self {
            IconColor::Primary => "primary",
            IconColor::Secondary => "secondary",
            IconColor::Tertiary => "tertiary",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            IconColor::Primary => "Primary",
            IconColor::Secondary => "Secondary",
            IconColor::Tertiary => "Tertiary",
        }
    }
    pub fn from_id(v: &str) -> Self {
        match v {
            "secondary" => IconColor::Secondary,
            "tertiary" => IconColor::Tertiary,
            _ => IconColor::Primary,
        }
    }
}
