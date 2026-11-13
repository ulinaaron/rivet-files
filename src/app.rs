//! The file manager window: state, navigation, actions and keyboard handling.
//! Drawing lives in the child modules (chrome, views, dialogs).

use eframe::egui::{self, Color32, CornerRadius, Key, Pos2, Rect, Stroke, StrokeKind, Vec2};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::fsops::{self, OpKind, Progress};
use crate::icons::{self, IconTheme};
use crate::launch::{self, DesktopApp};
use crate::mime::MimeDb;
use crate::model::{Entry, Location, Tab};
use crate::places::{self, Bookmark, Mount, UserDirs};
use crate::settings::{Settings, ThemeChoice, ViewMode};
use crate::palette::{self, mix, Palette};
use crate::thumbs::{self, Thumbnailer};

mod chrome;
mod bulkrename;
mod dialogs;
mod mediatools;
mod netview;
mod peek;
mod quickview;
mod views;
mod windows;

#[derive(Clone)]
struct DragFiles(Vec<PathBuf>);

enum Action {
    Open(PathBuf),
    OpenInNewTab(PathBuf),
    Navigate(Location),
    OpenWithDialog(Vec<PathBuf>),
    Launch(usize, Vec<PathBuf>),
    Cut,
    Copy,
    Paste(Option<PathBuf>),
    Rename(PathBuf),
    Trash,
    Delete,
    Undo,
    Redo,
    /// The Delete key: ask, offering Move to Trash (default) or Delete Permanently.
    AskDelete,
    Restore,
    EmptyTrash,
    NewFolder,
    NewFile(Option<PathBuf>),
    Properties(Option<PathBuf>),
    Terminal(PathBuf),
    AddBookmark(PathBuf),
    RemoveBookmark(usize),
    RenameBookmark(usize),
    Transfer(Vec<PathBuf>, PathBuf, bool),
    SelectAll,
    CopyLocation,
    Unmount(Mount),
    CloseTab(usize),
    NewTab,
    Run(PathBuf),
    /// Open the Preview light box on this file.
    QuickView(PathBuf),
    /// Rename several items at once.
    BulkRename(Vec<PathBuf>),
    /// Convert pictures / videos / audio to another format (ffmpeg).
    Convert(Vec<PathBuf>),
    /// Resize pictures (ffmpeg).
    Resize(Vec<PathBuf>),
    /// Ask for a name and format, then make an archive of these items.
    Compress(Vec<PathBuf>),
    /// Unpack these archives next to themselves.
    ExtractHere(Vec<PathBuf>),
}

/// The pages of the Preferences window.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum PrefsTab {
    #[default]
    Appearance,
    Views,
    Behavior,
    Menu,
    Modules,
    System,
    About,
}

impl PrefsTab {
    pub const ALL: [PrefsTab; 7] = [PrefsTab::Appearance, PrefsTab::Views, PrefsTab::Behavior, PrefsTab::Menu, PrefsTab::Modules, PrefsTab::System, PrefsTab::About];

    pub fn label(self) -> &'static str {
        match self {
            PrefsTab::Appearance => "Appearance",
            PrefsTab::Views => "Views",
            PrefsTab::Behavior => "Behavior",
            PrefsTab::Menu => "Menu",
            PrefsTab::Modules => "Modules",
            PrefsTab::System => "System",
            PrefsTab::About => "About",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PrefsTab::Menu => "Menu and toolbar",
            t => t.label(),
        }
    }

    pub fn icons(self) -> &'static [&'static str] {
        match self {
            PrefsTab::Appearance => &["preferences-desktop-appearance-symbolic", "applications-graphics-symbolic", "color-select-symbolic"],
            PrefsTab::Views => &["view-grid-symbolic", "view-app-grid-symbolic", "view-list-symbolic"],
            PrefsTab::Behavior => &["input-mouse-symbolic", "preferences-desktop-peripherals-symbolic", "preferences-system-symbolic"],
            PrefsTab::Menu => &["open-menu-symbolic", "view-more-symbolic", "application-menu-symbolic"],
            PrefsTab::Modules => &["application-x-addon-symbolic", "extension-symbolic", "applications-other-symbolic"],
            PrefsTab::System => &["computer-symbolic", "preferences-system-symbolic", "drive-harddisk-symbolic"],
            PrefsTab::About => &["help-about-symbolic", "dialog-information-symbolic"],
        }
    }

    fn from_id(s: &str) -> Option<PrefsTab> {
        PrefsTab::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(s))
    }
}

enum Dialog {
    None,
    Rename { path: PathBuf, text: String, focus: bool },
    NewItem { folder: bool, text: String, focus: bool, template: Option<PathBuf> },
    ConfirmDelete(Box<DeleteAsk>),
    ConfirmEmptyTrash,
    OpenWith { paths: Vec<PathBuf>, mime: String, show_all: bool, filter: String, chosen: Option<usize>, remember: bool, focus: bool },
    RenameBookmark { idx: usize, text: String },
    Message(String, String),
    Shortcuts,
    Compress { paths: Vec<PathBuf>, name: String, format: crate::archive::Format, focus: bool },
    BulkRename(Box<bulkrename::BulkState>),
    Convert(Box<mediatools::ConvertState>),
    Resize(Box<mediatools::ResizeState>),
}

/// What the delete confirmation shows.
struct DeleteAsk {
    paths: Vec<PathBuf>,
    /// Offer "Move to Trash" (not inside the Trash itself).
    allow_trash: bool,
    /// Enter means "Delete Permanently" (Shift+Delete, or already in the Trash).
    permanent_default: bool,
    folders: usize,
    files: usize,
    bytes: u64,
    /// Size counting stopped early (very big folders).
    partial: bool,
}

struct PropsState {
    path: PathBuf,
    entry: Option<Entry>,
    name: String,
    mode: u32,
    size: Option<(u64, u64, bool)>,
    owner: String,
    group: String,
    tab: u8,
}

pub struct RivetApp {
    s: Settings,
    dirty_settings: bool,
    icons: IconTheme,
    system_icon_theme: String,
    installed_themes: Vec<String>,
    mime: MimeDb,
    thumbs: Thumbnailer,
    apps: Vec<DesktopApp>,
    tabs: Vec<Tab>,
    active: usize,
    clipboard: Option<(Vec<PathBuf>, bool)>,
    /// The Network page (servers, shares, connecting).
    net: netview::NetState,
    /// The Peek panel's preview and video player.
    peek: peek::PeekState,
    /// The Preview light box, when open.
    quick: Option<quickview::QuickView>,
    net_scan_rx: Option<std::sync::mpsc::Receiver<netview::NetResult>>,
    connect_ask: Option<netview::ConnectAsk>,
    /// Undo / redo for file actions.
    history: crate::undo::History,
    ops: Vec<Arc<Progress>>,
    dialog: Dialog,
    prefs_open: bool,
    prefs_win: windows::WinState,
    /// The page shown in Preferences.
    prefs_tab: PrefsTab,
    /// Folder listing behind the location bar's suggestions (folder, its sub-folders).
    path_suggest: Option<(PathBuf, Vec<String>)>,
    /// Open Properties windows (several at once are fine).
    props: Vec<(windows::WinState, PropsState)>,
    next_win_id: u64,
    /// Where in-app windows are this frame, and whether one has the keyboard.
    window_rects: Vec<Rect>,
    window_focus: bool,
    /// Preferences has the keyboard (opened by keyboard, or last clicked).
    prefs_focus: bool,
    /// What was typed into the font list's search field.
    font_filter: String,
    /// A text field or drop-down list in Preferences was active last frame (Esc then only
    /// leaves it, instead of closing Preferences).
    prefs_busy_last: bool,
    /// The file list's keyboard stop, and whether focus is being moved with the keyboard
    /// (then a focus ring shows where it is).
    file_list_focus: Option<egui::Id>,
    kb_nav: bool,
    prefs_rect: Option<Rect>,
    path_editing: bool,
    path_text: String,
    path_focus: bool,
    /// Focus the location field with the caret at the end (no selection).
    path_caret_end: bool,
    search_open: bool,
    search_focus: bool,
    user_dirs: UserDirs,
    bookmarks: Vec<Bookmark>,
    mounts: Vec<Mount>,
    trash_empty: bool,
    places_checked: Instant,
    status_msg: Option<(String, Instant)>,
    pending: Vec<Action>,
    screenshot: Option<PathBuf>,
    frames: u32,
    /// Time spent building each frame (for RIVET_FRAME_STATS).
    frame_times: Vec<f64>,
    first_frame_ms: Option<f64>,
    stats_printed: bool,
    shot_requested: bool,
    shot_done: Arc<AtomicBool>,
    last_title: String,
    applied_visuals: Option<String>,
    palette: Option<Palette>,
    color_watch: palette::Watcher,
    /// Background change detection; the window never polls on its own.
    watch: crate::watch::Watch,
    sidebar_fill: Option<Color32>,
    /// Bumped whenever a movable window opens so it starts centred again.
    popup_serial: u64,
    /// Rivet's own icon, for the About window.
    app_icon: Option<egui::TextureHandle>,
    /// The sample folder used for screenshots (removed on exit).
    demo_dir: Option<PathBuf>,
    /// The font choice currently loaded (to avoid reloading every frame) and its resolved name.
    applied_font: Option<String>,
    font_name: String,
    font_families: Vec<String>,
    toolbar_drawing: bool,
    /// Menus opened from the hamburger button get roomier items.
    roomy_menus: bool,
    /// Draw tool buttons without a background (toolbar, status bar).
    flat_buttons: bool,
    last_decorations: bool,
    collapsed: std::collections::HashSet<String>,
    tree_expanded: std::collections::HashSet<PathBuf>,
    tree_children: std::collections::HashMap<PathBuf, Vec<PathBuf>>,
}

/// One radius scale for the whole app. Nested shapes use the next step down by the
/// padding between them, so corners stay concentric (e.g. a MD bar with 4 px
/// padding holds SM pills).
pub(crate) const R_XS: u8 = 4; // tiny things: swatches, selection band
pub(crate) const R_SM: u8 = 6; // controls: buttons, pills, tabs, items, list rows
pub(crate) const R_MD: u8 = 10; // bars and cards: location bar, sidebar rows, menus, settings cards
pub(crate) const R_LG: u8 = 14; // big surfaces: the file area, in-app windows

