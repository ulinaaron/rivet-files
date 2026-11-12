//! Title bar (when decorations are off), menus, toolbar, tabs, status bar and sidebar.

use eframe::egui::{self, Align2, Color32, CornerRadius, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::{R_MD, R_SM, accent_color, paint_tex, Action, Dialog, DragFiles, RivetApp};
use crate::fsops;
use crate::model::{self, Location};
use crate::places;
use crate::settings::{SortKey, ToolItem, ViewMode};

impl RivetApp {
    pub(super) fn window_resize_edges(&mut self, ctx: &egui::Context) {
        use egui::{CursorIcon, ResizeDirection as RD};
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let screen = ctx.content_rect();
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        let m = 5.0;
        let l = pos.x < screen.left() + m;
        let r = pos.x > screen.right() - m;
        let t = pos.y < screen.top() + m;
        let b = pos.y > screen.bottom() - m;
        let dir = match (l, r, t, b) {
            (true, _, true, _) => Some((RD::NorthWest, CursorIcon::ResizeNorthWest)),
            (_, true, true, _) => Some((RD::NorthEast, CursorIcon::ResizeNorthEast)),
            (true, _, _, true) => Some((RD::SouthWest, CursorIcon::ResizeSouthWest)),
            (_, true, _, true) => Some((RD::SouthEast, CursorIcon::ResizeSouthEast)),
            (true, ..) => Some((RD::West, CursorIcon::ResizeWest)),
            (_, true, ..) => Some((RD::East, CursorIcon::ResizeEast)),
            (_, _, true, _) => Some((RD::North, CursorIcon::ResizeNorth)),
            (_, _, _, true) => Some((RD::South, CursorIcon::ResizeSouth)),
            _ => None,
        };
        if let Some((d, c)) = dir {
            ctx.set_cursor_icon(c);
            if ctx.input(|i| i.pointer.primary_pressed()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
            }
        }
    }

    // -------------------------------------------------------------- menu bar / title bar

    /// Alt shows a hidden menu bar: held, it peeks; a quick tap keeps it open until a
    /// menu item is chosen, Escape is pressed or you click elsewhere.
    pub(super) fn update_menu_peek(&mut self, ctx: &egui::Context) {
        if self.s.show_menubar || self.screenshot.is_some() {
            self.menu_peek = false;
            self.alt_was_down = false;
            return;
        }
        let (alt, other_input, esc, press_pos) = ctx.input(|i| {
            let other = i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)))
                || i.pointer.any_pressed();
            let esc = i.key_pressed(egui::Key::Escape);
            let press = if i.pointer.primary_pressed() { i.pointer.interact_pos() } else { None };
            (i.modifiers.alt, other, esc, press)
        });
        if alt && !self.alt_was_down {
            self.alt_combo = false;
        }
        if alt && other_input {
            self.alt_combo = true; // Alt+Left etc. are shortcuts, not a menu request
        }
        if !alt && self.alt_was_down && !self.alt_combo {
            self.menu_peek = !self.menu_peek;
        }
        self.alt_was_down = alt;
        let popup_open = egui::Popup::is_any_open(ctx);
        if self.menu_peek {
            if esc && !popup_open {
                self.menu_peek = false;
            }
            if let Some(p) = press_pos {
                if !popup_open && !self.menubar_rect.is_some_and(|r| r.contains(p)) {
                    self.menu_peek = false;
                }
            }
            // A menu was open and has just closed (an item was chosen): hide again.
            if self.peek_popup_was_open && !popup_open && !alt {
                self.menu_peek = false;
            }
        }
        self.peek_popup_was_open = popup_open && self.menu_peek;
    }

    fn menubar_visible(&self) -> bool {
        self.s.show_menubar || self.menu_peek || (self.alt_was_down && !self.alt_combo)
    }

    pub(super) fn header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let csd = !self.s.decorations;
        let show_menu = self.menubar_visible();
        if !show_menu && !csd {
            self.menubar_rect = None;
            return;
        }
        let fill = ctx.global_style().visuals.panel_fill;
        let panel = egui::Panel::top("menubar").show_separator_line(false)
            .frame(egui::Frame::NONE.fill(fill).inner_margin(egui::Margin::symmetric(6, if csd { 4 } else { 2 })))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if csd {
                        if let Some(t) = self.app_icon_texture(ctx) {
                            ui.add(egui::Image::from_texture(&t).fit_to_exact_size(Vec2::splat(18.0)));
                        }
                    }
                    if show_menu {
                        if csd {
                            // The menu bar takes all the width it is given, so give it just enough.
                            let font = egui::TextStyle::Button.resolve(ui.style());
                            let pad = ui.spacing().button_padding.x * 2.0 + ui.spacing().item_spacing.x;
                            let w: f32 = ["File", "Edit", "View", "Go", "Bookmarks", "Help"]
                                .iter()
                                .map(|l| ui.painter().layout_no_wrap(l.to_string(), font.clone(), Color32::WHITE).size().x + pad)
                                .sum::<f32>()
                                + 8.0;
                            ui.allocate_ui_with_layout(Vec2::new(w, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_max_width(w);
                                egui::MenuBar::new().ui(ui, |ui| self.menus(ui, ctx));
                            });
                        } else {
                            egui::MenuBar::new().ui(ui, |ui| self.menus(ui, ctx));
                        }
                    }
                    if csd {
                        // The remaining space is the drag handle, with the title centred.
                        let btn_w = 3.0 * 32.0;
                        let avail = ui.available_width() - btn_w;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail.max(10.0), 24.0), Sense::click_and_drag());
                        let full = ui.max_rect();
                        ui.painter().with_clip_rect(rect).text(
                            Pos2::new(full.center().x.max(rect.left() + 80.0), rect.center().y),
                            Align2::CENTER_CENTER,
                            &self.last_title,
                            crate::fonts::ui_font(13.5),
                            ui.visuals().strong_text_color(),
                        );
                        if resp.drag_started_by(egui::PointerButton::Primary) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                        if resp.double_clicked() {
                            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                        }
                        self.window_buttons(ui, ctx);
                    }
                });
            });
        self.menubar_rect = Some(panel.response.rect);
    }

    fn window_buttons(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let restore: &[&str] = &["window-restore-symbolic", "window-restore"];
        let maximize: &[&str] = &["window-maximize-symbolic", "window-maximize"];
        let items: [(&[&str], u8); 3] = [
            (&["window-minimize-symbolic", "window-minimize"], 0),
            (if max { restore } else { maximize }, 1),
            (&["window-close-symbolic", "window-close"], 2),
        ];
        for (names, kind) in items {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 24.0), Sense::click());
            let hovered = resp.hovered();
            let fg = ui.visuals().text_color();
            if hovered {
                let c = if kind == 2 { Color32::from_rgb(0xd9, 0x3f, 0x3f) } else { ui.visuals().widgets.hovered.bg_fill };
                ui.painter().circle_filled(rect.center(), 11.0, c);
            }
            let tint = if hovered && kind == 2 { Color32::WHITE } else { fg };
            if let Some(t) = self.icon(ctx, names, 16.0) {
                paint_tex(ui.painter(), &t, Rect::from_center_size(rect.center(), Vec2::splat(16.0)), tint);
            } else {
                let c = rect.center();
                let s = Stroke::new(1.5, tint);
                match kind {
                    0 => {
                        ui.painter().line_segment([c + Vec2::new(-5.0, 4.0), c + Vec2::new(5.0, 4.0)], s);
                    }
                    1 => {
                        ui.painter().rect_stroke(Rect::from_center_size(c, Vec2::splat(10.0)), 1.0, s, StrokeKind::Middle);
                    }
                    _ => {
                        ui.painter().line_segment([c + Vec2::new(-5.0, -5.0), c + Vec2::new(5.0, 5.0)], s);
                        ui.painter().line_segment([c + Vec2::new(5.0, -5.0), c + Vec2::new(-5.0, 5.0)], s);
                    }
                }
            }
            if resp.clicked() {
                match kind {
                    0 => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                    1 => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max)),
                    _ => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                }
            }
        }
    }

    fn menus(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let in_dir = self.current_dir();
        let sel = self.tab().selected_paths();
        let in_trash = matches!(self.tab().loc, Location::Trash);
        ui.menu_button("File", |ui| {
            if ui.add_enabled(in_dir.is_some(), egui::Button::new("Create New Folder").shortcut_text("Shift+Ctrl+N")).clicked() {
                self.pending.push(Action::NewFolder);
            }
            if ui.add_enabled(in_dir.is_some(), egui::Button::new("Create New Document")).clicked() {
                self.pending.push(Action::NewFile(None));
            }
            ui.separator();
            if ui.add(egui::Button::new("New Tab").shortcut_text("Ctrl+T")).clicked() {
                self.pending.push(Action::NewTab);
            }
            if ui.add_enabled(in_dir.is_some(), egui::Button::new("Open in Terminal")).clicked() {
                if let Some(d) = in_dir.clone() {
                    self.pending.push(Action::Terminal(d));
                }
            }
            ui.separator();
            if ui.add(egui::Button::new("Properties").shortcut_text("Ctrl+I")).clicked() {
                self.pending.push(Action::Properties(sel.first().cloned()));
            }
            if in_trash && ui.button("Empty Trash").clicked() {
                self.pending.push(Action::EmptyTrash);
            }
            ui.separator();
            if ui.add(egui::Button::new("Close Tab").shortcut_text("Ctrl+W")).clicked() {
                self.pending.push(Action::CloseTab(self.active));
            }
            if ui.add(egui::Button::new("Close Window").shortcut_text("Ctrl+Q")).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("Edit", |ui| {
            let has = !sel.is_empty();
            if ui.add_enabled(has && !in_trash, egui::Button::new("Cut").shortcut_text("Ctrl+X")).clicked() {
                self.pending.push(Action::Cut);
            }
            if ui.add_enabled(has && !in_trash, egui::Button::new("Copy").shortcut_text("Ctrl+C")).clicked() {
                self.pending.push(Action::Copy);
            }
            if ui.add_enabled(self.clipboard.is_some() && in_dir.is_some(), egui::Button::new("Paste").shortcut_text("Ctrl+V")).clicked() {
                self.pending.push(Action::Paste(None));
            }
            ui.separator();
            if ui.add(egui::Button::new("Select All").shortcut_text("Ctrl+A")).clicked() {
                self.pending.push(Action::SelectAll);
            }
            if ui.button("Copy Location").clicked() {
                self.pending.push(Action::CopyLocation);
            }
            ui.separator();
            if ui.add_enabled(sel.len() == 1 && !in_trash, egui::Button::new("Rename…").shortcut_text("F2")).clicked() {
                self.pending.push(Action::Rename(sel[0].clone()));
            }
            if in_trash {
                if ui.add_enabled(has, egui::Button::new("Restore")).clicked() {
                    self.pending.push(Action::Restore);
                }
            } else if ui.add_enabled(has, egui::Button::new("Move to Trash").shortcut_text("Delete")).clicked() {
                self.pending.push(Action::Trash);
            }
            if ui.add_enabled(has, egui::Button::new("Delete Permanently").shortcut_text("Shift+Delete")).clicked() {
                self.pending.push(Action::Delete);
            }
            ui.separator();
            if ui.button("Preferences").clicked() {
                self.open_prefs();
            }
        });
        ui.menu_button("View", |ui| {
            if ui.add(egui::Button::new("Reload").shortcut_text("F5")).clicked() {
                self.reload_all();
            }
            ui.separator();
            for (v, label, key) in [(ViewMode::Icons, "Icons", "Ctrl+1"), (ViewMode::List, "List", "Ctrl+2"), (ViewMode::Compact, "Compact", "Ctrl+3")] {
                if ui.add(egui::Button::selectable(self.s.view == v, label).shortcut_text(key)).clicked() {
                    self.s.view = v;
                    self.dirty_settings = true;
                }
            }
            ui.separator();
            if ui.checkbox(&mut self.s.show_hidden, "Show Hidden Files").changed() {
                self.resort_all();
            }
            let mut c = false;
            c |= ui.checkbox(&mut self.s.show_sidebar, "Sidebar").changed();
            c |= ui.checkbox(&mut self.s.show_statusbar, "Statusbar").changed();
            c |= ui.checkbox(&mut self.s.show_menubar, "Menubar (Ctrl+M)").changed();
            c |= ui.checkbox(&mut self.s.show_thumbnails, "Picture Thumbnails").changed();
            if c {
                self.dirty_settings = true;
            }
            ui.separator();
            ui.menu_button("Sort Items", |ui| self.sort_menu(ui));
            ui.separator();
            if ui.add(egui::Button::new("Zoom In").shortcut_text("Ctrl++")).clicked() {
                self.zoom(1);
            }
            if ui.add(egui::Button::new("Zoom Out").shortcut_text("Ctrl+-")).clicked() {
                self.zoom(-1);
            }
        });
        ui.menu_button("Go", |ui| {
            if ui.add_enabled(!self.tab().back.is_empty(), egui::Button::new("Back").shortcut_text("Alt+Left")).clicked() {
                self.go_back();
            }
            if ui.add_enabled(!self.tab().forward.is_empty(), egui::Button::new("Forward").shortcut_text("Alt+Right")).clicked() {
                self.go_forward();
            }
            if ui.add(egui::Button::new("Open Parent").shortcut_text("Alt+Up")).clicked() {
                self.go_up();
            }
            ui.separator();
            if ui.add(egui::Button::new("Home").shortcut_text("Alt+Home")).clicked() {
                self.pending.push(Action::Navigate(Location::Dir(places::home())));
            }
            if ui.button("File System").clicked() {
                self.pending.push(Action::Navigate(Location::Dir("/".into())));
            }
            if ui.button("Recent").clicked() {
                self.pending.push(Action::Navigate(Location::Recent));
            }
            if ui.button("Trash").clicked() {
                self.pending.push(Action::Navigate(Location::Trash));
            }
            if ui.button("Network").clicked() {
                self.pending.push(Action::Navigate(Location::Network));
            }
            ui.separator();
            if ui.add(egui::Button::new("Location…").shortcut_text("Ctrl+L")).clicked() {
                self.path_editing = true;
                self.path_focus = true;
                self.path_text = self.current_dir().map(|d| d.display().to_string()).unwrap_or_default();
            }
            if ui.add(egui::Button::new("Search for Files…").shortcut_text("Ctrl+F")).clicked() {
                self.search_open = true;
                self.search_focus = true;
            }
        });
        ui.menu_button("Bookmarks", |ui| {
            if ui.add_enabled(in_dir.is_some(), egui::Button::new("Add to Bookmarks").shortcut_text("Ctrl+D")).clicked() {
                if let Some(d) = in_dir.clone() {
                    self.pending.push(Action::AddBookmark(d));
                }
            }
            if !self.bookmarks.is_empty() {
                ui.separator();
            }
            for b in self.bookmarks.clone() {
                if ui.button(&b.label).clicked() {
                    self.pending.push(Action::Navigate(Location::Dir(b.path.clone())));
                }
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.add(egui::Button::new("Keyboard Shortcuts").shortcut_text("F1")).clicked() {
                self.dialog = Dialog::Shortcuts;
            }
            ui.separator();
            if ui.button("About Rivet").clicked() {
                self.dialog = Dialog::About;
            }
        });
    }

    pub(super) fn sort_menu(&mut self, ui: &mut egui::Ui) {
        for (k, l) in [(SortKey::Name, "By Name"), (SortKey::Size, "By Size"), (SortKey::Type, "By Type"), (SortKey::Modified, "By Modification Date")] {
            if ui.radio(self.s.sort == k, l).clicked() {
                self.s.sort = k;
                self.resort_all();
            }
        }
        ui.separator();
        let a = ui.checkbox(&mut self.s.sort_desc, "Reversed Order").changed();
        let b = ui.checkbox(&mut self.s.folders_first, "Folders First").changed();
        if a || b {
            self.resort_all();
        }
    }

    // -------------------------------------------------------------- toolbar

    pub(super) fn tool_button(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, icons: &[&str], fallback: &str, tip: &str, enabled: bool, selected: bool) -> bool {
        let tex = self.icon(ctx, icons, 16.0);
        let flat = self.toolbar_drawing || self.flat_buttons;
        // Active buttons get the same look as a selected file; the icon keeps the normal text colour.
        let fg = if selected && !flat { ui.visuals().selection.stroke.color } else if selected { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        let btn = match tex {
            Some(t) => egui::Button::image(egui::Image::from_texture(&t).fit_to_exact_size(Vec2::splat(16.0)).tint(fg)),
            None => egui::Button::new(fallback),
        };
        let size = if self.toolbar_drawing { Vec2::new(34.0, 32.0) } else { Vec2::new(30.0, 28.0) };
        // Reserve a slot underneath the button for the selected tile.
        let under = ui.painter().add(egui::Shape::Noop);
        let btn = btn.selected(selected && !flat).min_size(size).frame_when_inactive(!flat || (selected && !flat));
        let resp = ui.add_enabled(enabled, btn);
        if selected && flat {
            let v = ui.visuals();
            let (fill, stroke) = super::selected_tile(v, v.panel_fill);
            let r = resp.rect;
            ui.painter().set(under, egui::Shape::Vec(vec![
                egui::Shape::rect_filled(r, R_SM, fill),
                egui::Shape::rect_stroke(r, R_SM, stroke, StrokeKind::Inside),
            ]));
        }
        resp.on_hover_text(tip).clicked()
    }

    /// Draw one configurable toolbar item.
    fn toolbar_item(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, item: ToolItem) {
        use ToolItem::*;
        if item == Separator {
            ui.add_space(7.0);
            return;
        }
        if item == Menu {
            if !self.s.show_menubar {
                self.hamburger_menu(ui, ctx);
            }
            return;
        }
        let (icons, fb) = item.icons();
        let (enabled, selected, tip): (bool, bool, String) = match item {
            Back => (!self.tab().back.is_empty(), false, "Go back".into()),
            Forward => (!self.tab().forward.is_empty(), false, "Go forward".into()),
            Up => (true, false, "Open the parent folder".into()),
            EditLocation => (true, self.path_editing, "Type a location (Ctrl+L)".into()),
            Search => (true, self.search_open, "Search (Ctrl+F)".into()),
            ViewIcons => (true, self.s.view == ViewMode::Icons, "Icon view".into()),
            ViewList => (true, self.s.view == ViewMode::List, "List view".into()),
            ViewCompact => (true, self.s.view == ViewMode::Compact, "Compact view".into()),
            NewFolder | Terminal | Bookmark => (self.current_dir().is_some(), false, item.label().into()),
            ShowHidden => (true, self.s.show_hidden, "Show hidden files (Ctrl+H)".into()),
            Sidebar => (true, self.s.show_sidebar, "Show or hide the sidebar (F9)".into()),
            Trash => (true, self.tab().loc == Location::Trash, "Trash".into()),
            _ => (true, false, item.label().into()),
        };
        if !self.tool_button(ui, ctx, icons, fb, &tip, enabled, selected) {
            return;
        }
        match item {
            Back => self.go_back(),
            Forward => self.go_forward(),
            Up => self.go_up(),
            Reload => self.reload_all(),
            Home => self.pending.push(Action::Navigate(Location::Dir(places::home()))),
            Computer => self.pending.push(Action::Navigate(Location::Dir("/".into()))),
            EditLocation => {
                self.path_editing = !self.path_editing;
                if self.path_editing {
                    self.path_focus = true;
                    self.path_text = self.current_dir().map(|d| d.display().to_string()).unwrap_or_default();
                }
            }
            Search => {
                self.search_open = !self.search_open;
                self.search_focus = self.search_open;
                if !self.search_open {
                    self.tab_mut().filter.clear();
                    self.tabs[self.active].resort(&self.mime, &self.s);
                }
            }
            ViewIcons | ViewList | ViewCompact => {
                self.s.view = match item {
                    ViewIcons => ViewMode::Icons,
                    ViewList => ViewMode::List,
                    _ => ViewMode::Compact,
                };
                self.dirty_settings = true;
            }
            NewFolder => self.pending.push(Action::NewFolder),
            NewTab => self.pending.push(Action::NewTab),
            Terminal => {
                if let Some(d) = self.current_dir() {
                    self.pending.push(Action::Terminal(d));
                }
            }
            ShowHidden => {
                self.s.show_hidden = !self.s.show_hidden;
                self.resort_all();
            }
            Trash => self.pending.push(Action::Navigate(Location::Trash)),
            Properties => {
                let sel = self.tab().selected_paths();
                self.pending.push(Action::Properties(sel.first().cloned()));
            }
            Bookmark => {
                if let Some(d) = self.current_dir() {
                    self.pending.push(Action::AddBookmark(d));
                }
            }
            Sidebar => {
                self.s.show_sidebar = !self.s.show_sidebar;
                self.dirty_settings = true;
            }
            PathBar | Separator | Menu => {}
        }
    }

    /// With the menu bar hidden, its menus live behind this hamburger button.
    fn hamburger_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let fg = ui.visuals().text_color();
        let btn = egui::Button::new("").min_size(Vec2::new(34.0, 32.0)).frame_when_inactive(false);
        let r = egui::containers::menu::MenuButton::from_button(btn).ui(ui, |ui| {
            if ui.add(egui::Button::new("Show Menubar").shortcut_text("Ctrl+M")).clicked() {
                self.s.show_menubar = true;
                self.dirty_settings = true;
            }
            ui.separator();
            self.menus(ui, ctx);
        });
        // Drawn as three bars so it looks the same in every icon theme.
        let c = r.0.rect.center();
        for dy in [-4.5f32, 0.0, 4.5] {
            ui.painter().line_segment([c + Vec2::new(-6.5, dy), c + Vec2::new(6.5, dy)], Stroke::new(1.8, fg));
        }
        r.0.on_hover_text("Menu — Ctrl+M shows the menu bar again, Alt shows it briefly");
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.toolbar_drawing = true;
        let fill = ctx.global_style().visuals.panel_fill;
        let layout = self.s.toolbar.clone();
        let split = layout.iter().position(|t| *t == ToolItem::PathBar).unwrap_or(layout.len());
        let (left, right) = (layout[..split].to_vec(), layout[(split + 1).min(layout.len())..].to_vec());
        egui::Panel::top("toolbar").show_separator_line(false)
            .frame(egui::Frame::NONE.fill(fill).inner_margin(egui::Margin::symmetric(12, 9)))
            .show(ui, |ui| {
                // Right-click on empty toolbar space to customise it.
                let bg = ui.interact(ui.max_rect(), Id::unique("toolbar-bg"), Sense::click());
                bg.context_menu(|ui| {
                    if ui.button("Customise Toolbar…").clicked() {
                        self.open_prefs();
                    }
                    if ui.button("Reset Toolbar").clicked() {
                        self.s.toolbar = ToolItem::default_layout();
                        self.dirty_settings = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    for item in &left {
                        self.toolbar_item(ui, ctx, *item);
                    }
                    ui.add_space(8.0);
                    let menubar = self.s.show_menubar;
                    let width = |t: &ToolItem| if *t == ToolItem::Menu && menubar { 0.0 } else { t.width() };
                    let right_w: f32 = right.iter().map(width).sum::<f32>() + 14.0;
                    let path_w = (ui.available_width() - right_w).max(80.0);
                    // The location bar sits on its own raised surface.
                    let v = ui.visuals();
                    let elevated = crate::palette::mix(v.window_fill, v.text_color(), 0.04);
                    let shadow = egui::Shadow { offset: [0, 1], blur: 6, spread: 0, color: Color32::from_black_alpha(if v.dark_mode { 90 } else { 40 }) };
                    ui.allocate_ui_with_layout(Vec2::new(path_w, 32.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        egui::Frame::NONE.fill(elevated).shadow(shadow).corner_radius(R_MD).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                            // Even padding all round; pills inside use R_MD − 4 = R_SM, so corners nest.
                            ui.set_min_width(ui.available_width());
                            ui.set_min_height(24.0);
                            if self.search_open {
                                self.search_bar(ui, ctx);
                            } else if self.path_editing {
                                self.location_entry(ui);
                            } else {
                                self.breadcrumbs(ui, ctx);
                            }
                        });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Right-to-left layout: draw in reverse so the saved order reads left to right.
                        for item in right.iter().rev() {
                            self.toolbar_item(ui, ctx, *item);
                        }
                    });
                });
            });
        self.toolbar_drawing = false;
    }

    pub(super) fn start_path_editing(&mut self) {
        self.path_editing = true;
        self.path_focus = true;
        self.path_text = self.current_dir().map(|d| d.display().to_string()).unwrap_or_default();
    }

    fn location_entry(&mut self, ui: &mut egui::Ui) {
        let resp = ui.add(egui::TextEdit::singleline(&mut self.path_text).frame(egui::Frame::NONE).vertical_align(egui::Align::Center).min_size(Vec2::new(0.0, 26.0)).desired_width(f32::INFINITY).hint_text("Type a path, e.g. /home or ~/Documents"));
        if self.path_focus {
            resp.request_focus();
            // Select the whole path so typing replaces it.
            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
                st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(self.path_text.chars().count()),
                )));
                st.store(ui.ctx(), resp.id);
            }
            self.path_focus = false;
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if resp.lost_focus() && !enter {
            // Clicked elsewhere or pressed Escape: back to the folder buttons.
            self.path_editing = false;
        }
        if resp.lost_focus() && enter {
            let txt = self.path_text.trim().to_string();
            let p = if let Some(rest) = txt.strip_prefix('~') {
                places::home().join(rest.trim_start_matches('/'))
            } else if let Some(r) = txt.strip_prefix("file://") {
                PathBuf::from(places::percent_decode(r))
            } else {
                PathBuf::from(&txt)
            };
            if p.is_dir() {
                self.pending.push(Action::Navigate(Location::Dir(p)));
                self.path_editing = false;
            } else if p.is_file() {
                self.pending.push(Action::Open(p));
                self.path_editing = false;
            } else {
                self.message("Location not found", &format!("“{txt}” does not exist."));
            }
        }
    }

    fn search_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let in_dir = self.current_dir().is_some() || matches!(self.tab().loc, Location::Search { .. });
        let changed;
        let mut run_deep = false;
        // Search icon at the start of the field.
        let fg = ui.visuals().weak_text_color();
        if let Some(t) = self.icon(ctx, &["edit-find-symbolic", "system-search-symbolic"], 16.0) {
            let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 26.0), Sense::hover());
            paint_tex(ui.painter(), &t, Rect::from_center_size(r.center(), Vec2::splat(15.0)), fg);
        }
        {
            let focus = self.search_focus;
            let t = &mut self.tabs[self.active];
            let resp = ui.add(
                egui::TextEdit::singleline(&mut t.filter)
                    .frame(egui::Frame::NONE).vertical_align(egui::Align::Center).min_size(Vec2::new(0.0, 26.0))
                    .desired_width((ui.available_width() - 150.0).max(80.0))
                    .hint_text("Filter this folder — press Enter to search all subfolders"),
            );
            if focus {
                resp.request_focus();
                if let Some(mut state) = egui::TextEdit::load_state(ctx, resp.id) {
                    let cc = egui::text::CCursor::new(t.filter.chars().count());
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(cc)));
                    state.store(ctx, resp.id);
                }
            }
            changed = resp.changed();
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !t.filter.trim().is_empty() {
                run_deep = true;
            }
            if t.searching {
                ui.spinner();
            }
        }
        self.search_focus = false;
        // Flat like the rest of the raised location bar; a background only on hover.
        if ui.add_enabled(in_dir, egui::Button::new("Search subfolders").frame_when_inactive(false)).clicked() {
            run_deep = true;
        }
        if changed && !matches!(self.tab().loc, Location::Search { .. }) {
            self.tabs[self.active].resort(&self.mime, &self.s);
        }
        if run_deep {
            let q = self.tab().filter.trim().to_string();
            let root = match &self.tab().loc {
                Location::Dir(p) => Some(p.clone()),
                Location::Search { root, .. } => Some(root.clone()),
                _ => None,
            };
            if let (Some(root), false) = (root, q.is_empty()) {
                let rx = model::spawn_search(root.clone(), q.clone(), self.s.show_hidden, ctx.clone());
                let loc = Location::Search { root, query: q };
                let t = &mut self.tabs[self.active];
                if !matches!(t.loc, Location::Search { .. }) {
                    let old = std::mem::replace(&mut t.loc, loc);
                    t.back.push(old);
                    t.forward.clear();
                } else {
                    t.loc = loc;
                }
                t.entries.clear();
                t.visible.clear();
                t.selected.clear();
                t.filter.clear();
                t.search_rx = Some(rx);
                t.searching = true;
                self.search_open = false;
            }
        }
    }

    fn breadcrumbs(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let loc = self.tab().loc.clone();
        let mut crumbs: Vec<(String, Option<Location>, &'static str)> = Vec::new();
        match &loc {
            Location::Dir(p) => {
                let home = places::home();
                let (rest, mut acc): (Vec<_>, PathBuf) = if p.starts_with(&home) {
                    let user = home.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Home".into());
                    crumbs.push((user, Some(Location::Dir(home.clone())), "user-home"));
                    (p.strip_prefix(&home).unwrap().components().collect(), home.clone())
                } else {
                    crumbs.push(("File System".into(), Some(Location::Dir("/".into())), "drive-harddisk"));
                    (p.strip_prefix("/").unwrap_or(p).components().collect(), PathBuf::from("/"))
                };
                for c in rest {
                    acc.push(c);
                    crumbs.push((c.as_os_str().to_string_lossy().to_string(), Some(Location::Dir(acc.clone())), ""));
                }
            }
            Location::Trash => crumbs.push(("Trash".into(), None, "user-trash")),
            Location::Recent => crumbs.push(("Recent".into(), None, "document-open-recent")),
            Location::Network => crumbs.push(("Network".into(), None, "network-workgroup")),
            Location::Search { root, query } => {
                crumbs.push((format!("Search for “{query}” in {}", root.display()), None, "edit-find"));
            }
        }
        let last = crumbs.len() - 1;
        // Clicking the empty part of the location bar turns it into a text field.
        let area = ui.max_rect();
        let bg = ui.interact(area, Id::unique("crumbs-bg"), Sense::click());
        if bg.hovered() {
            // Lift the bar a touch on hover instead of drawing an outline.
            ui.painter().rect_filled(area.expand(4.0), R_MD, ui.visuals().text_color().gamma_multiply(0.05));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if bg.clicked() {
            self.start_path_editing();
        }
        egui::ScrollArea::horizontal()
            .id_salt("crumbs")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .stick_to_right(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (i, (label, target, icon)) in crumbs.into_iter().enumerate() {
                        let tex = if icon.is_empty() { None } else { self.icon(ctx, &[&format!("{icon}-symbolic"), icon], 16.0) };
                        let sel = i == last;
                        let fg = if sel { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
                        let text = if sel { egui::RichText::new(&label).strong().color(fg) } else { egui::RichText::new(&label) };
                        let btn = match tex {
                            Some(t) => egui::Button::image_and_text(egui::Image::from_texture(&t).fit_to_exact_size(Vec2::splat(16.0)).tint(fg), text),
                            None => egui::Button::new(text),
                        };
                        let resp = ui.add(btn.selected(sel).frame_when_inactive(sel).min_size(Vec2::new(0.0, 24.0)));
                        if resp.clicked() {
                            if let Some(t) = target.clone() {
                                self.pending.push(Action::Navigate(t));
                            }
                        }
                        if let Some(Location::Dir(d)) = &target {
                            if let Some(p) = resp.dnd_release_payload::<DragFiles>() {
                                self.pending.push(Action::Transfer(p.0.clone(), d.clone(), ctx.input(|i| i.modifiers.ctrl)));
                            }
                        }
                    }
                });
            });
    }

    pub(super) fn tab_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let fill = ctx.global_style().visuals.panel_fill;
        egui::Panel::top("tabs").show_separator_line(false).frame(egui::Frame::NONE.fill(fill).inner_margin(egui::Margin::symmetric(6, 2))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let n = self.tabs.len();
                let w = ((ui.available_width() - 40.0) / n as f32).clamp(80.0, 240.0);
                for i in 0..n {
                    let title = self.tabs[i].loc.title();
                    let sel = i == self.active;
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::click());
                    let v = ui.visuals().clone();
                    let bg = if sel { v.extreme_bg_color } else if resp.hovered() { v.widgets.hovered.weak_bg_fill } else { v.panel_fill };
                    ui.painter().rect_filled(rect, CornerRadius { nw: R_SM, ne: R_SM, sw: 0, se: 0 }, bg);
                    if sel {
                        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(2.0, accent_color(&self.s)));
                    }
                    let close_rect = Rect::from_center_size(Pos2::new(rect.right() - 14.0, rect.center().y), Vec2::splat(16.0));
                    let col = if sel { v.strong_text_color() } else { v.text_color() };
                    let mut job = egui::text::LayoutJob::simple_singleline(title, crate::fonts::ui_font(13.0), col);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(close_rect.left() - rect.left() - 14.0);
                    let galley = ui.painter().layout_job(job);
                    ui.painter().galley(Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, col);
                    let close = ui.interact(close_rect, Id::unique(("tabclose", i)), Sense::click());
                    if close.hovered() {
                        ui.painter().circle_filled(close_rect.center(), 8.0, v.widgets.hovered.bg_fill);
                    }
                    let c = close_rect.center();
                    let s = Stroke::new(1.2, col);
                    ui.painter().line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], s);
                    ui.painter().line_segment([c + Vec2::new(3.5, -3.5), c + Vec2::new(-3.5, 3.5)], s);
                    if close.clicked() || resp.clicked_by(egui::PointerButton::Middle) {
                        self.pending.push(Action::CloseTab(i));
                    } else if resp.clicked() {
                        self.active = i;
                    }
                    if let Some(p) = resp.dnd_release_payload::<DragFiles>() {
                        if let Some(d) = self.tabs[i].loc.dir() {
                            self.pending.push(Action::Transfer(p.0.clone(), d.to_path_buf(), ctx.input(|i| i.modifiers.ctrl)));
                        }
                    }
                }
                if ui.button("+").on_hover_text("New tab (Ctrl+T)").clicked() {
                    self.pending.push(Action::NewTab);
                }
            });
        });
    }

    // -------------------------------------------------------------- status bar

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let fill = ctx.global_style().visuals.panel_fill;
        egui::Panel::bottom("status").show_separator_line(false)
            .frame(egui::Frame::NONE.fill(fill).inner_margin(egui::Margin { left: 6, right: 6, top: 3, bottom: 6 }))
            .show(ui, |ui| {
                self.flat_buttons = true;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (sb, tree) = (self.s.show_sidebar, self.s.sidebar_tree);
                    if self.tool_button(ui, ctx, &["user-bookmarks-symbolic", "starred-symbolic", "bookmark-new-symbolic"], "★", "Show places", true, sb && !tree) {
                        self.s.show_sidebar = true;
                        self.s.sidebar_tree = false;
                        self.dirty_settings = true;
                    }
                    if self.tool_button(ui, ctx, &["view-list-tree-symbolic", "folder-symbolic", "view-list-symbolic"], "🌲", "Show the folder tree", true, sb && tree) {
                        self.s.show_sidebar = true;
                        self.s.sidebar_tree = true;
                        self.dirty_settings = true;
                    }
                    if self.tool_button(ui, ctx, &["sidebar-show-symbolic", "view-sidebar-symbolic", "view-dual-symbolic"], "◧", "Hide the sidebar (F9)", true, false) {
                        self.s.show_sidebar = !self.s.show_sidebar;
                        self.dirty_settings = true;
                    }
                    ui.add_space(6.0);
                    let text = self.status_text();
                    let zoom_w = 180.0;
                    ui.allocate_ui_with_layout(Vec2::new((ui.available_width() - zoom_w).max(50.0), 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(ui.available_width());
                        if let Some(op) = self.ops.first().cloned() {
                            let done = op.done_items.load(Ordering::Relaxed) as f32;
                            ui.add(egui::ProgressBar::new(done / op.total_items as f32).desired_width(120.0).desired_height(10.0));
                            ui.label(format!("{} “{}”…", op.kind.verb(), op.current.lock().unwrap()));
                            if ui.small_button("Cancel").clicked() {
                                op.cancel.store(true, Ordering::Relaxed);
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(text);
                            });
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new("+").small().frame_when_inactive(false)).on_hover_text("Zoom in").clicked() {
                            self.zoom(1);
                        }
                        let (val, range) = match self.s.view {
                            ViewMode::Icons => (&mut self.s.zoom, 32.0..=256.0),
                            _ => (&mut self.s.list_zoom, 16.0..=64.0),
                        };
                        ui.spacing_mut().slider_width = 110.0;
                        if ui.add(egui::Slider::new(val, range).show_value(false)).changed() {
                            self.dirty_settings = true;
                        }
                        if ui.add(egui::Button::new("−").small().frame_when_inactive(false)).on_hover_text("Zoom out").clicked() {
                            self.zoom(-1);
                        }
                    });
                });
            });
    }

    fn status_text(&self) -> String {
        if let Some((m, _)) = &self.status_msg {
            return m.clone();
        }
        let t = self.tab();
        let sel = t.selected_entries();
        let free = t.loc.dir().and_then(places::disk_space).map(|(_, f)| format!(", Free space: {}", fsops::human_size(f))).unwrap_or_default();
        if t.searching {
            return format!("Searching… {} found", t.visible.len());
        }
        if sel.is_empty() {
            let n = t.visible.len();
            return format!("{n} item{}{free}", if n == 1 { "" } else { "s" });
        }
        if sel.len() == 1 {
            let e = sel[0];
            if e.is_dir {
                let c = e.child_count.unwrap_or(0);
                return format!("“{}” selected (containing {c} item{}){free}", e.name, if c == 1 { "" } else { "s" });
            }
            return format!("“{}” selected ({}){free}", e.name, fsops::human_size(e.size));
        }
        let folders = sel.iter().filter(|e| e.is_dir).count();
        let files: Vec<_> = sel.iter().filter(|e| !e.is_dir).collect();
        let bytes: u64 = files.iter().map(|e| e.size).sum();
        let mut parts = Vec::new();
        if folders > 0 {
            parts.push(format!("{folders} folder{} selected", if folders == 1 { "" } else { "s" }));
        }
        if !files.is_empty() {
            parts.push(format!("{} other item{} selected ({})", files.len(), if files.len() == 1 { "" } else { "s" }, fsops::human_size(bytes)));
        }
        format!("{}{free}", parts.join(", "))
    }

    // -------------------------------------------------------------- sidebar

    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let fill = ctx.global_style().visuals.panel_fill;
        let resp = egui::Panel::left("sidebar").show_separator_line(false)
            .resizable(true)
            .default_size(self.s.sidebar_width)
            .size_range(120.0..=420.0)
            .frame(egui::Frame::NONE.fill(fill).inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 10 }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.s.sidebar_tree {
                        self.tree_contents(ui, ctx);
                    } else {
                        self.sidebar_contents(ui, ctx);
                    }
                });
            });
        let w = resp.response.rect.width();
        if (w - self.s.sidebar_width).abs() > 1.0 && ctx.input(|i| !i.pointer.any_down()) {
            self.s.sidebar_width = w;
            self.dirty_settings = true;
        }
    }

    /// A collapsible section title ("▾ My Computer"). Returns whether the section is open.
    fn section_header(&mut self, ui: &mut egui::Ui, text: &str) -> bool {
        ui.add_space(10.0);
        let open = !self.collapsed.contains(text);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0 * crate::fonts::scale().max(1.0)), Sense::click());
        let col = if resp.hovered() { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
        let c = Pos2::new(rect.left() + 12.0, rect.center().y);
        let pts = if open {
            vec![c + Vec2::new(-4.0, -2.0), c + Vec2::new(4.0, -2.0), c + Vec2::new(0.0, 3.0)]
        } else {
            vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(-2.0, 4.0), c + Vec2::new(3.0, 0.0)]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        ui.painter().text(Pos2::new(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, text, crate::fonts::ui_font(13.5), col);
        if resp.clicked() {
            if open {
                self.collapsed.insert(text.to_string());
            } else {
                self.collapsed.remove(text);
            }
        }
        ui.add_space(1.0);
        open
    }

    /// One sidebar row. Returns the response (for menus) and whether the eject button was clicked.
    #[allow(clippy::too_many_arguments)]
    fn place_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, icon: &[&str], label: &str, loc: Option<Location>, usage: Option<f32>, eject: bool) -> (egui::Response, bool) {
        let active = loc.as_ref() == Some(&self.tab().loc);
        // Comfortable rows that grow with the chosen text size.
        let sc = crate::fonts::scale().max(1.0);
        // Icon size from Preferences (0 = no icons); rows grow a little for big icons.
        let isz = self.s.sidebar_icon;
        let grow = (isz - 16.0).max(0.0);
        let h = if usage.is_some() { 38.0 * sc + grow } else { 32.0 * sc + grow };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
        let v = ui.visuals().clone();
        let accent = accent_color(&self.s);
        if active {
            ui.painter().rect_filled(rect, R_MD, accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, R_MD, v.widgets.hovered.weak_bg_fill);
        }
        let droppable = loc.as_ref().is_some_and(|l| l.dir().is_some() || *l == Location::Trash);
        if droppable && resp.dnd_hover_payload::<DragFiles>().is_some() {
            ui.painter().rect_stroke(rect, R_MD, Stroke::new(2.0, accent), StrokeKind::Inside);
        }
        let fg = if active { v.selection.stroke.color } else { v.text_color() };
        let row_mid = if usage.is_some() { rect.top() + 15.0 * sc + grow / 2.0 } else { rect.center().y };
        // With icons off the text starts where an icon would have been.
        let icon_rect = if isz > 0.0 {
            Rect::from_center_size(Pos2::new(rect.left() + 12.0 + isz / 2.0, row_mid), Vec2::splat(isz))
        } else {
            Rect::from_center_size(Pos2::new(rect.left() + 2.0, row_mid), Vec2::new(0.0, 16.0))
        };
        if active {
            // On the highlight: single-colour icon in the highlight's text colour, or a backdrop.
            if isz > 0.0 {
                match self.accent_icon(ctx, icon, isz) {
                    Some((t, true)) => paint_tex(ui.painter(), &t, icon_rect, fg),
                    Some((t, false)) => paint_tex(ui.painter(), &t, icon_rect, fg),
                    None => {}
                }
            }
        } else if isz > 0.0 {
            if let Some(t) = self.icon(ctx, icon, isz) {
                paint_tex(ui.painter(), &t, icon_rect, Color32::WHITE);
            }
        }
        let text_right = rect.right() - if eject { 30.0 } else { 10.0 };
        let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), crate::fonts::ui_font(13.5), fg);
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_right - icon_rect.right() - 12.0);
        let g = ui.painter().layout_job(job);
        ui.painter().galley(Pos2::new(icon_rect.right() + 12.0, icon_rect.center().y - g.size().y / 2.0), g, fg);
        if let Some(u) = usage {
            let bar = Rect::from_min_max(Pos2::new(icon_rect.right() + 12.0, rect.bottom() - 9.0), Pos2::new(text_right, rect.bottom() - 6.0));
            ui.painter().rect_filled(bar, 1.5, if active { Color32::from_white_alpha(80) } else { v.widgets.inactive.bg_fill });
            let mut filled = bar;
            filled.set_right(bar.left() + bar.width() * u.clamp(0.0, 1.0));
            let c = if u > 0.9 { Color32::from_rgb(0xd9, 0x3f, 0x3f) } else if active { v.selection.stroke.color } else { accent };
            ui.painter().rect_filled(filled, 1.5, c);
        }
        let mut eject_clicked = false;
        if eject {
            let er = Rect::from_center_size(Pos2::new(rect.right() - 17.0, icon_rect.center().y), Vec2::splat(20.0));
            let eresp = ui.interact(er, resp.id.with("eject"), Sense::click()).on_hover_text("Eject");
            if eresp.hovered() {
                ui.painter().circle_filled(er.center(), 10.0, v.widgets.hovered.bg_fill);
            }
            if let Some(t) = self.icon(ctx, &["media-eject-symbolic", "media-eject"], 14.0) {
                paint_tex(ui.painter(), &t, Rect::from_center_size(er.center(), Vec2::splat(14.0)), fg);
            } else {
                ui.painter().text(er.center(), Align2::CENTER_CENTER, "⏏", crate::fonts::ui_font(13.0), fg);
            }
            eject_clicked = eresp.clicked();
        }
        if let Some(l) = &loc {
            if resp.clicked() && !eject_clicked {
                self.pending.push(Action::Navigate(l.clone()));
            }
            if resp.clicked_by(egui::PointerButton::Middle) {
                if let Some(d) = l.dir() {
                    self.pending.push(Action::OpenInNewTab(d.to_path_buf()));
                }
            }
            if let Some(p) = resp.dnd_release_payload::<DragFiles>() {
                match l {
                    Location::Dir(d) => self.pending.push(Action::Transfer(p.0.clone(), d.clone(), ctx.input(|i| i.modifiers.ctrl))),
                    Location::Trash => self.pending.push(Action::Transfer(p.0.clone(), places::trash_dir().join("files"), false)),
                    _ => {}
                }
            }
        }
        (resp, eject_clicked)
    }

    fn sidebar_contents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.spacing_mut().item_spacing.y = 3.0;
        if self.section_header(ui, "My Computer") {
            self.my_computer(ui, ctx);
        }
        self.other_sections(ui, ctx);
    }

    fn my_computer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let home = places::home();
        let (resp, _) = self.place_row(ui, ctx, &["user-home"], "Home", Some(Location::Dir(home.clone())), None, false);
        resp.context_menu(|ui| self.place_menu(ui, &home));
        let d = self.user_dirs.clone();
        for (p, icon, label) in [
            (&d.desktop, "user-desktop", "Desktop"),
            (&d.documents, "folder-documents", "Documents"),
            (&d.music, "folder-music", "Music"),
            (&d.pictures, "folder-pictures", "Pictures"),
            (&d.videos, "folder-videos", "Videos"),
            (&d.download, "folder-download", "Downloads"),
        ] {
            if let Some(p) = p {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| label.into());
                let (resp, _) = self.place_row(ui, ctx, &[icon, "folder"], &name, Some(Location::Dir(p.clone())), None, false);
                resp.context_menu(|ui| self.place_menu(ui, p));
            }
        }
        self.place_row(ui, ctx, &["document-open-recent", "folder-recent"], "Recent", Some(Location::Recent), None, false);
        let root_usage = places::disk_space(Path::new("/")).map(|(t, f)| if t > 0 { 1.0 - f as f32 / t as f32 } else { 0.0 });
        let (resp, _) = self.place_row(ui, ctx, &["drive-harddisk-system", "drive-harddisk"], "File System", Some(Location::Dir("/".into())), root_usage, false);
        resp.context_menu(|ui| self.place_menu(ui, Path::new("/")));
        let trash_icon: &[&str] = if self.trash_empty { &["user-trash"] } else { &["user-trash-full", "user-trash"] };
        let (resp, _) = self.place_row(ui, ctx, trash_icon, "Trash", Some(Location::Trash), None, false);
        resp.context_menu(|ui| {
            if ui.add_enabled(!self.trash_empty, egui::Button::new("Empty Trash")).clicked() {
                self.pending.push(Action::EmptyTrash);
            }
        });
    }

    fn other_sections(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.bookmarks.is_empty() && self.section_header(ui, "Bookmarks") {
            for (i, b) in self.bookmarks.clone().into_iter().enumerate() {
                let icon: Vec<&str> = if b.path.is_dir() {
                    match places::special_folder_icon(&b.path, &self.user_dirs) {
                        Some(s) => vec![s, "folder"],
                        None => vec!["folder"],
                    }
                } else {
                    vec!["folder-remote", "folder"]
                };
                let (resp, _) = self.place_row(ui, ctx, &icon, &b.label, Some(Location::Dir(b.path.clone())), None, false);
                resp.context_menu(|ui| {
                    self.place_menu(ui, &b.path);
                    ui.separator();
                    if ui.button("Rename…").clicked() {
                        self.pending.push(Action::RenameBookmark(i));
                    }
                    if ui.button("Remove").clicked() {
                        self.pending.push(Action::RemoveBookmark(i));
                    }
                });
            }
        }

        let devices: Vec<_> = self.mounts.iter().filter(|m| !m.network).cloned().collect();
        if !devices.is_empty() && self.section_header(ui, "Devices") {
            for m in devices {
                let usage = places::disk_space(&m.path).map(|(t, f)| if t > 0 { 1.0 - f as f32 / t as f32 } else { 0.0 });
                let icon: &[&str] = if m.removable { &["drive-removable-media", "media-removable", "drive-harddisk"] } else { &["drive-harddisk"] };
                let (resp, eject) = self.place_row(ui, ctx, icon, &m.label, Some(Location::Dir(m.path.clone())), usage, m.removable);
                if eject {
                    self.pending.push(Action::Unmount(m.clone()));
                }
                resp.context_menu(|ui| {
                    self.place_menu(ui, &m.path);
                    ui.separator();
                    if ui.button("Unmount").clicked() {
                        self.pending.push(Action::Unmount(m.clone()));
                    }
                });
            }
        }

        if self.section_header(ui, "Network") {
            let (resp, _) = self.place_row(ui, ctx, &["network-workgroup", "network-server", "folder-remote"], "Network", Some(Location::Network), None, false);
            resp.on_hover_text("Mounted network shares");
            for m in self.mounts.clone().into_iter().filter(|m| m.network) {
                let (resp, eject) = self.place_row(ui, ctx, &["folder-remote", "network-server"], &m.label, Some(Location::Dir(m.path.clone())), None, true);
                if eject {
                    self.pending.push(Action::Unmount(m.clone()));
                }
                resp.context_menu(|ui| self.place_menu(ui, &m.path));
            }
        }
    }

    /// Folder tree sidebar: Home and File System, expandable.
    fn tree_contents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.spacing_mut().item_spacing.y = 3.0;
        let current = self.current_dir();
        let mut rows: Vec<(PathBuf, usize)> = Vec::new();
        let mut stack: Vec<(PathBuf, usize)> = vec![(PathBuf::from("/"), 0), (places::home(), 0)];
        while let Some((p, depth)) = stack.pop() {
            rows.push((p.clone(), depth));
            if self.tree_expanded.contains(&p) {
                if !self.tree_children.contains_key(&p) {
                    let mut kids: Vec<PathBuf> = std::fs::read_dir(&p)
                        .map(|rd| {
                            rd.flatten()
                                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false) || e.path().is_dir())
                                .map(|e| e.path())
                                .collect()
                        })
                        .unwrap_or_default();
                    kids.sort_by(|a, b| model::natural_cmp(&a.file_name().unwrap_or_default().to_string_lossy(), &b.file_name().unwrap_or_default().to_string_lossy()));
                    self.tree_children.insert(p.clone(), kids);
                }
                let kids = self.tree_children.get(&p).cloned().unwrap_or_default();
                for k in kids.into_iter().rev() {
                    let hidden = k.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
                    if hidden && !self.s.show_hidden {
                        continue;
                    }
                    stack.push((k, depth + 1));
                }
            }
        }
        let accent = accent_color(&self.s);
        for (p, depth) in rows {
            let isz = self.s.sidebar_icon;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0 * crate::fonts::scale().max(1.0) + (isz - 16.0).max(0.0)), Sense::click());
            let active = current.as_deref() == Some(p.as_path());
            let v = ui.visuals().clone();
            if active {
                ui.painter().rect_filled(rect, R_MD, accent);
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, R_MD, v.widgets.hovered.weak_bg_fill);
            }
            let fg = if active { v.selection.stroke.color } else { v.text_color() };
            let x = rect.left() + 4.0 + depth as f32 * 14.0;
            let expanded = self.tree_expanded.contains(&p);
            let chev = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(14.0, rect.height()));
            let c = chev.center();
            let pts = if expanded {
                vec![c + Vec2::new(-4.0, -2.0), c + Vec2::new(4.0, -2.0), c + Vec2::new(0.0, 3.0)]
            } else {
                vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(-2.0, 4.0), c + Vec2::new(3.0, 0.0)]
            };
            ui.painter().add(egui::Shape::convex_polygon(pts, fg.gamma_multiply(0.8), Stroke::NONE));
            let icon: Vec<&str> = match places::special_folder_icon(&p, &self.user_dirs) {
                Some(s) => vec![s, "folder"],
                None => vec!["folder"],
            };
            let ir = Rect::from_min_size(Pos2::new(chev.right() + 2.0, rect.center().y - isz / 2.0), Vec2::new(isz, isz.max(1.0)));
            if active {
                if isz > 0.0 {
                    match self.accent_icon(ctx, &icon, isz) {
                        Some((t, true)) => paint_tex(ui.painter(), &t, ir, fg),
                        Some((t, false)) => paint_tex(ui.painter(), &t, ir, fg),
                        None => {}
                    }
                }
            } else if isz > 0.0 {
                if let Some(t) = self.icon(ctx, &icon, isz) {
                    paint_tex(ui.painter(), &t, ir, Color32::WHITE);
                }
            }
            let name = if p == Path::new("/") {
                "File System".to_string()
            } else if p == places::home() {
                "Home".to_string()
            } else {
                p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            };
            let mut job = egui::text::LayoutJob::simple_singleline(name, crate::fonts::ui_font(13.0), fg);
            job.wrap = egui::text::TextWrapping::truncate_at_width((rect.right() - ir.right() - 10.0).max(10.0));
            let g = ui.painter().layout_job(job);
            ui.painter().galley(Pos2::new(ir.right() + 6.0, rect.center().y - g.size().y / 2.0), g, fg);
            let on_chevron = resp.interact_pointer_pos().is_some_and(|pp| pp.x < chev.right());
            if resp.clicked() {
                if on_chevron {
                    if expanded {
                        self.tree_expanded.remove(&p);
                    } else {
                        self.tree_expanded.insert(p.clone());
                        self.tree_children.remove(&p);
                    }
                } else {
                    self.pending.push(Action::Navigate(Location::Dir(p.clone())));
                    self.tree_expanded.insert(p.clone());
                    self.tree_children.remove(&p);
                }
            }
            if resp.double_clicked() && !on_chevron && expanded {
                self.tree_expanded.remove(&p);
            }
            if let Some(pl) = resp.dnd_release_payload::<DragFiles>() {
                self.pending.push(Action::Transfer(pl.0.clone(), p.clone(), ctx.input(|i| i.modifiers.ctrl)));
            }
            if resp.dnd_hover_payload::<DragFiles>().is_some() {
                ui.painter().rect_stroke(rect, R_MD, Stroke::new(2.0, accent), StrokeKind::Inside);
            }
            resp.context_menu(|ui| self.place_menu(ui, &p));
        }
    }

    fn place_menu(&mut self, ui: &mut egui::Ui, p: &Path) {
        if ui.button("Open").clicked() {
            self.pending.push(Action::Navigate(Location::Dir(p.to_path_buf())));
        }
        if ui.button("Open in New Tab").clicked() {
            self.pending.push(Action::OpenInNewTab(p.to_path_buf()));
        }
        if ui.button("Open in Terminal").clicked() {
            self.pending.push(Action::Terminal(p.to_path_buf()));
        }
        if ui.button("Properties").clicked() {
            self.pending.push(Action::Properties(Some(p.to_path_buf())));
        }
    }
}
