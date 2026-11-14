//! Quick Actions (Ctrl+Shift+P): every menu command and view option in one searchable list.

use eframe::egui::{self, Align2, Color32, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::chrome::fuzzy_score;
use super::{Action, Dialog, RivetApp, r_lg, r_sm};
use crate::model::Location;
use crate::places;
use crate::settings::{SortKey, ThemeChoice, ViewMode};

#[derive(Clone, Copy, PartialEq)]
enum Cmd {
    NewFolder,
    NewDocument,
    NewTab,
    CloseTab,
    Terminal,
    Properties,
    CloseWindow,
    Undo,
    Redo,
    SelectAll,
    CopyLocation,
    Rename,
    Trash,
    EmptyTrash,
    Preferences,
    Reload,
    Icons,
    List,
    Compact,
    Hidden,
    Thumbnails,
    Sidebar,
    FolderTree,
    Peek,
    Statusbar,
    Menubar,
    Zen,
    TerminalMode,
    Filters,
    ZoomIn,
    ZoomOut,
    Sort(SortKey),
    Reversed,
    FoldersFirst,
    SidebarRight,
    PeekLeft,
    ToolbarBottom,
    StatusTop,
    AppTitleBar,
    Theme(ThemeChoice),
    Back,
    Forward,
    Parent,
    Home,
    FileSystem,
    Recent,
    TrashFolder,
    Network,
    Connect,
    Location,
    Search,
    QuickOpen,
    Bookmark,
    Shortcuts,
    About,
}

/// One row: where it lives, its name, its keys, and (for on / off options) its state.
#[derive(Clone)]
struct Row {
    cmd: Cmd,
    group: &'static str,
    label: String,
    keys: &'static str,
    state: Option<bool>,
}

pub(super) struct QuickActions {
    query: String,
    sel: usize,
    focus: bool,
}

impl RivetApp {
    pub(super) fn toggle_actions(&mut self) {
        self.spotlight = None;
        self.actions = if self.actions.is_some() { None } else { Some(QuickActions { query: String::new(), sel: 0, focus: true }) };
    }

    /// What can be done right now (commands that don't apply here are left out).
    fn action_rows(&self) -> Vec<Row> {
        let s = &self.s;
        let in_dir = self.current_dir().is_some();
        let sel = self.tab().selected_paths();
        let in_trash = matches!(self.tab().loc, Location::Trash);
        let mut v: Vec<Row> = Vec::new();
        let mut add = |cmd: Cmd, group: &'static str, label: &str, keys: &'static str, state: Option<bool>, when: bool| {
            if when {
                v.push(Row { cmd, group, label: label.into(), keys, state });
            }
        };
        // View options first: they're what this list is mostly for.
        // Options for a hidden panel are left out, and pick-one choices (view, sort key, theme) list only the ones you can switch to.
        add(Cmd::Icons, "View", "Icon View", "Ctrl+1", None, s.view != ViewMode::Icons);
        add(Cmd::List, "View", "List View", "Ctrl+2", None, s.view != ViewMode::List);
        add(Cmd::Compact, "View", "Compact View", "Ctrl+3", None, s.view != ViewMode::Compact);
        add(Cmd::Hidden, "View", "Show Hidden Files", "Ctrl+H", Some(s.show_hidden), true);
        add(Cmd::Thumbnails, "View", "Picture Thumbnails", "", Some(s.show_thumbnails), true);
        add(Cmd::Sidebar, "View", "Sidebar", "F9", Some(s.show_sidebar), true);
        add(Cmd::FolderTree, "View", "Folder Tree in the Sidebar", "", Some(s.sidebar_tree), s.show_sidebar);
        add(Cmd::Peek, "View", "Peek", "F11", Some(s.show_peek), true);
        add(Cmd::Statusbar, "View", "Statusbar", "F8", Some(s.show_statusbar), true);
        add(Cmd::Menubar, "View", "Menubar", "Ctrl+M", Some(s.show_menubar), true);
        add(Cmd::Zen, "View", "Zen Mode", "F12", Some(s.zen), true);
        add(Cmd::TerminalMode, "View", "Terminal Mode", "F7", Some(s.terminal), true);
        add(Cmd::Filters, "View", "Filters", "Ctrl+Shift+F", None, true);
        add(Cmd::Reload, "View", "Reload", "F5", None, true);
        add(Cmd::ZoomIn, "View", "Zoom In", "Ctrl++", None, true);
        add(Cmd::ZoomOut, "View", "Zoom Out", "Ctrl+-", None, true);
        for (k, l) in [(SortKey::Name, "Sort by Name"), (SortKey::Size, "Sort by Size"), (SortKey::Type, "Sort by Type"), (SortKey::Modified, "Sort by Modification Date")] {
            add(Cmd::Sort(k), "Sort", l, "", None, s.sort != k);
        }
        add(Cmd::Reversed, "Sort", "Reversed Order", "", Some(s.sort_desc), true);
        add(Cmd::FoldersFirst, "Sort", "Folders First", "", Some(s.folders_first), true);
        add(Cmd::SidebarRight, "Layout", "Sidebar on the Right", "", Some(s.sidebar_right), s.show_sidebar);
        add(Cmd::PeekLeft, "Layout", "Peek on the Left", "", Some(s.peek_left), s.show_peek);
        add(Cmd::ToolbarBottom, "Layout", "Toolbar at the Bottom", "", Some(s.toolbar_bottom), true);
        add(Cmd::StatusTop, "Layout", "Statusbar at the Top", "", Some(s.statusbar_top), s.show_statusbar);
        add(Cmd::AppTitleBar, "Layout", "App Title Bar", "", Some(!s.decorations), true);
        add(Cmd::Theme(ThemeChoice::System), "Theme", "Follow System", "", None, s.theme != ThemeChoice::System);
        add(Cmd::Theme(ThemeChoice::Light), "Theme", "Light", "", None, s.theme != ThemeChoice::Light);
        add(Cmd::Theme(ThemeChoice::Dark), "Theme", "Dark", "", None, s.theme != ThemeChoice::Dark);
        add(Cmd::Preferences, "Rivet", "Preferences", "Ctrl+,", None, true);
        add(Cmd::QuickOpen, "Go", "Quick Open", "Ctrl+P", None, true);
        add(Cmd::Back, "Go", "Back", "Alt+Left", None, !self.tab().back.is_empty());
        add(Cmd::Forward, "Go", "Forward", "Alt+Right", None, !self.tab().forward.is_empty());
        add(Cmd::Parent, "Go", "Open Parent", "Alt+Up", None, true);
        add(Cmd::Home, "Go", "Home", "Alt+Home", None, true);
        add(Cmd::FileSystem, "Go", "File System", "", None, true);
        add(Cmd::Recent, "Go", "Recent", "", None, true);
        add(Cmd::TrashFolder, "Go", "Trash", "", None, true);
        add(Cmd::Network, "Go", "Network", "", None, true);
        add(Cmd::Connect, "Go", "Connect to Server", "", None, true);
        add(Cmd::Location, "Go", "Type a Location", "Ctrl+L", None, true);
        add(Cmd::Search, "Go", "Search for Files", "Ctrl+F", None, true);
        add(Cmd::NewFolder, "File", "Create New Folder", "Shift+Ctrl+N", None, in_dir);
        add(Cmd::NewDocument, "File", "Create New Document", "", None, in_dir);
        add(Cmd::NewTab, "File", "New Tab", "Ctrl+T", None, true);
        add(Cmd::CloseTab, "File", "Close Tab", "Ctrl+W", None, self.tabs.len() > 1);
        add(Cmd::Terminal, "File", "Open in Terminal", "", None, in_dir);
        add(Cmd::Properties, "File", "Properties", "Ctrl+I", None, true);
        add(Cmd::Bookmark, "File", "Add to Bookmarks", "Ctrl+D", None, in_dir);
        add(Cmd::Undo, "Edit", "Undo", "Ctrl+Z", None, self.history.next_undo().is_some());
        add(Cmd::Redo, "Edit", "Redo", "Shift+Ctrl+Z", None, self.history.next_redo().is_some());
        add(Cmd::SelectAll, "Edit", "Select All", "Ctrl+A", None, true);
        add(Cmd::CopyLocation, "Edit", "Copy Location", "", None, true);
        let bulk = sel.len() > 1 && s.bulk_rename;
        add(Cmd::Rename, "Edit", if bulk { "Bulk Rename" } else { "Rename" }, "F2", None, !in_trash && (sel.len() == 1 || bulk));
        add(Cmd::Trash, "Edit", "Move to Trash", "Delete", None, !sel.is_empty() && !in_trash);
        add(Cmd::EmptyTrash, "Edit", "Empty Trash", "", None, !self.trash_empty);
        add(Cmd::Shortcuts, "Help", "Keyboard Shortcuts", "F1", None, true);
        add(Cmd::About, "Help", "About Rivet", "", None, true);
        add(Cmd::CloseWindow, "File", "Close Window", "Ctrl+Q", None, true);
        v
    }

    fn run_action(&mut self, ctx: &egui::Context, cmd: Cmd) {
        let sel = self.tab().selected_paths();
        let dir = self.current_dir();
        let mut dirty = true;
        match cmd {
            Cmd::NewFolder => self.pending.push(Action::NewFolder),
            Cmd::NewDocument => self.pending.push(Action::NewFile(None)),
            Cmd::NewTab => self.pending.push(Action::NewTab),
            Cmd::CloseTab => self.pending.push(Action::CloseTab(self.active)),
            Cmd::Terminal => {
                if let Some(d) = dir {
                    self.pending.push(Action::Terminal(d));
                }
            }
            Cmd::Properties => self.pending.push(Action::Properties(sel.first().cloned())),
            Cmd::CloseWindow => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Cmd::Undo => self.pending.push(Action::Undo),
            Cmd::Redo => self.pending.push(Action::Redo),
            Cmd::SelectAll => self.pending.push(Action::SelectAll),
            Cmd::CopyLocation => self.pending.push(Action::CopyLocation),
            Cmd::Rename => {
                if sel.len() > 1 {
                    self.pending.push(Action::BulkRename(sel));
                } else if let Some(p) = sel.into_iter().next() {
                    self.pending.push(Action::Rename(p));
                }
            }
            Cmd::Trash => self.pending.push(Action::Trash),
            Cmd::EmptyTrash => self.pending.push(Action::EmptyTrash),
            Cmd::Preferences => self.open_prefs(),
            Cmd::Reload => self.reload_all(),
            Cmd::Icons => self.s.view = ViewMode::Icons,
            Cmd::List => self.s.view = ViewMode::List,
            Cmd::Compact => self.s.view = ViewMode::Compact,
            Cmd::Hidden => {
                self.s.show_hidden = !self.s.show_hidden;
                self.resort_all();
            }
            Cmd::Thumbnails => self.s.show_thumbnails = !self.s.show_thumbnails,
            Cmd::Sidebar => self.s.show_sidebar = !self.s.show_sidebar,
            Cmd::FolderTree => self.s.sidebar_tree = !self.s.sidebar_tree,
            Cmd::Peek => self.s.show_peek = !self.s.show_peek,
            Cmd::Statusbar => self.s.show_statusbar = !self.s.show_statusbar,
            Cmd::Menubar => self.s.show_menubar = !self.s.show_menubar,
            Cmd::Zen => {
                self.toggle_zen();
                dirty = false;
            }
            Cmd::TerminalMode => self.s.terminal = !self.s.terminal,
            Cmd::Filters => self.toggle_filters(),
            Cmd::ZoomIn => self.zoom(1),
            Cmd::ZoomOut => self.zoom(-1),
            Cmd::Sort(k) => {
                self.s.sort = k;
                self.resort_all();
            }
            Cmd::Reversed => {
                self.s.sort_desc = !self.s.sort_desc;
                self.resort_all();
            }
            Cmd::FoldersFirst => {
                self.s.folders_first = !self.s.folders_first;
                self.resort_all();
            }
            Cmd::SidebarRight => self.s.sidebar_right = !self.s.sidebar_right,
            Cmd::PeekLeft => self.s.peek_left = !self.s.peek_left,
            Cmd::ToolbarBottom => self.s.toolbar_bottom = !self.s.toolbar_bottom,
            Cmd::StatusTop => self.s.statusbar_top = !self.s.statusbar_top,
            Cmd::AppTitleBar => self.s.decorations = !self.s.decorations,
            Cmd::Theme(t) => self.s.theme = t,
            Cmd::Back => self.go_back(),
            Cmd::Forward => self.go_forward(),
            Cmd::Parent => self.go_up(),
            Cmd::Home => self.pending.push(Action::Navigate(Location::Dir(places::home()))),
            Cmd::FileSystem => self.pending.push(Action::Navigate(Location::Dir("/".into()))),
            Cmd::Recent => self.pending.push(Action::Navigate(Location::Recent)),
            Cmd::TrashFolder => self.pending.push(Action::Navigate(Location::Trash)),
            Cmd::Network => self.pending.push(Action::Navigate(Location::Network)),
            Cmd::Connect => self.open_connect_dialog(),
            Cmd::Location => self.start_path_editing(),
            Cmd::Search => {
                self.search_open = true;
                self.search_focus = true;
            }
            Cmd::QuickOpen => self.toggle_spotlight(),
            Cmd::Bookmark => {
                if let Some(d) = dir {
                    self.pending.push(Action::AddBookmark(d));
                }
            }
            Cmd::Shortcuts => self.dialog = Dialog::Shortcuts,
            Cmd::About => self.open_about(),
        }
        if dirty {
            self.dirty_settings = true;
        }
    }

    pub(super) fn quick_actions(&mut self, ctx: &egui::Context) {
        if self.actions.is_none() {
            // Test hook: RIVET_ACTIONS=1 (or a query) opens it on the first frame.
            if self.screenshot.is_some() && self.frames == 1
                && let Ok(q) = std::env::var("RIVET_ACTIONS") {
                    self.toggle_actions();
                    if let Some(a) = &mut self.actions {
                        a.query = if q == "1" { String::new() } else { q };
                    }
                }
            if self.actions.is_none() {
                return;
            }
        }
        let (up, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        let mut qa = self.actions.take().unwrap();
        let q = qa.query.trim().to_string();
        let rows: Vec<Row> = if q.is_empty() {
            self.action_rows()
        } else {
            let mut scored: Vec<(i32, Row)> = self
                .action_rows()
                .into_iter()
                .filter_map(|r| {
                    let by_label = fuzzy_score(&r.label, &q);
                    // "view list", "theme dark": the group's name counts too.
                    let by_both = fuzzy_score(&format!("{} {}", r.group, r.label), &q).map(|v| v - 200);
                    by_label.max(by_both).map(|v| (v, r))
                })
                .collect();
            scored.sort_by_key(|s| std::cmp::Reverse(s.0));
            scored.into_iter().map(|(_, r)| r).collect()
        };
        let n = rows.len();
        if qa.sel >= n {
            qa.sel = 0;
        }
        if n > 0 {
            if down {
                qa.sel = (qa.sel + 1) % n;
            }
            if up {
                qa.sel = (qa.sel + n - 1) % n;
            }
        }
        let mut run: Option<Cmd> = enter.then(|| rows.get(qa.sel).map(|r| r.cmd)).flatten();
        let mut close = esc;

        let screen = ctx.content_rect();
        let w = 600f32.min(screen.width() - 40.0);
        let top = (screen.height() * 0.16).max(30.0);
        let id = Id::unique("quick-actions");
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(8)).corner_radius(r_lg());
        let area = egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, Vec2::new(0.0, top));
        let backdrop = Color32::from_black_alpha(if ctx.global_style().visuals.dark_mode { 90 } else { 50 });
        let before = qa.query.clone();
        let r = egui::Modal::new(id).area(area).frame(frame).backdrop_color(backdrop).show(ctx, |ui| {
            ui.set_width(w);
            let v = ui.visuals().clone();
            self.palette_field(ui, ctx, &mut qa.query, &mut qa.focus, "Type a command or option…");
            if !rows.is_empty() {
                ui.add_space(4.0);
                let (sep, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().hline(sep.x_range(), sep.center().y, Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color));
                ui.add_space(4.0);
            }
            let row_h = 34.0;
            egui::ScrollArea::vertical().max_height(row_h * 11.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for (i, row) in rows.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                        qa.sel = i;
                    }
                    if i == qa.sel {
                        let (fill, stroke) = super::selected_tile(&v, v.window_fill);
                        ui.painter().rect_filled(rect, r_sm(), fill);
                        ui.painter().rect_stroke(rect, r_sm(), stroke, StrokeKind::Inside);
                        if up || down {
                            ui.scroll_to_rect(rect, None);
                        }
                    }
                    let cy = rect.center().y;
                    // On / off options show a check box like the menus do.
                    if let Some(on) = row.state {
                        let b = Rect::from_center_size(Pos2::new(rect.left() + 18.0, cy), Vec2::splat(14.0));
                        if on {
                            ui.painter().rect_filled(b, 3.0, v.selection.bg_fill);
                            let c = b.center();
                            let s = Stroke::new(1.6, v.selection.stroke.color);
                            ui.painter().line_segment([c + Vec2::new(-3.5, 0.0), c + Vec2::new(-1.0, 2.8)], s);
                            ui.painter().line_segment([c + Vec2::new(-1.0, 2.8), c + Vec2::new(3.8, -2.8)], s);
                        } else {
                            ui.painter().rect_stroke(b, 3.0, Stroke::new(1.2, v.weak_text_color()), StrokeKind::Inside);
                        }
                    }
                    let x = rect.left() + 36.0;
                    let g = ui.painter().layout_no_wrap(format!("{}  ", row.group), crate::fonts::ui_font(12.5), v.weak_text_color());
                    let gw = g.size().x;
                    ui.painter().galley(Pos2::new(x, cy - g.size().y / 2.0), g, v.weak_text_color());
                    let g = ui.painter().layout_no_wrap(row.label.clone(), crate::fonts::ui_font(14.0), v.strong_text_color());
                    ui.painter().galley(Pos2::new(x + gw.max(52.0), cy - g.size().y / 2.0), g, v.strong_text_color());
                    if !row.keys.is_empty() {
                        let g = ui.painter().layout_no_wrap(row.keys.to_string(), crate::fonts::ui_font(12.0), v.weak_text_color());
                        let kr = Rect::from_min_size(Pos2::new(rect.right() - 12.0 - g.size().x - 12.0, cy - 10.0), Vec2::new(g.size().x + 12.0, 20.0));
                        ui.painter().rect_stroke(kr, 5.0, Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);
                        ui.painter().galley(Pos2::new(kr.left() + 6.0, cy - g.size().y / 2.0), g, v.weak_text_color());
                    }
                    if resp.clicked() {
                        run = Some(row.cmd);
                    }
                }
            });
            if rows.is_empty() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("No matching commands").color(v.weak_text_color()));
                });
                ui.add_space(4.0);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Up/Down choose   ·   Enter run   ·   Esc close").size(11.5).color(v.weak_text_color()));
            });
        });
        if qa.query != before {
            qa.sel = 0;
        }
        if r.backdrop_response.clicked() {
            close = true;
        }
        if let Some(cmd) = run {
            // On / off options keep the list open so several can be flipped in a row;
            // everything else closes it.
            let keep = rows.iter().find(|r| r.cmd == cmd).is_some_and(|r| r.state.is_some() && !matches!(cmd, Cmd::Zen));
            if !keep {
                close = true;
            }
            if !close {
                self.actions = Some(qa);
            }
            self.run_action(ctx, cmd);
            return;
        }
        if !close {
            self.actions = Some(qa);
        }
    }
}