const ACCENTS: [([u8; 3], &str); 7] = [
    ([0x35, 0x84, 0xe4], "Blue"),
    ([0x5e, 0x9c, 0x3c], "Green"),
    ([0x1f, 0x9e, 0x9a], "Teal"),
    ([0x91, 0x41, 0xac], "Purple"),
    ([0xe6, 0x61, 0x00], "Orange"),
    ([0xc0, 0x1c, 0x28], "Red"),
    ([0x77, 0x76, 0x7b], "Grey"),
];

/// Info / danger / warning / success: a fixed base hue (70%) tinted by the theme accent (30%),
/// so action colors belong to the current palette.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StatusColors {
    pub info: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub success: Color32,
}

impl StatusColors {
    pub fn from_theme(theme: Color32) -> Self {
        let m = |b: Color32| mix(b, theme, 0.3);
        StatusColors {
            info: m(Color32::from_rgb(0x35, 0x84, 0xe4)),
            danger: m(Color32::from_rgb(0xe0, 0x1b, 0x24)),
            warning: m(Color32::from_rgb(0xe5, 0xa5, 0x0a)),
            success: m(Color32::from_rgb(0x26, 0xa2, 0x69)),
        }
    }

    /// Readable text on a filled button of this color.
    pub fn on(c: Color32) -> Color32 {
        if palette::luminance(c) > 0.62 { Color32::from_gray(20) } else { Color32::WHITE }
    }
}

/// The current status colors (set whenever the visuals change).
pub(crate) fn status(ctx: &egui::Context) -> StatusColors {
    ctx.data(|d| d.get_temp::<StatusColors>(egui::Id::unique("rivet-status-colors")))
        .unwrap_or_else(|| StatusColors::from_theme(Color32::from_rgb(0x35, 0x84, 0xe4)))
}

fn accent_color(s: &Settings) -> Color32 {
    let a = s.live_accent.unwrap_or(s.accent);
    Color32::from_rgb(a[0], a[1], a[2])
}

fn fmt_time(t: Option<std::time::SystemTime>) -> String {
    match t {
        Some(t) => {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            let now = chrono::Local::now();
            if dt.date_naive() == now.date_naive() {
                dt.format("%H:%M").to_string()
            } else if (now - dt).num_days() < 7 && dt <= now {
                dt.format("%a %H:%M").to_string()
            } else {
                dt.format("%-d %b %Y").to_string()
            }
        }
        None => "—".into(),
    }
}

fn user_name(uid: u32) -> String {
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() || (*pw).pw_name.is_null() {
            return uid.to_string();
        }
        std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().to_string()
    }
}

fn group_name(gid: u32) -> String {
    unsafe {
        let gr = libc::getgrgid(gid);
        if gr.is_null() || (*gr).gr_name.is_null() {
            return gid.to_string();
        }
        std::ffi::CStr::from_ptr((*gr).gr_name).to_string_lossy().to_string()
    }
}

fn step_in(steps: &[f32], cur: f32, dir: i32) -> f32 {
    let idx = steps.iter().position(|&s| s >= cur - 0.5).unwrap_or(steps.len() - 1) as i32;
    steps[(idx + dir).clamp(0, steps.len() as i32 - 1) as usize]
}

fn desktop_icon(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    text.lines().find_map(|l| l.strip_prefix("Icon=").map(|s| s.trim().to_string()))
}

/// The shared "selected" look (selected files, active view and sidebar-mode buttons):
/// the surface tinted with the accent plus a thin accent outline. `bg` is the surface underneath.
pub(crate) fn selected_tile(v: &egui::Visuals, bg: Color32) -> (Color32, Stroke) {
    (crate::palette::mix(bg, v.selection.bg_fill, 0.30), Stroke::new(1.0, v.selection.bg_fill.gamma_multiply(0.65)))
}

fn paint_tex(painter: &egui::Painter, tex: &egui::TextureHandle, rect: Rect, tint: Color32) {
    let size = tex.size_vec2();
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let r = Rect::from_center_size(rect.center(), size * scale);
    painter.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
}

impl RivetApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings, start: Option<PathBuf>, screenshot: Option<PathBuf>, demo: Option<PathBuf>) -> Self {
        // In screenshot mode nothing personal is shown: no bookmarks, no drive names.
        let private = screenshot.is_some();
        let ctx = cc.egui_ctx.clone();
        let system_icon_theme = icons::detect_system_theme();
        let theme_name = if settings.icon_theme.is_empty() { system_icon_theme.clone() } else { settings.icon_theme.clone() };
        let mime = MimeDb::load();
        let start_loc = match start {
            Some(p) if p.to_string_lossy().starts_with("network:") => Location::Network,
            Some(p) if p.is_dir() => Location::Dir(std::fs::canonicalize(&p).unwrap_or(p)),
            Some(p) if p.is_file() => Location::Dir(p.parent().map(|x| x.to_path_buf()).unwrap_or_else(places::home)),
            _ => Location::Dir(places::home()),
        };
        let mut tab = Tab::new(start_loc);
        tab.reload(&mime, &settings);
        let last_decorations = settings.decorations;
        let (pal, watched) = palette::load();
        let mut app = RivetApp {
            icons: IconTheme::new(&theme_name),
            system_icon_theme,
            installed_themes: Vec::new(),
            thumbs: Thumbnailer::new(ctx.clone()),
            apps: launch::all_apps(),
            tabs: vec![tab],
            active: 0,
            clipboard: None,
            net: Default::default(),
            peek: Default::default(),
            quick: None,
            net_scan_rx: None,
            connect_ask: None,
            history: Default::default(),
            ops: Vec::new(),
            dialog: Dialog::None,
            prefs_open: false,
            prefs_win: windows::WinState::new(1, Vec2::new(800.0, 640.0)),
            prefs_tab: PrefsTab::default(),
            path_suggest: None,
            props: Vec::new(),
            next_win_id: 10,
            window_rects: Vec::new(),
            window_focus: false,
            prefs_focus: false,
            font_filter: String::new(),
            prefs_busy_last: false,
            file_list_focus: None,
            kb_nav: false,
            prefs_rect: None,
            path_editing: false,
            path_text: String::new(),
            path_focus: false,
            path_caret_end: false,
            search_open: false,
            search_focus: false,
            user_dirs: places::user_dirs(),
            bookmarks: if private { Vec::new() } else { places::load_bookmarks() },
            mounts: if private { Vec::new() } else { places::mounts() },
            demo_dir: demo,
            trash_empty: places::trash_is_empty(),
            places_checked: Instant::now(),
            status_msg: None,
            pending: Vec::new(),
            screenshot,
            frames: 0,
            frame_times: Vec::new(),
            first_frame_ms: None,
            stats_printed: false,
            shot_requested: false,
            shot_done: Arc::new(AtomicBool::new(false)),
            last_title: String::new(),
            applied_visuals: None,
            palette: pal,
            color_watch: palette::Watcher::new(watched),
            watch: crate::watch::Watch::spawn(ctx.clone()),
            sidebar_fill: None,
            popup_serial: 0,
            app_icon: None,
            applied_font: None,
            font_name: String::new(),
            font_families: Vec::new(),
            toolbar_drawing: false,
            roomy_menus: false,
            flat_buttons: false,
            last_decorations,
            collapsed: Default::default(),
            tree_expanded: [places::home()].into_iter().collect(),
            tree_children: Default::default(),
            mime,
            s: settings,
            dirty_settings: false,
        };
        app.apply_visuals(&ctx);
        // Convert & Resize needs ffmpeg: switch the module off when it isn't installed.
        if app.s.media_tools && !crate::media::available() {
            app.s.media_tools = false;
            app.dirty_settings = true;
        }
        // Test hooks for checks: open a window straight away.
        if std::env::var_os("RIVET_OPEN_PREFS").is_some() {
            app.prefs_open = true;
            app.popup_serial += 1;
        }
        // RIVET_PREFS_TAB=views|behavior|menu|system|about opens Preferences on that page.
        if let Some(t) = std::env::var("RIVET_PREFS_TAB").ok().and_then(|t| PrefsTab::from_id(&t)) {
            app.prefs_tab = t;
            app.open_prefs();
        }
        if let Some(p) = std::env::var_os("RIVET_EXTRA_TAB") {
            app.new_tab(Location::Dir(PathBuf::from(p)));
        }
        if let Ok(list) = std::env::var("RIVET_SELECT") {
            let t = &mut app.tabs[app.active];
            for n in list.split(',').filter_map(|v| v.trim().parse::<usize>().ok()) {
                if let Some(&i) = t.visible.get(n) {
                    let p = t.entries[i].path.clone();
                    t.selected.insert(p);
                }
            }
        }
        match std::env::var("RIVET_DIALOG").as_deref() {
            Ok("about") => app.open_about(),
            Ok("shortcuts") => app.dialog = Dialog::Shortcuts,
            Ok("connect") => app.open_connect_dialog(),
            Ok("compress") => {
                let sel = app.tab().selected_paths();
                app.pending.push(Action::Compress(sel));
            }
            Ok("openwith") => {
                let sel = app.tab().selected_paths();
                app.pending.push(Action::OpenWithDialog(sel));
            }
            Ok("preview") => {
                if let Some(p) = app.tab().selected_paths().first().cloned() {
                    app.pending.push(Action::QuickView(p));
                }
            }
            Ok("convert") => {
                let sel = app.tab().selected_paths();
                app.pending.push(Action::Convert(sel));
            }
            Ok("resize") => {
                let sel = app.tab().selected_paths();
                app.pending.push(Action::Resize(sel));
            }
            Ok("bulkrename") => {
                let sel = app.tab().selected_paths();
                app.pending.push(Action::BulkRename(sel));
            }
            Ok("rename") => {
                if let Some(p) = app.tab().selected_paths().first().cloned() {
                    app.pending.push(Action::Rename(p));
                }
            }
            Ok("delete") | Ok("delete-permanently") => {
                let sel = app.tab().selected_paths();
                let perm = std::env::var("RIVET_DIALOG").as_deref() == Ok("delete-permanently");
                app.ask_delete(sel, perm);
            }
            _ => {}
        }
        if std::env::var_os("RIVET_SEARCH").is_some() {
            app.search_open = true;
            app.search_focus = true;
        }
        if let Ok(v) = std::env::var("RIVET_EDIT_PATH") {
            app.start_path_editing();
            // RIVET_EDIT_PATH=/some/pa types that text (for checking suggestions).
            if v.starts_with('/') || v.starts_with('~') {
                app.path_text = v;
                app.path_focus = false;
                app.path_caret_end = true;
            }
        }
        if let Ok(list) = std::env::var("RIVET_OPEN_PROPERTIES") {
            for p in list.split(':').filter(|p| !p.is_empty()) {
                app.open_properties(PathBuf::from(p));
            }
        }
        if std::env::var_os("RIVET_PREFS_MAX").is_some() {
            app.prefs_win.maximized = true;
        }
        // RIVET_PROPS_TAB=1 shows the Permissions side of opened Properties windows.
        if std::env::var("RIVET_PROPS_TAB").as_deref() == Ok("1") {
            for (_, st) in app.props.iter_mut() {
                st.tab = 1;
            }
        }
        if std::env::var_os("RIVET_PROPS_ROLLED").is_some() {
            for (w, _) in &mut app.props {
                w.shaded = true;
            }
        }
        app
    }

    fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }
    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Re-read the desktop colours (and icon theme) after a theme tool changed them.
    fn reload_desktop_theme(&mut self) {
        let (pal, watched) = palette::load();
        self.palette = pal;
        self.color_watch.set_files(watched);
        let detected = icons::detect_system_theme();
        if detected != self.system_icon_theme {
            self.system_icon_theme = detected;
            if self.s.icon_theme.is_empty() {
                self.icons = IconTheme::new(&self.system_icon_theme);
            }
        }
    }

    /// Load the chosen UI font (or the desktop's) and set the text scale.
    fn apply_font(&mut self, ctx: &egui::Context) {
        let size = if self.s.ui_font_size > 0.0 {
            self.s.ui_font_size
        } else {
            crate::fonts::desktop_font_cached().map(|(_, pt)| crate::fonts::points_to_ui(pt)).unwrap_or(crate::fonts::BASE_SIZE)
        };
        crate::fonts::set_scale(size / crate::fonts::BASE_SIZE);
        if self.applied_font.as_deref() == Some(self.s.ui_font.as_str()) {
            return;
        }
        self.applied_font = Some(self.s.ui_font.clone());
        let mut defs = egui::FontDefinitions::default();
        if let Some((name, path)) = crate::fonts::resolve(&self.s.ui_font) {
            if let Ok(bytes) = std::fs::read(&path) {
                defs.font_data.insert("ui-font".into(), Arc::new(egui::FontData::from_owned(bytes)));
                defs.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "ui-font".into());
                self.font_name = name;
            }
        }
        ctx.set_fonts(defs);
    }

    /// The theme's three colors: primary (accent), secondary and tertiary.
    fn theme_trio(&self) -> (Color32, Color32, Color32) {
        let primary = accent_color(&self.s);
        let pal = if self.s.follow_desktop_colors { self.palette.as_ref() } else { None };
        (
            primary,
            pal.and_then(|p| p.secondary).unwrap_or_else(|| crate::builtin_icons::derive_secondary(primary)),
            pal.and_then(|p| p.tertiary).unwrap_or_else(|| crate::builtin_icons::derive_tertiary(primary)),
        )
    }

    /// The palette colour picked for Rivet's own icons (primary / secondary / tertiary).
    fn icon_base_color(&self) -> Color32 {
        use crate::settings::IconColor;
        let primary = accent_color(&self.s);
        let pal = if self.s.follow_desktop_colors { self.palette.as_ref() } else { None };
        match self.s.icon_color {
            IconColor::Primary => primary,
            IconColor::Secondary => pal.and_then(|p| p.secondary).unwrap_or_else(|| crate::builtin_icons::derive_secondary(primary)),
            IconColor::Tertiary => pal.and_then(|p| p.tertiary).unwrap_or_else(|| crate::builtin_icons::derive_tertiary(primary)),
        }
    }

    /// Keep Rivet's own icon set (if chosen) coloured like the rest of the window.
    fn sync_builtin_icons(&mut self, ctx: &egui::Context) {
        if !self.s.builtin_icons {
            self.icons.set_builtin(None);
            return;
        }
        let st = ctx.style_of(ctx.theme());
        let v = &st.visuals;
        self.icons.set_builtin(Some(crate::builtin_icons::BuiltinColors {
            accent: self.icon_base_color(),
            bg: v.extreme_bg_color,
            fg: v.text_color(),
            dark: v.dark_mode,
        }));
    }

    fn apply_visuals(&mut self, ctx: &egui::Context) {
        let pal = if self.s.follow_desktop_colors { self.palette.clone() } else { None };
        let key = format!("{:?}{:?}{:?}{}", self.s.theme, self.s.accent, pal, crate::fonts::scale());
        if self.applied_visuals.as_ref() == Some(&key) {
            return;
        }
        self.applied_visuals = Some(key);
        self.s.live_accent = pal.as_ref().and_then(|p| p.accent).map(|c| [c.r(), c.g(), c.b()]);
        self.sidebar_fill = pal.as_ref().and_then(|p| p.sidebar_bg);
        ctx.set_theme(match (&pal, self.s.theme) {
            // A generated palette is made for one mode; follow it.
            (Some(p), _) => {
                if p.is_dark() { egui::ThemePreference::Dark } else { egui::ThemePreference::Light }
            }
            (None, ThemeChoice::System) => egui::ThemePreference::System,
            (None, ThemeChoice::Light) => egui::ThemePreference::Light,
            (None, ThemeChoice::Dark) => egui::ThemePreference::Dark,
        });
        let accent = accent_color(&self.s);
        let status = StatusColors::from_theme(accent);
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("rivet-status-colors"), status));
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let dark = theme == egui::Theme::Dark;
            // Start from a clean style each time so colours from an old palette never linger.
            let mut style = egui::Style { visuals: if dark { egui::Visuals::dark() } else { egui::Visuals::light() }, ..Default::default() };
            style.visuals.dark_mode = dark;
            ctx.set_style_of(theme, style);
            ctx.style_mut_of(theme, |st| {
                let v = &mut st.visuals;
                v.selection.bg_fill = accent;
                v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
                v.hyperlink_color = accent;
                v.error_fg_color = status.danger;
                v.warn_fg_color = status.warning;
                if dark {
                    // Elevation by surface: content (lowest) < chrome < pop-ups.
                    v.extreme_bg_color = Color32::from_rgb(0x24, 0x24, 0x24);
                    v.panel_fill = Color32::from_rgb(0x30, 0x30, 0x30);
                    v.window_fill = Color32::from_rgb(0x3a, 0x3a, 0x3a);
                    v.faint_bg_color = Color32::from_rgb(0x2b, 0x2b, 0x2b);
                } else {
                    // Light: a slightly deeper chrome and darker secondary text, so
                    // edges, labels and pale icons keep enough contrast on white.
                    v.extreme_bg_color = Color32::WHITE;
                    v.panel_fill = Color32::from_rgb(0xea, 0xea, 0xec);
                    v.window_fill = Color32::from_rgb(0xfb, 0xfb, 0xfc);
                    v.faint_bg_color = Color32::from_rgb(0xf3, 0xf3, 0xf5);
                    v.weak_text_color = Some(Color32::from_gray(104));
                    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xe2, 0xe2, 0xe5);
                    // Checkboxes, slider rails and other controls need a visible well on white.
                    v.widgets.inactive.bg_fill = Color32::from_rgb(0xd3, 0xd3, 0xd8);
                    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xdc, 0xdc, 0xe0);
                }
                for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
                    w.corner_radius = CornerRadius::same(R_SM);
                }
                v.window_corner_radius = CornerRadius::same(R_LG);
                v.menu_corner_radius = CornerRadius::same(R_MD);
                // No outlines: pop-ups stand out by their raised surface and shadow.
                v.window_stroke = Stroke::NONE;
                if let Some(p) = pal.as_ref().filter(|p| p.is_dark() == dark) {
                    let bg = p.window_bg.or(p.view_bg).unwrap_or(v.panel_fill);
                    let fg = p.window_fg.or(p.view_fg).unwrap_or(if dark { Color32::from_gray(225) } else { Color32::from_gray(30) });
                    let view = p.view_bg.unwrap_or(bg);
                    // Toolbar, sidebar, menu bar and status bar share one raised surface.
                    v.panel_fill = p.headerbar_bg.or(p.sidebar_bg).unwrap_or_else(|| mix(bg, fg, 0.05));
                    v.extreme_bg_color = view;
                    v.faint_bg_color = mix(view, fg, 0.035);
                    if v.panel_fill == view {
                        v.panel_fill = mix(view, fg, 0.05); // keep the two levels distinguishable
                    }
                    let chrome = v.panel_fill;
                    v.window_fill = p.popover_bg.filter(|c| *c != chrome && *c != view).unwrap_or_else(|| mix(chrome, fg, 0.07));
                    v.text_edit_bg_color = Some(mix(view, fg, 0.05));
                    // Secondary text: lighter than the main text, but readable on light schemes too.
                    v.weak_text_color = Some(mix(fg, view, if dark { 0.4 } else { 0.32 }));
                    v.selection.stroke = Stroke::new(1.0, p.accent_fg.unwrap_or(if palette::luminance(accent) > 0.6 { Color32::BLACK } else { Color32::WHITE }));
                    v.widgets.noninteractive.bg_fill = bg;
                    v.widgets.noninteractive.weak_bg_fill = bg;
                    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, mix(bg, fg, 0.12));
                    for (w, t) in [(&mut v.widgets.inactive, 0.08), (&mut v.widgets.hovered, 0.15), (&mut v.widgets.active, 0.22), (&mut v.widgets.open, 0.12)] {
                        w.bg_fill = mix(bg, fg, t);
                        w.weak_bg_fill = mix(bg, fg, t);
                    }
                    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
                        w.fg_stroke.color = fg;
                    }
                    v.widgets.hovered.bg_stroke = Stroke::new(1.0, mix(bg, fg, 0.25));
                    if let Some(d) = p.destructive {
                        v.error_fg_color = d;
                    }
                }
                st.spacing.item_spacing = Vec2::new(6.0, 4.0);
                st.spacing.button_padding = Vec2::new(6.0, 3.0);
                st.spacing.interact_size.y = 24.0;
                // A little more room inside every pop-up menu.
                st.spacing.menu_margin = egui::Margin::same(8);
                for (ts, size) in [
                    (egui::TextStyle::Body, 13.5),
                    (egui::TextStyle::Button, 13.5),
                    (egui::TextStyle::Small, 11.0),
                    (egui::TextStyle::Heading, 18.0),
                ] {
                    if let Some(f) = st.text_styles.get_mut(&ts) {
                        f.size = size * crate::fonts::scale();
                    }
                }
            });
        }
    }

    // ---------------------------------------------------------------- icons

    fn icon(&mut self, ctx: &egui::Context, names: &[&str], px: f32) -> Option<egui::TextureHandle> {
        let ppp = ctx.pixels_per_point();
        self.icons.texture(ctx, names, (px * ppp).round().max(8.0) as u32)
    }

    /// An icon for a highlighted row: the very same icon shown when it isn't highlighted,
    /// recolored into one solid color (tint it with the highlight's text color).
    /// Returns (texture, is_single_color).
    fn accent_icon(&mut self, ctx: &egui::Context, names: &[&str], px: f32) -> Option<(egui::TextureHandle, bool)> {
        let ppp = ctx.pixels_per_point();
        self.icons.mask(ctx, names, (px * ppp).round().max(8.0) as u32).map(|t| (t, true))
    }

    fn entry_texture(&mut self, ctx: &egui::Context, e: &Entry, px: f32) -> Option<egui::TextureHandle> {
        if self.s.show_thumbnails && px >= 32.0 && thumbs::can_thumbnail(&e.mime) {
            if let Some(t) = self.thumbs.get(&e.path, e.size) {
                return Some(t);
            }
        }
        self.entry_icon(ctx, e, px, false)
    }

    /// The entry's icon as a single-colour silhouette (tint it), for highlighted rows.
    fn entry_mask(&mut self, ctx: &egui::Context, e: &Entry, px: f32) -> Option<egui::TextureHandle> {
        self.entry_icon(ctx, e, px, true)
    }

    /// On light backgrounds: a soft shadow and a faint edge under a file's icon (or
    /// thumbnail), so white "paper" icons and pale pictures don't melt into the background.
    /// Call before painting the icon itself.
    fn icon_lift(&mut self, ctx: &egui::Context, painter: &egui::Painter, e: &Entry, tex: &egui::TextureHandle, rect: Rect, dark: bool) {
        if dark || rect.width() < 14.0 {
            return;
        }
        let z = rect.width();
        if tex.name().starts_with("thumb:") || tex.name().starts_with("peek:") {
            let size = tex.size_vec2();
            let scale = (rect.width() / size.x).min(rect.height() / size.y);
            let r = Rect::from_center_size(rect.center(), size * scale);
            painter.rect_filled(r.translate(Vec2::new(0.0, 1.0)), 1.5, Color32::from_black_alpha(18));
            painter.rect_stroke(r, 1.0, Stroke::new(0.5, Color32::from_black_alpha(22)), StrokeKind::Outside);
            return;
        }
        let Some(mask) = self.entry_mask(ctx, e, z) else { return };
        // A barely-there edge and a soft shadow just below: enough to separate a white
        // page from a white background without drawing an outline.
        let d = (z / 48.0).clamp(0.5, 1.2);
        for off in [Vec2::new(-0.4, 0.0), Vec2::new(0.4, 0.0), Vec2::new(0.0, -0.4)] {
            paint_tex(painter, &mask, rect.translate(off * d), Color32::from_black_alpha(10));
        }
        paint_tex(painter, &mask, rect.translate(Vec2::new(0.0, 0.8 * d)), Color32::from_black_alpha(20));
        paint_tex(painter, &mask, rect.translate(Vec2::new(0.0, 1.8 * d)), Color32::from_black_alpha(9));
    }

    fn icon_or_mask(&mut self, ctx: &egui::Context, names: &[&str], px: f32, mask: bool) -> Option<egui::TextureHandle> {
        if mask {
            let ppp = ctx.pixels_per_point();
            self.icons.mask(ctx, names, (px * ppp).round().max(8.0) as u32)
        } else {
            self.icon(ctx, names, px)
        }
    }

    fn entry_icon(&mut self, ctx: &egui::Context, e: &Entry, px: f32, mask: bool) -> Option<egui::TextureHandle> {
        if e.is_dir {
            if let Some(special) = places::special_folder_icon(&e.path, &self.user_dirs) {
                return self.icon_or_mask(ctx, &[special, "folder"], px, mask);
            }
            return self.icon_or_mask(ctx, &["folder", "inode-directory"], px, mask);
        }
        if e.broken_link {
            return self.icon_or_mask(ctx, &["emblem-unreadable", "text-x-generic"], px, mask);
        }
        if e.mime == "application/x-desktop" || e.name.ends_with(".desktop") {
            if let Some(icon) = desktop_icon(&e.path) {
                if icon.starts_with('/') {
                    let ppp = ctx.pixels_per_point();
                    let size = (px * ppp) as u32;
                    let t = if mask { self.icons.mask_from_path(ctx, Path::new(&icon), size) } else { self.icons.texture_from_path(ctx, Path::new(&icon), size) };
                    if t.is_some() {
                        return t;
                    }
                } else if let Some(t) = self.icon_or_mask(ctx, &[icon.as_str()], px, mask) {
                    return Some(t);
                }
            }
        }
        let names = self.mime.icon_names(&e.mime);
        let mut refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        if e.is_executable() && !e.mime.starts_with("text/") && !e.mime.contains("script") {
            refs.insert(0, "application-x-executable");
        }
        self.icon_or_mask(ctx, &refs, px, mask)
    }

    // ---------------------------------------------------------------- navigation

    fn navigate(&mut self, loc: Location) {
        if self.tab().loc == loc {
            return;
        }
        let t = &mut self.tabs[self.active];
        let old = std::mem::replace(&mut t.loc, loc);
        t.back.push(old);
        t.forward.clear();
        self.after_location_change(None);
    }

    fn after_location_change(&mut self, select: Option<PathBuf>) {
        let t = &mut self.tabs[self.active];
        t.selected.clear();
        t.anchor = None;
        t.cursor = None;
        t.filter.clear();
        t.search_rx = None;
        t.searching = false;
        t.entries.clear();
        t.reload(&self.mime, &self.s);
        if let Some(p) = select {
            t.selected.insert(p.clone());
            t.cursor = t.visible_index_of(&p);
            t.anchor = t.cursor;
            t.scroll_to = Some(p);
        }
        self.search_open = false;
        self.path_editing = false;
    }

    fn go_back(&mut self) {
        let t = &mut self.tabs[self.active];
        if let Some(prev) = t.back.pop() {
            let cur = std::mem::replace(&mut t.loc, prev);
            let sel = cur.dir().map(|p| p.to_path_buf());
            t.forward.push(cur);
            self.after_location_change(sel);
        }
    }

    fn go_forward(&mut self) {
        let t = &mut self.tabs[self.active];
        if let Some(next) = t.forward.pop() {
            let cur = std::mem::replace(&mut t.loc, next);
            t.back.push(cur);
            self.after_location_change(None);
        }
    }

    fn go_up(&mut self) {
        if let Some(d) = self.tab().loc.dir() {
            if let Some(parent) = d.parent() {
                let parent = parent.to_path_buf();
                let child = d.to_path_buf();
                self.navigate(Location::Dir(parent));
                let t = self.tab_mut();
                t.selected.insert(child.clone());
                t.cursor = t.visible_index_of(&child);
                t.anchor = t.cursor;
                t.scroll_to = Some(child);
            }
        } else {
            self.navigate(Location::Dir(places::home()));
        }
    }

    fn current_dir(&self) -> Option<PathBuf> {
        self.tab().loc.dir().map(|p| p.to_path_buf())
    }

    fn open_entry(&mut self, path: &Path, new_tab: bool) {
        let Some(e) = Entry::from_path(path.to_path_buf(), &self.mime) else {
            self.message("Could not open", &format!("{} no longer exists.", path.display()));
            return;
        };
        if matches!(self.tab().loc, Location::Trash) && !e.is_dir {
            self.flash("Restore the item from the Trash to open it.");
            return;
        }
        if e.is_dir {
            if new_tab {
                self.new_tab(Location::Dir(e.path.clone()));
            } else {
                self.navigate(Location::Dir(e.path.clone()));
            }
            return;
        }
        let native = matches!(e.mime.as_str(), "application/x-executable" | "application/x-pie-executable" | "application/x-sharedlib");
        let res = if e.is_executable() && native { launch::run_executable(&e.path) } else { launch::open_default(&self.apps, &e.mime, &e.path) };
        match res {
            Ok(()) => self.flash(&format!("Opening “{}”…", e.name)),
            Err(err) => self.message("Could not open file", &err),
        }
    }

    fn new_tab(&mut self, loc: Location) {
        let mut t = Tab::new(loc);
        t.reload(&self.mime, &self.s);
        self.tabs.insert(self.active + 1, t);
        self.active += 1;
    }

    fn flash(&mut self, msg: &str) {
        self.status_msg = Some((msg.to_string(), Instant::now()));
    }

    fn message(&mut self, title: &str, body: &str) {
        self.dialog = Dialog::Message(title.into(), body.into());
    }

    fn reload_all(&mut self) {
        for t in &mut self.tabs {
            t.reload(&self.mime, &self.s);
        }
        self.trash_empty = places::trash_is_empty();
    }

    fn resort_all(&mut self) {
        for t in &mut self.tabs {
            t.resort(&self.mime, &self.s);
        }
        self.dirty_settings = true;
    }

    fn start_op(&mut self, kind: OpKind, src: Vec<PathBuf>, dest: Option<PathBuf>) {
        if src.is_empty() && kind != OpKind::EmptyTrash {
            return;
        }
        self.ops.push(fsops::spawn_op(kind, src, dest));
    }

    fn set_theme_name(&mut self, name: String) {
        self.s.icon_theme = name;
        let n = if self.s.icon_theme.is_empty() { self.system_icon_theme.clone() } else { self.s.icon_theme.clone() };
        self.icons = IconTheme::new(&n);
        self.dirty_settings = true;
    }

    fn zoom(&mut self, dir: i32) {
        match self.s.view {
            ViewMode::List | ViewMode::Compact => {
                self.s.list_zoom = step_in(&[16.0, 22.0, 32.0, 48.0, 64.0], self.s.list_zoom, dir);
            }
            ViewMode::Icons => {
                self.s.zoom = step_in(&[32.0, 48.0, 64.0, 96.0, 128.0, 192.0, 256.0], self.s.zoom, dir);
            }
        }
        self.dirty_settings = true;
    }

    fn close_tab(&mut self, i: usize, ctx: &egui::Context) {
        if self.tabs.len() <= 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.tabs.remove(i);
        if self.active >= self.tabs.len() || self.active > i {
            self.active = self.active.saturating_sub(1);
        }
    }

    // ---------------------------------------------------------------- actions

    fn process_actions(&mut self, ctx: &egui::Context) {
        let actions = std::mem::take(&mut self.pending);
        for a in actions {
            match a {
                Action::Open(p) => self.open_entry(&p, false),
                Action::OpenInNewTab(p) => self.open_entry(&p, true),
                Action::Navigate(l) => self.navigate(l),
                Action::OpenWithDialog(paths) => {
                    let mime = paths.first().and_then(|p| Entry::from_path(p.clone(), &self.mime)).map(|e| e.mime).unwrap_or_default();
                    let chosen = launch::default_app(&self.apps, &mime).and_then(|d| self.apps.iter().position(|a| a.id == d.id));
                    self.dialog = Dialog::OpenWith { paths, mime, show_all: false, filter: String::new(), chosen, remember: false, focus: true };
                }
                Action::Launch(i, paths) => {
                    if let Some(app) = self.apps.get(i).cloned() {
                        if let Err(e) = launch::launch_app(&app, &paths) {
                            self.message("Could not open", &e);
                        }
                    }
                }
                Action::Cut | Action::Copy => {
                    let cut = matches!(a, Action::Cut);
                    let sel = self.tab().selected_paths();
                    if !sel.is_empty() && !matches!(self.tab().loc, Location::Trash) {
                        let text = sel.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
                        ctx.copy_text(text);
                        let n = sel.len();
                        self.flash(&format!(
                            "{} {n} item{} — paste to {} {}",
                            if cut { "Cut" } else { "Copied" },
                            if n == 1 { "" } else { "s" },
                            if cut { "move" } else { "copy" },
                            if n == 1 { "it" } else { "them" }
                        ));
                        self.clipboard = Some((sel, cut));
                    }
                }
                Action::Paste(target) => {
                    let Some(dest) = target.or_else(|| self.current_dir()) else { continue };
                    if let Some((files, cut)) = self.clipboard.clone() {
                        self.start_op(if cut { OpKind::Move } else { OpKind::Copy }, files, Some(dest));
                        if cut {
                            self.clipboard = None;
                        }
                    }
                }
                Action::Convert(paths) => {
                    let mimes: Vec<String> = paths.iter().filter_map(|p| Entry::from_path(p.clone(), &self.mime)).map(|e| e.mime).collect();
                    let refs: Vec<&str> = mimes.iter().map(|m| m.as_str()).collect();
                    if let Some(kind) = crate::media::common_kind(&refs) {
                        // Default to the first format the files aren't already in.
                        let current = paths.first().and_then(|p| p.extension()).map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                        let ext = crate::media::formats(kind).iter().map(|(e, _)| *e).find(|e| *e != current && !(current == "jpeg" && *e == "jpg")).unwrap_or("png");
                        self.dialog = Dialog::Convert(Box::new(mediatools::ConvertState { files: paths, kind, ext: ext.to_string(), quality: 90, keep: crate::media::Keep::Originals }));
                    }
                }
                Action::Resize(paths) => {
                    let files: Vec<(PathBuf, u32, u32)> = paths.into_iter().filter_map(|p| image::image_dimensions(&p).ok().map(|(w, h)| (p, w, h))).collect();
                    if !files.is_empty() {
                        self.dialog = Dialog::Resize(Box::new(mediatools::ResizeState { files, by_percent: true, percent: 50, width: 1920, height: 1080, keep: crate::media::Keep::Originals }));
                    }
                }
                Action::BulkRename(paths) => {
                    // In folder order, so numbering follows what you see.
                    let t = self.tab();
                    let items: Vec<(PathBuf, bool, Option<std::time::SystemTime>)> = t
                        .visible
                        .iter()
                        .map(|&i| &t.entries[i])
                        .filter(|e| paths.contains(&e.path))
                        .map(|e| (e.path.clone(), e.is_dir, e.modified))
                        .collect();
                    if !items.is_empty() {
                        self.dialog = Dialog::BulkRename(Box::new(bulkrename::BulkState::new(items)));
                    }
                }
                Action::Rename(p) => {
                    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    self.dialog = Dialog::Rename { path: p, text: name, focus: true };
                }
                Action::Trash => {
                    let sel = self.tab().selected_paths();
                    if sel.is_empty() {
                        continue;
                    }
                    let in_trash = matches!(self.tab().loc, Location::Trash);
                    if in_trash || self.s.confirm_trash {
                        self.ask_delete(sel, in_trash);
                    } else {
                        self.start_op(OpKind::Trash, sel, None);
                    }
                }
                Action::AskDelete => {
                    let sel = self.tab().selected_paths();
                    if !sel.is_empty() {
                        let in_trash = matches!(self.tab().loc, Location::Trash);
                        self.ask_delete(sel, in_trash);
                    }
                }
                Action::Delete => {
                    let sel = self.tab().selected_paths();
                    if !sel.is_empty() {
                        self.ask_delete(sel, true);
                    }
                }
                Action::Undo | Action::Redo => {
                    let undoing = matches!(a, Action::Undo);
                    let step = if undoing { self.history.pop_undo() } else { self.history.pop_redo() };
                    match step {
                        Some(step) => {
                            let what = step.describe();
                            self.run_inverse(step, if undoing { 1 } else { 2 });
                            self.flash(&format!("{} {what}", if undoing { "Undo:" } else { "Redo:" }));
                        }
                        None => self.flash(if undoing { "Nothing to undo" } else { "Nothing to redo" }),
                    }
                }
                Action::Restore => {
                    let sel = self.tab().selected_paths();
                    self.start_op(OpKind::Restore, sel, None);
                }
                Action::EmptyTrash => self.dialog = Dialog::ConfirmEmptyTrash,
                Action::NewFolder => {
                    if let Some(d) = self.current_dir() {
                        let p = fsops::unique_new_name(&d, "Untitled Folder");
                        let text = p.file_name().unwrap().to_string_lossy().to_string();
                        self.dialog = Dialog::NewItem { folder: true, text, focus: true, template: None };
                    }
                }
                Action::NewFile(template) => {
                    if let Some(d) = self.current_dir() {
                        let base = template.as_ref().and_then(|t| t.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled Document".into());
                        let p = fsops::unique_new_name(&d, &base);
                        let text = p.file_name().unwrap().to_string_lossy().to_string();
                        self.dialog = Dialog::NewItem { folder: false, text, focus: true, template };
                    }
                }
                Action::Properties(p) => {
                    if let Some(p) = p.or_else(|| self.current_dir()) {
                        self.open_properties(p);
                    }
                }
                Action::Terminal(d) => {
                    if let Err(e) = launch::open_terminal(&d) {
                        self.message("Could not open a terminal", &e);
                    }
                }
                Action::AddBookmark(p) => {
                    if !self.bookmarks.iter().any(|b| b.path == p) {
                        let label = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());
                        self.bookmarks.push(Bookmark { path: p, label: label.clone() });
                        let _ = places::save_bookmarks(&self.bookmarks);
                        self.flash(&format!("Bookmarked “{label}”"));
                    }
                }
                Action::RemoveBookmark(i) => {
                    if i < self.bookmarks.len() {
                        self.bookmarks.remove(i);
                        let _ = places::save_bookmarks(&self.bookmarks);
                    }
                }
                Action::RenameBookmark(i) => {
                    if let Some(b) = self.bookmarks.get(i) {
                        self.dialog = Dialog::RenameBookmark { idx: i, text: b.label.clone() };
                    }
                }
                Action::Transfer(files, dest, copy) => {
                    let files: Vec<PathBuf> = files.into_iter().filter(|f| *f != dest).collect();
                    if dest == places::trash_dir().join("files") {
                        self.start_op(OpKind::Trash, files, None);
                    } else {
                        self.start_op(if copy { OpKind::Copy } else { OpKind::Move }, files, Some(dest));
                    }
                }
                Action::SelectAll => {
                    let t = self.tab_mut();
                    t.selected = t.visible.iter().map(|&i| t.entries[i].path.clone()).collect();
                }
                Action::CopyLocation => {
                    let sel = self.tab().selected_paths();
                    let text = if sel.is_empty() {
                        self.current_dir().map(|d| d.display().to_string()).unwrap_or_default()
                    } else {
                        sel.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n")
                    };
                    ctx.copy_text(text);
                    self.flash("Location copied");
                }
                Action::Unmount(m) => {
                    let quiet = |c: &mut std::process::Command| {
                        c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
                    };
                    let ok = quiet(std::process::Command::new("udisksctl").args(["unmount", "-b", &m.device]))
                        || quiet(std::process::Command::new("umount").arg(&m.path));
                    if ok {
                        self.flash(&format!("{} can be safely removed", m.label));
                        self.mounts = places::mounts();
                        if self.tab().loc.dir().is_some_and(|d| d.starts_with(&m.path)) {
                            self.navigate(Location::Dir(places::home()));
                        }
                    } else {
                        self.message("Could not eject", &format!("{} is busy or you lack permission to unmount it.", m.label));
                    }
                }
                Action::CloseTab(i) => self.close_tab(i, ctx),
                Action::NewTab => {
                    let loc = self.tab().loc.clone();
                    self.new_tab(loc);
                }
                Action::Compress(paths) => {
                    if !paths.is_empty() {
                        let format = crate::archive::Format::from_id(&self.s.compress_format);
                        let format = if format.available() { format } else { crate::archive::Format::ALL.into_iter().find(|f| f.available()).unwrap_or(format) };
                        self.dialog = Dialog::Compress { name: crate::archive::suggest_name(&paths), paths, format, focus: true };
                    }
                }
                Action::ExtractHere(archives) => {
                    let into = archives.first().and_then(|a| a.parent()).map(|d| d.to_path_buf());
                    if let Some(into) = into {
                        self.ops.push(crate::archive::spawn_extract(archives, into));
                    }
                }
                Action::QuickView(p) => self.open_quick_view(ctx, p),
                Action::Run(p) => {
                    if let Err(e) = launch::run_executable(&p) {
                        self.message("Could not run", &e);
                    }
                }
            }
        }
    }

    /// Where in-app windows may live: the whole app minus a small margin.
    fn window_area(&self, ctx: &egui::Context) -> Rect {
        ctx.content_rect().shrink(6.0)
    }

    /// Title bar textures for an in-app window.
    fn window_chrome(&mut self, title: &str, icon: &[&str], ctx: &egui::Context) -> windows::Chrome {
        windows::Chrome {
            title: title.to_string(),
            icon: self.icon(ctx, icon, 16.0),
            close: self.icon(ctx, &["window-close-symbolic"], 14.0),
            flush: false,
        }
    }

    /// Rivet's own app icon as a texture (rendered once).
    fn app_icon_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.app_icon.is_none() {
            let px = (96.0 * ctx.pixels_per_point()) as u32;
            self.app_icon = crate::icons::render_svg(crate::desktop::ICON_SVG.as_bytes(), px)
                .map(|img| ctx.load_texture("rivet-app-icon", img, egui::TextureOptions::LINEAR));
        }
        self.app_icon.clone()
    }

    /// Run the opposite of a recorded step. `role` 1 = undo, 2 = redo; the result is
    /// recorded on the other stack when it finishes.
    fn run_inverse(&mut self, step: crate::undo::Step, role: u8) {
        use crate::undo::Step;
        match step {
            Step::Copied(paths) => self.ops.push(fsops::spawn_op_role(OpKind::Trash, paths, None, role)),
            Step::Created(p) => self.ops.push(fsops::spawn_op_role(OpKind::Trash, vec![p], None, role)),
            Step::Moved(pairs) => self.ops.push(fsops::spawn_move_pairs(pairs.into_iter().map(|(from, to)| (to, from)).collect(), role)),
            Step::Trashed(pairs) => self.ops.push(fsops::spawn_op_role(OpKind::Restore, pairs.into_iter().map(|(_, t)| t).collect(), None, role)),
            Step::Restored(pairs) => self.ops.push(fsops::spawn_op_role(OpKind::Trash, pairs.into_iter().map(|(_, r)| r).collect(), None, role)),
            Step::RenamedMany(pairs) => {
                // Back to the old names, with the same two-pass safety as the rename itself.
                let back: Vec<crate::rename::Planned> = pairs
                    .iter()
                    .map(|(old, new)| crate::rename::Planned { from: new.clone(), to: old.clone(), problem: None })
                    .collect();
                match crate::rename::apply(&back) {
                    Ok(done) => {
                        self.history.record_role(Step::RenamedMany(done.clone()), role);
                        self.reload_all();
                        let t = self.tab_mut();
                        t.selected = done.iter().map(|(_, n)| n.clone()).collect();
                    }
                    Err(e) => self.message("Can't undo the rename", &e),
                }
            }
            Step::Renamed(old, new) => {
                if old.symlink_metadata().is_ok() {
                    self.message("Can't undo the rename", &format!("“{}” already exists.", old.display()));
                    return;
                }
                match std::fs::rename(&new, &old) {
                    Ok(()) => {
                        self.history.record_role(Step::Renamed(new, old.clone()), role);
                        self.reload_all();
                        let t = self.tab_mut();
                        t.selected.clear();
                        t.selected.insert(old.clone());
                        t.scroll_to = Some(old);
                    }
                    Err(e) => self.message("Can't undo the rename", &e.to_string()),
                }
            }
        }
    }

    /// Open the delete confirmation for these items (counts and size measured up front).
    fn ask_delete(&mut self, paths: Vec<PathBuf>, permanent_default: bool) {
        let allow_trash = !matches!(self.tab().loc, Location::Trash);
        let (mut folders, mut files, mut bytes, mut partial) = (0, 0, 0u64, false);
        let deadline = Instant::now() + std::time::Duration::from_millis(400);
        for p in &paths {
            match std::fs::symlink_metadata(p) {
                Ok(m) if m.is_dir() => {
                    folders += 1;
                    let (b, _, complete) = fsops::dir_size(p, deadline);
                    bytes += b;
                    partial |= !complete;
                }
                Ok(m) => {
                    files += 1;
                    bytes += m.len();
                }
                Err(_) => {}
            }
        }
        self.dialog = Dialog::ConfirmDelete(Box::new(DeleteAsk {
            paths,
            allow_trash,
            permanent_default: permanent_default || !allow_trash,
            folders,
            files,
            bytes,
            partial,
        }));
    }

    /// Help → About: the About page of Preferences.
    fn open_about(&mut self) {
        self.prefs_tab = PrefsTab::About;
        self.open_prefs();
    }

    /// A clear accent ring around whatever has keyboard focus, shown once Tab is used and
    /// hidden again on the next mouse press.
    fn focus_ring(&mut self, ctx: &egui::Context) {
        let (tabbed, pressed) = ctx.input(|i| {
            (i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Tab, pressed: true, .. })), i.pointer.any_pressed())
        });
        if tabbed {
            self.kb_nav = true;
        }
        if pressed {
            self.kb_nav = false;
        }
        if !self.kb_nav {
            return;
        }
        let Some(id) = ctx.memory(|m| m.focused()) else { return };
        let Some(r) = ctx.read_response(id) else { return };
        let accent = accent_color(&self.s);
        // The file list's ring sits just inside its edge; small controls get one just outside.
        let (rect, radius) = if Some(id) == self.file_list_focus { (r.interact_rect.shrink(2.0), R_LG) } else { (r.interact_rect.expand(2.0), R_SM) };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("focus-ring")));
        painter.rect_stroke(rect, radius, Stroke::new(2.0, accent), StrokeKind::Outside);
    }

    fn open_prefs(&mut self) {
        if !self.prefs_open {
            self.popup_serial += 1; // a fresh window id re-centres it
        }
        self.prefs_open = true;
        // The keyboard goes to Preferences straight away (arrows move between its pages).
        self.prefs_focus = true;
        self.window_focus = true;
    }

    fn open_properties(&mut self, p: PathBuf) {
        // Already open? Just bring it back up.
        if let Some((w, _)) = self.props.iter_mut().find(|(_, s)| s.path == p) {
            w.shaded = false;
            return;
        }
        use std::os::unix::fs::MetadataExt;
        let entry = Entry::from_path(p.clone(), &self.mime);
        let meta = std::fs::metadata(&p).ok();
        let mode = entry.as_ref().map(|e| e.mode).unwrap_or(0);
        let size = if entry.as_ref().is_some_and(|e| e.is_dir) {
            Some(fsops::dir_size(&p, Instant::now() + std::time::Duration::from_millis(1500)))
        } else {
            None
        };
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());
        self.next_win_id += 1;
        let mut win = windows::WinState::new(self.next_win_id, Vec2::new(480.0, 560.0));
        // Cascade new windows a little so they don't sit exactly on top of each other.
        win.cascade = Vec2::splat(32.0 * (self.props.len() % 6) as f32);
        self.props.push((win, PropsState {
            owner: meta.as_ref().map(|m| user_name(m.uid())).unwrap_or_default(),
            group: meta.as_ref().map(|m| group_name(m.gid())).unwrap_or_default(),
            path: p,
            entry,
            name,
            mode,
            size,
            tab: 0,
        }));
    }

    // ---------------------------------------------------------------- per-frame housekeeping

    fn housekeeping(&mut self, ctx: &egui::Context) {
        self.thumbs.poll(ctx);
        let mut changed = false;
        let mut created: Vec<PathBuf> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        let history = &mut self.history;
        self.ops.retain(|op| {
            if op.finished.load(Ordering::Acquire) {
                changed = true;
                if op.kind == OpKind::EmptyTrash {
                    history.forget_trash();
                } else if let Some(step) = crate::undo::Step::from_op(op.kind, op.done.lock().unwrap().clone()) {
                    history.record_role(step, op.role);
                }
                created.extend(op.created.lock().unwrap().iter().cloned());
                errors.extend(op.errors.lock().unwrap().iter().cloned());
                false
            } else {
                true
            }
        });
        if !self.ops.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if changed {
            self.reload_all();
            let t = &mut self.tabs[self.active];
            let here: Vec<PathBuf> = created.into_iter().filter(|p| t.loc.dir().is_some_and(|d| p.parent() == Some(d))).collect();
            if !here.is_empty() {
                t.selected = here.iter().cloned().collect();
                t.scroll_to = here.first().cloned();
            }
            if !errors.is_empty() {
                self.message("Some items could not be processed", &errors.join("\n"));
            }
        }
        // search results
        {
            let t = &mut self.tabs[self.active];
            if let Some(rx) = &t.search_rx {
                let mut got = Vec::new();
                loop {
                    match rx.try_recv() {
                        Ok(batch) => got.extend(batch),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            t.searching = false;
                            t.search_rx = None;
                            break;
                        }
                    }
                }
                if !got.is_empty() {
                    for p in got {
                        if let Some(e) = Entry::from_path(p, &self.mime) {
                            t.entries.push(e);
                        }
                    }
                    t.resort(&self.mime, &self.s);
                }
            }
        }
        // Tell the watcher what to look at: every open folder and the colour files.
        let mut watched: Vec<PathBuf> = self.tabs.iter().filter_map(|t| t.watched_dir()).collect();
        watched.push(places::trash_dir().join("files"));
        watched.extend(self.color_watch.files().iter().cloned());
        self.watch.set_paths(watched);
        // A status message disappears after 4 s; wake up for that only while one is shown.
        if let Some((_, t)) = &self.status_msg {
            let shown = t.elapsed();
            if shown.as_secs() >= 4 {
                self.status_msg = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs(4) - shown + std::time::Duration::from_millis(20));
            }
        }
        // files dropped from other applications: copy into the current folder
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        let dropped: Vec<PathBuf> = dropped.into_iter().filter(|p| !p.as_os_str().is_empty()).collect();
        if !dropped.is_empty() {
            if let Some(d) = self.current_dir() {
                self.start_op(OpKind::Copy, dropped, Some(d));
            }
        }
        let title = self.tab().loc.title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if self.s.decorations != self.last_decorations {
            self.last_decorations = self.s.decorations;
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(self.s.decorations));
            self.dirty_settings = true;
        }
    }

    // ---------------------------------------------------------------- keyboard

    fn keyboard(&mut self, ctx: &egui::Context, cols: usize) {
        if self.screenshot.is_some() {
            // A check must not be steered by whatever the person is typing elsewhere.
            return;
        }
        // A focused widget takes the keyboard — except the file list's own stop.
        let typing = ctx.egui_wants_keyboard_input() && ctx.memory(|m| m.focused()) != self.file_list_focus;
        // Dialogs block shortcuts; in-app windows only while you're working in one.
        let modal = !matches!(self.dialog, Dialog::None) || self.window_focus || self.connect_ask.is_some() || self.quick.is_some();
        let (events, m) = ctx.input(|i| (i.events.clone(), i.modifiers));
        let pressed = |k: Key| events.iter().any(|e| matches!(e, egui::Event::Key { key, pressed: true, .. } if *key == k));
        let cmd = m.command;

        if pressed(Key::Escape) {
            if self.path_editing {
                self.path_editing = false;
            } else if self.search_open {
                self.search_open = false;
                self.tab_mut().filter.clear();
                self.tabs[self.active].resort(&self.mime, &self.s);
            } else if !modal {
                self.tab_mut().selected.clear();
            }
        }
        if cmd && pressed(Key::L) {
            self.path_editing = true;
            self.path_focus = true;
            self.path_text = self.current_dir().map(|d| d.display().to_string()).unwrap_or_default();
        }
        if cmd && pressed(Key::F) {
            self.search_open = true;
            self.search_focus = true;
        }
        // Ctrl+, opens Preferences (or closes it when it is already open).
        if cmd && pressed(Key::Comma) {
            if self.prefs_open {
                self.prefs_open = false;
            } else {
                self.open_prefs();
            }
        }
        if cmd && pressed(Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if cmd && pressed(Key::T) {
            self.pending.push(Action::NewTab);
        }
        if cmd && pressed(Key::W) {
            self.pending.push(Action::CloseTab(self.active));
        }
        if m.ctrl && pressed(Key::Tab) && self.tabs.len() > 1 {
            let n = self.tabs.len();
            self.active = if m.shift { (self.active + n - 1) % n } else { (self.active + 1) % n };
        }
        if pressed(Key::F1) {
            self.dialog = Dialog::Shortcuts;
        }
        if pressed(Key::F8) {
            self.s.show_statusbar = !self.s.show_statusbar;
            self.dirty_settings = true;
        }
        if pressed(Key::F9) {
            self.s.show_sidebar = !self.s.show_sidebar;
            self.dirty_settings = true;
        }
        if pressed(Key::F11) {
            self.s.show_peek = !self.s.show_peek;
            self.dirty_settings = true;
        }
        if (cmd && pressed(Key::M)) || pressed(Key::F10) {
            self.s.show_menubar = !self.s.show_menubar;
            self.dirty_settings = true;
            if !self.s.show_menubar {
                self.flash("Menu bar hidden — press Ctrl+M (or F10) to bring it back");
            }
        }
        // Mouse side buttons (back / forward) work like the toolbar arrows (even while a text box has focus).
        let (mouse_back, mouse_fwd) = ctx.input(|i| (i.pointer.button_pressed(egui::PointerButton::Extra1), i.pointer.button_pressed(egui::PointerButton::Extra2)));
        if mouse_back {
            self.go_back();
        }
        if mouse_fwd {
            self.go_forward();
        }
        if modal {
            return;
        }
        if typing {
            return;
        }

        for e in &events {
            match e {
                egui::Event::Copy => self.pending.push(Action::Copy),
                egui::Event::Cut => self.pending.push(Action::Cut),
                egui::Event::Paste(text) => {
                    if self.clipboard.is_some() {
                        self.pending.push(Action::Paste(None));
                    } else if let Some(d) = self.current_dir() {
                        // Paths copied from other applications.
                        let files: Vec<PathBuf> = text
                            .lines()
                            .map(|l| l.trim())
                            .map(|l| l.strip_prefix("file://").map(places::percent_decode).unwrap_or_else(|| l.to_string()))
                            .map(PathBuf::from)
                            .filter(|p| p.is_absolute() && p.exists())
                            .collect();
                        if !files.is_empty() {
                            self.start_op(OpKind::Copy, files, Some(d));
                        }
                    }
                }
                egui::Event::Text(t) if !cmd && !m.alt && !t.trim().is_empty() => {
                    // Type-ahead: start filtering the folder.
                    self.search_open = true;
                    self.search_focus = true;
                    self.tab_mut().filter.push_str(t);
                    self.tabs[self.active].resort(&self.mime, &self.s);
                }
                _ => {}
            }
        }

        // Space: Preview the selected file (pictures, videos, music, text).
        if pressed(Key::Space) && !cmd && !m.alt {
            let sel = self.tab().selected_entries();
            if let Some(e) = sel.first().filter(|e| sel.len() == 1 && Self::quick_viewable(e)) {
                self.pending.push(Action::QuickView(e.path.clone()));
            }
        }
        if (m.alt && pressed(Key::ArrowLeft)) || pressed(Key::Backspace) || pressed(Key::BrowserBack) {
            self.go_back();
        }
        if (m.alt && pressed(Key::ArrowRight)) || pressed(Key::BrowserForward) {
            self.go_forward();
        }
        if m.alt && pressed(Key::ArrowUp) {
            self.go_up();
        }
        if m.alt && pressed(Key::Home) {
            self.navigate(Location::Dir(places::home()));
        }
        if pressed(Key::F5) || (cmd && pressed(Key::R)) {
            self.reload_all();
        }
        if cmd && pressed(Key::H) {
            self.s.show_hidden = !self.s.show_hidden;
            self.resort_all();
        }
        if cmd && (pressed(Key::Y) || (m.shift && pressed(Key::Z))) {
            self.pending.push(Action::Redo);
        } else if cmd && pressed(Key::Z) {
            self.pending.push(Action::Undo);
        }
        if cmd && pressed(Key::A) {
            self.pending.push(Action::SelectAll);
        }
        if cmd && m.shift && pressed(Key::N) {
            self.pending.push(Action::NewFolder);
        }
        if cmd && pressed(Key::D) {
            if let Some(d) = self.current_dir() {
                self.pending.push(Action::AddBookmark(d));
            }
        }
        if cmd && (pressed(Key::Plus) || pressed(Key::Equals)) {
            self.zoom(1);
        }
        if cmd && pressed(Key::Minus) {
            self.zoom(-1);
        }
        for (k, v) in [(Key::Num1, ViewMode::Icons), (Key::Num2, ViewMode::List), (Key::Num3, ViewMode::Compact)] {
            if cmd && pressed(k) {
                self.s.view = v;
                self.dirty_settings = true;
            }
        }
        let sel = self.tab().selected_paths();
        if pressed(Key::F2) && sel.len() == 1 {
            self.pending.push(Action::Rename(sel[0].clone()));
        } else if pressed(Key::F2) && sel.len() > 1 && self.s.bulk_rename && !matches!(self.tab().loc, Location::Trash) {
            self.pending.push(Action::BulkRename(sel.clone()));
        }
        if pressed(Key::Delete) {
            self.pending.push(if m.shift { Action::Delete } else { Action::AskDelete });
        }
        if (cmd && pressed(Key::I)) || (m.alt && pressed(Key::Enter)) {
            self.pending.push(Action::Properties(sel.first().cloned()));
        } else if pressed(Key::Enter) {
            for p in sel.iter().take(10) {
                self.pending.push(Action::Open(p.clone()));
            }
        }

        // Arrow keys move the selection.
        let n = self.tab().visible.len();
        if n == 0 || m.alt {
            return;
        }
        let (step_lr, step_ud) = match self.s.view {
            ViewMode::Icons => (1isize, cols.max(1) as isize),
            ViewMode::List => (0, 1),
            ViewMode::Compact => (cols.max(1) as isize, 1),
        };
        let mut delta: isize = 0;
        if pressed(Key::ArrowRight) {
            delta = step_lr;
        }
        if pressed(Key::ArrowLeft) {
            delta = -step_lr;
        }
        if pressed(Key::ArrowDown) {
            delta = step_ud;
        }
        if pressed(Key::ArrowUp) {
            delta = -step_ud;
        }
        let cur = self.tab().cursor.filter(|&c| c < n);
        let mut target: Option<usize> = None;
        if delta != 0 {
            target = Some(match cur {
                Some(c) => (c as isize + delta).clamp(0, n as isize - 1) as usize,
                None => 0,
            });
        }
        if pressed(Key::Home) {
            target = Some(0);
        }
        if pressed(Key::End) {
            target = Some(n - 1);
        }
        if let Some(ti) = target {
            let t = self.tab_mut();
            let p = t.entries[t.visible[ti]].path.clone();
            if m.shift {
                let a = t.anchor.unwrap_or(ti);
                let (lo, hi) = if a < ti { (a, ti) } else { (ti, a) };
                t.selected = (lo..=hi).map(|i| t.entries[t.visible[i]].path.clone()).collect();
            } else {
                t.selected.clear();
                t.selected.insert(p.clone());
                t.anchor = Some(ti);
            }
            t.cursor = Some(ti);
            t.scroll_to = Some(p);
        }
    }

    // ---------------------------------------------------------------- item interaction

    fn click_item(&mut self, vis_idx: usize, ctx: &egui::Context, resp: &egui::Response) {
        let m = ctx.input(|i| i.modifiers);
        let open_now = if self.s.single_click { resp.clicked() && !m.command && !m.shift } else { resp.double_clicked() };
        let t = &mut self.tabs[self.active];
        let path = t.entries[t.visible[vis_idx]].path.clone();
        if resp.clicked() || (resp.secondary_clicked() && !t.selected.contains(&path)) {
            if m.command && resp.clicked() {
                if !t.selected.remove(&path) {
                    t.selected.insert(path.clone());
                }
                t.anchor = Some(vis_idx);
            } else if m.shift && resp.clicked() {
                let a = t.anchor.unwrap_or(vis_idx);
                let (lo, hi) = if a < vis_idx { (a, vis_idx) } else { (vis_idx, a) };
                t.selected = (lo..=hi).map(|i| t.entries[t.visible[i]].path.clone()).collect();
            } else {
                t.selected.clear();
                t.selected.insert(path.clone());
                t.anchor = Some(vis_idx);
            }
            t.cursor = Some(vis_idx);
        }
        if resp.clicked_by(egui::PointerButton::Middle) {
            self.pending.push(Action::OpenInNewTab(path));
        } else if open_now {
            self.pending.push(Action::Open(path));
        }
    }

    /// Drag source + drop target handling for an item.
    fn dnd_item(&mut self, ctx: &egui::Context, vis_idx: usize, resp: &egui::Response) {
        let t = &self.tabs[self.active];
        let e = &t.entries[t.visible[vis_idx]];
        if resp.drag_started() && !matches!(t.loc, Location::Trash) {
            let payload = if t.selected.contains(&e.path) { t.selected_paths() } else { vec![e.path.clone()] };
            resp.dnd_set_drag_payload(DragFiles(payload));
        }
        if e.is_dir {
            if let Some(p) = resp.dnd_release_payload::<DragFiles>() {
                if !p.0.contains(&e.path) {
                    let copy = ctx.input(|i| i.modifiers.ctrl);
                    self.pending.push(Action::Transfer(p.0.clone(), e.path.clone(), copy));
                }
            }
        }
    }

    fn drop_highlight(&self, ui: &egui::Ui, rect: Rect, resp: &egui::Response, is_dir: bool) {
        if is_dir && resp.dnd_hover_payload::<DragFiles>().is_some() {
            ui.painter().rect_stroke(rect, R_SM, Stroke::new(2.0, accent_color(&self.s)), StrokeKind::Inside);
        }
    }

    // ---------------------------------------------------------------- screenshot mode

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        // Checks never touch the real clipboard: report what would be copied instead.
        ctx.output_mut(|o| {
            o.commands.retain(|c| match c {
                egui::OutputCommand::CopyText(t) => {
                    println!("copied: {t}");
                    false
                }
                _ => true,
            })
        });
        self.frames += 1;
        ctx.request_repaint();
        // Keep painting even if the compositor reports the window as hidden.
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        if self.shot_done.load(Ordering::Acquire) {
            if std::env::var_os("RIVET_FRAME_STATS").is_some() && self.frame_times.len() > 10 && !self.stats_printed {
                self.stats_printed = true;
                if let Some(f) = self.first_frame_ms {
                    println!("first frame ready {f:.0} ms after start");
                }
                // Skip the first frames (fonts, icons and the folder are loaded then).
                let mut t: Vec<f64> = self.frame_times[10..].to_vec();
                t.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let first: f64 = self.frame_times[..10].iter().sum();
                println!("first 10 frames: {first:.1} ms total; afterwards median {:.2} ms, 95th percentile {:.2} ms per frame", t[t.len() / 2], t[t.len() * 95 / 100]);
            }
            if let Some(d) = self.demo_dir.take() {
                places::remove_demo_dir(&d);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            let done = self.shot_done.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img: Arc<egui::ColorImage>| {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&p.to_srgba_unmultiplied());
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = image::save_buffer(&path, &buf, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                    eprintln!("could not save screenshot: {e}");
                }
                done.store(true, Ordering::Release);
                ctx2.request_repaint();
            });
        }
        if self.frames > 900 {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for RivetApp {
    /// Runs every frame and also while the window is hidden, so a new matugen/GTK
    /// scheme is picked up even when the file manager is in the background.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let (Some(secs), Some(t)) = (crate::MEASURE_SECS.get(), crate::STARTED.get()) {
            let left = secs - t.elapsed().as_secs_f64();
            if left <= 0.0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
            }
        }
        // Only when the background watcher saw a change (no polling here).
        if self.watch.take() {
            if self.color_watch.changed() {
                self.reload_desktop_theme();
                self.apply_visuals(ctx);
                self.flash("Desktop theme changed — colors updated");
            }
            for i in 0..self.tabs.len() {
                if self.tabs[i].needs_refresh() {
                    let t = &mut self.tabs[i];
                    t.reload(&self.mime, &self.s);
                    for e in &t.entries {
                        self.thumbs.forget(&e.path);
                    }
                }
            }
            if self.screenshot.is_none() {
                self.mounts = places::mounts();
            }
            self.trash_empty = places::trash_is_empty();
            self.places_checked = Instant::now();
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        let ctx = ui.ctx().clone();
        self.apply_font(&ctx);
        self.apply_visuals(&ctx);
        self.sync_builtin_icons(&ctx);
        self.housekeeping(&ctx);

        if !self.s.decorations {
            self.window_resize_edges(&ctx);
        }
        self.header(ui, &ctx);
        // Toolbar and status bar can each sit at the top or the bottom. Panels added first sit
        // nearer the window edge: at the top the toolbar comes first, at the bottom the status
        // bar is outermost with the toolbar stacked just above it.
        let status = self.s.show_statusbar;
        let both_bottom = status && self.s.toolbar_bottom && !self.s.statusbar_top;
        if both_bottom {
            self.status_bar(ui, &ctx);
            self.flat_buttons = false;
            self.toolbar(ui, &ctx);
        } else {
            self.toolbar(ui, &ctx);
            if status {
                self.status_bar(ui, &ctx);
                self.flat_buttons = false;
            }
        }
        if self.s.show_sidebar {
            self.sidebar(ui, &ctx);
        }
        if self.s.show_peek {
            self.peek_panel(ui, &ctx);
        } else {
            self.peek.stop();
        }
        // Added after the sidebar, so the tab strip starts where the sidebar ends.
        if self.tabs.len() > 1 {
            self.tab_bar(ui, &ctx);
        }
        let cols = self.file_area(ui, &ctx);
        self.dialogs(&ctx);
        self.window_rects.clear();
        if self.prefs_open {
            self.prefs_window(&ctx);
        }
        self.properties_windows(&ctx);
        self.net_poll();
        self.connect_dialog(&ctx);
        // Preview draws over everything else and takes the arrow keys while open.
        self.quick_view(&ctx);
        // The last click decides whether the keyboard belongs to a window or the file view.
        if let Some(p) = ctx.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None }) {
            self.window_focus = self.window_rects.iter().any(|r| r.contains(p));
            self.prefs_focus = self.prefs_open && self.prefs_rect.is_some_and(|r| r.contains(p));
        }
        if !self.prefs_open {
            self.prefs_focus = false;
            self.prefs_rect = None;
        }
        if self.window_rects.is_empty() {
            self.window_focus = false;
        }
        self.keyboard(&ctx, cols);
        self.focus_ring(&ctx);
        self.drag_overlay(&ctx);
        self.process_actions(&ctx);

        if self.dirty_settings && self.screenshot.is_none() && !crate::measuring() {
            if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
                self.s.window_w = r.width();
                self.s.window_h = r.height();
            }
            self.s.save();
            self.dirty_settings = false;
        }
        if self.screenshot.is_some() {
            self.frame_times.push(frame_start.elapsed().as_secs_f64() * 1000.0);
            if self.first_frame_ms.is_none() {
                self.first_frame_ms = crate::STARTED.get().map(|t| t.elapsed().as_secs_f64() * 1000.0);
            }
        }
        self.screenshot_step(&ctx);
    }

    /// Test hook for screenshots: RIVET_CLICKS="x,y@frame;x,y@frame" clicks there.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.screenshot.is_none() {
            return;
        }
        let Ok(list) = std::env::var("RIVET_CLICKS") else { return };
        for item in list.split(';') {
            let Some((xy, f)) = item.split_once('@') else { continue };
            // "hX,Y@N" only moves the pointer there (hover) from frame N on.
            if let Some(rest) = xy.trim().strip_prefix('h') {
                if let (Some((x, y)), Ok(f)) = (rest.split_once(','), f.trim().parse::<u32>()) {
                    if let (Ok(x), Ok(y)) = (x.parse::<f32>(), y.parse::<f32>()) {
                        if self.frames + 1 >= f {
                            raw.events.push(egui::Event::PointerMoved(Pos2::new(x, y)));
                        }
                    }
                }
                continue;
            }
            // "left@N" / "right@N" / "up@N" / "down@N" / "enter@N" / "tab@N" press that key at frame N.
            if matches!(xy.trim(), "left" | "right" | "up" | "down" | "enter" | "tab" | "esc") {
                if let Ok(f) = f.trim().parse::<u32>() {
                    if self.frames + 1 == f {
                        let key = match xy.trim() {
                            "left" => egui::Key::ArrowLeft,
                            "right" => egui::Key::ArrowRight,
                            "tab" => egui::Key::Tab,
                            "up" => egui::Key::ArrowUp,
                            "down" => egui::Key::ArrowDown,
                            "esc" => egui::Key::Escape,
                            _ => egui::Key::Enter,
                        };
                        raw.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() });
                    }
                }
                continue;
            }
            // "type:TEXT@N" types TEXT at frame N.
            if let Some(text) = xy.trim().strip_prefix("type:") {
                if let Ok(f) = f.trim().parse::<u32>() {
                    if self.frames + 1 == f {
                        raw.events.push(egui::Event::Text(text.to_string()));
                    }
                }
                continue;
            }
            // "copy@N" sends Ctrl+C at frame N.
            if xy.trim() == "copy" {
                if let Ok(f) = f.trim().parse::<u32>() {
                    if self.frames + 1 == f {
                        let m = egui::Modifiers::COMMAND;
                        raw.events.push(egui::Event::Key { key: egui::Key::C, physical_key: None, pressed: true, repeat: false, modifiers: m });
                        raw.events.push(egui::Event::Copy);
                    }
                }
                continue;
            }
            // "r" in front means a right click: "r400,300@5".
            let (button, xy) = match xy.trim().strip_prefix('r') {
                Some(rest) => (egui::PointerButton::Secondary, rest),
                None => (egui::PointerButton::Primary, xy),
            };
            let Some((x, y)) = xy.split_once(',') else { continue };
            let (Ok(x), Ok(y), Ok(f)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>(), f.trim().parse::<u32>()) else { continue };
            let pos = Pos2::new(x, y);
            if self.frames + 1 == f {
                raw.events.push(egui::Event::PointerMoved(pos));
                raw.events.push(egui::Event::PointerButton { pos, button, pressed: true, modifiers: Default::default() });
            } else if self.frames + 1 == f + 1 {
                raw.events.push(egui::Event::PointerButton { pos, button, pressed: false, modifiers: Default::default() });
            } else if self.frames + 1 > f + 1 {
                raw.events.push(egui::Event::PointerMoved(pos));
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.screenshot.is_none() && !crate::measuring() {
            self.s.save();
        }
        if let Some(d) = self.demo_dir.take() {
            places::remove_demo_dir(&d);
        }
    }
}
