//! Pop-up dialogs: rename, new item, delete confirmations, open with, properties, preferences.

use eframe::egui::{self, Color32, Id, Key, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::Path;

use super::{Action, Dialog, PrefsTab, PropsState, RivetApp, ACCENTS};
use crate::fsops::{self, OpKind};
use crate::desktop;
use crate::icons;
use crate::palette;
use crate::launch;
use crate::places;
use crate::settings::{ThemeChoice, ToolItem, ViewMode};

fn select_stem(ctx: &egui::Context, id: Id, text: &str, is_dir: bool) {
    let (stem, _) = fsops::split_name(text, is_dir);
    if let Some(mut st) = egui::TextEdit::load_state(ctx, id) {
        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(stem.chars().count()),
        )));
        st.store(ctx, id);
    }
}

/// An on/off switch (a pill with a sliding knob), in the accent color when on.
pub(super) fn switch(ui: &mut egui::Ui, on: &mut bool, enabled: bool) -> egui::Response {
    let size = Vec2::new(40.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let v = ui.visuals().clone();
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let off_fill = v.widgets.inactive.bg_fill;
    let mut track = crate::palette::mix(off_fill, v.selection.bg_fill, t);
    let mut knob = if t > 0.5 { v.selection.stroke.color } else { v.text_color() };
    if !enabled {
        track = track.gamma_multiply(0.5);
        knob = knob.gamma_multiply(0.5);
    }
    ui.painter().rect_filled(rect, rect.height() / 2.0, track);
    let r = rect.height() / 2.0 - 3.0;
    let x = egui::lerp((rect.left() + r + 3.0)..=(rect.right() - r - 3.0), t);
    ui.painter().circle_filled(egui::Pos2::new(x, rect.center().y), r, knob);
    if resp.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A label with a drawn tick (the symbol isn't in every system font).
fn check_label(ui: &mut egui::Ui, text: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
    let c = r.center();
    let col = ui.visuals().selection.bg_fill;
    ui.painter().line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.5, 3.5)], Stroke::new(2.0, col));
    ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(5.5, -4.0)], Stroke::new(2.0, col));
    ui.label(text);
}

/// A small flat button with a drawn up / down chevron (looks the same in every icon theme).
fn arrow_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> egui::Response {
    let resp = ui.add_enabled(enabled, egui::Button::new("").frame_when_inactive(false).min_size(Vec2::new(24.0, 22.0)));
    let c = resp.rect.center();
    let col = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color().gamma_multiply(0.5) };
    let (dy, tip) = if up { (-2.5, 2.5) } else { (2.5, -2.5) };
    let stroke = Stroke::new(1.8, col);
    ui.painter().line_segment([c + Vec2::new(-4.5, tip), c + Vec2::new(0.0, dy)], stroke);
    ui.painter().line_segment([c + Vec2::new(0.0, dy), c + Vec2::new(4.5, tip)], stroke);
    resp
}

/// A small flat button with a drawn trash can (red on hover).
fn trash_button(ui: &mut egui::Ui) -> egui::Response {
    let resp = ui.add(egui::Button::new("").frame_when_inactive(false).min_size(Vec2::new(24.0, 22.0)));
    let c = resp.rect.center();
    let col = if resp.hovered() { super::status(ui.ctx()).danger } else { ui.visuals().text_color() };
    let s = Stroke::new(1.5, col);
    let p = ui.painter();
    // lid and handle
    p.line_segment([c + Vec2::new(-5.5, -4.0), c + Vec2::new(5.5, -4.0)], s);
    p.line_segment([c + Vec2::new(-2.0, -4.0), c + Vec2::new(-2.0, -6.0)], s);
    p.line_segment([c + Vec2::new(-2.0, -6.0), c + Vec2::new(2.0, -6.0)], s);
    p.line_segment([c + Vec2::new(2.0, -6.0), c + Vec2::new(2.0, -4.0)], s);
    // can body
    p.add(egui::Shape::closed_line(
        vec![c + Vec2::new(-4.2, -2.0), c + Vec2::new(4.2, -2.0), c + Vec2::new(3.4, 6.0), c + Vec2::new(-3.4, 6.0)],
        s,
    ));
    p.line_segment([c + Vec2::new(-1.3, 0.0), c + Vec2::new(-1.1, 4.0)], Stroke::new(1.1, col));
    p.line_segment([c + Vec2::new(1.3, 0.0), c + Vec2::new(1.1, 4.0)], Stroke::new(1.1, col));
    resp
}

fn danger_button(ui: &mut egui::Ui, label: &str) -> bool {
    let d = super::status(ui.ctx()).danger;
    ui.add(egui::Button::new(egui::RichText::new(label).color(super::StatusColors::on(d))).fill(d)).clicked()
}

/// A font family list you can type into: Enter picks the closest match. `system_label` is
/// the "use the system's" entry (an empty `value`). Returns whether the choice changed.
fn font_picker(ui: &mut egui::Ui, id: &str, value: &mut String, families: &[String], filter: &mut String, system_label: &str) -> bool {
    let mut changed = false;
    let current = if value.is_empty() { system_label.to_string() } else { value.clone() };
    let opened = egui::ComboBox::from_id_salt(id).selected_text(current).width(260.0).height(320.0).show_ui(ui, |ui| {
        let field_bg = crate::palette::mix(ui.visuals().window_fill, ui.visuals().text_color(), 0.06);
        let search = egui::Frame::NONE
            .fill(field_bg)
            .corner_radius(super::r_sm())
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| ui.add(egui::TextEdit::singleline(filter).frame(egui::Frame::NONE).hint_text("Type a font name…").desired_width(f32::INFINITY)))
            .inner;
        search.request_focus();
        let pick_best = ui.input(|i| i.key_pressed(Key::Enter));
        ui.add_space(4.0);
        let typed = filter.trim().to_string();
        let mut matches: Vec<(i32, &String)> = families.iter().filter_map(|f| super::chrome::fuzzy_score(f, &typed).map(|s| (s, f))).collect();
        if !typed.is_empty() {
            matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        }
        let show_system = typed.is_empty() || super::chrome::fuzzy_score(system_label, &typed).is_some();
        if pick_best && !typed.is_empty() {
            if let Some((_, best)) = matches.first() {
                *value = (*best).clone();
                changed = true;
                ui.close();
            }
        }
        if show_system && ui.selectable_label(value.is_empty(), system_label).clicked() {
            value.clear();
            changed = true;
        }
        if show_system && !matches.is_empty() {
            ui.separator();
        }
        if matches.is_empty() && !show_system {
            ui.label(egui::RichText::new("No matching font").color(ui.visuals().weak_text_color()));
        }
        for (_, fam) in matches {
            if ui.selectable_label(*value == *fam, fam.as_str()).clicked() {
                *value = fam.clone();
                changed = true;
            }
        }
    });
    // Start fresh the next time the list opens.
    if opened.inner.is_none() {
        filter.clear();
    }
    changed
}

/// A filled button in the theme-tinted success color (e.g. Rename).
fn success_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let c = super::status(ui.ctx()).success;
    ui.add(egui::Button::new(egui::RichText::new(label).color(super::StatusColors::on(c))).fill(c))
}

impl RivetApp {
    /// Compress…: a name, a format (with a one-line hint each), and where it goes.
    /// Returns true when the dialog should close.
    fn compress_dialog(&mut self, ctx: &egui::Context) -> bool {
        use crate::archive::Format;
        let Dialog::Compress { paths, name, format, focus } = &mut self.dialog else { return true };
        let mut ok = false;
        let mut cancel = false;
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(super::r_lg());
        let r = egui::Modal::new(Id::unique("compress")).frame(frame).show(ctx, |ui| {
            ui.set_width(420.0);
            let v = ui.visuals().clone();
            ui.label(egui::RichText::new("Compress").heading());
            ui.add_space(2.0);
            let what = match paths.len() {
                1 => format!("“{}”", paths[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                n => format!("{n} items"),
            };
            ui.label(egui::RichText::new(format!("Make an archive of {what}")).color(v.weak_text_color()));
            ui.add_space(14.0);
            ui.label(egui::RichText::new("Archive name").small().color(v.weak_text_color()));
            let field_bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
            egui::Frame::NONE.fill(field_bg).corner_radius(super::r_md()).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let ext_w = ui.painter().layout_no_wrap(format.ext().into(), egui::TextStyle::Body.resolve(ui.style()), Color32::WHITE).size().x;
                    let resp = ui.add(egui::TextEdit::singleline(name).frame(egui::Frame::NONE).desired_width(ui.available_width() - ext_w - 8.0));
                    if *focus {
                        resp.request_focus();
                        *focus = false;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        ok = true;
                    }
                    ui.label(egui::RichText::new(format.ext()).color(v.weak_text_color()));
                });
            });
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Format").small().color(v.weak_text_color()));
            ui.add_space(2.0);
            for f in Format::ALL {
                let avail = f.available();
                let chosen = *format == f;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), if avail { Sense::click() } else { Sense::hover() });
                if chosen {
                    let (fill, stroke) = super::selected_tile(&v, v.window_fill);
                    ui.painter().rect(rect, super::r_sm(), fill, stroke, egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, super::r_sm(), v.widgets.hovered.weak_bg_fill);
                }
                let fg = if avail { v.text_color() } else { v.weak_text_color().gamma_multiply(0.6) };
                let sub = v.weak_text_color();
                let dot = if chosen { v.selection.bg_fill } else { fg };
                // radio dot
                let c = Pos2::new(rect.left() + 18.0, rect.center().y);
                ui.painter().circle_stroke(c, 6.5, Stroke::new(1.5, dot));
                if chosen {
                    ui.painter().circle_filled(c, 3.5, dot);
                }
                let x = rect.left() + 36.0;
                ui.painter().text(Pos2::new(x, rect.top() + 6.0), egui::Align2::LEFT_TOP, f.ext().trim_start_matches('.'), egui::FontId::proportional(13.5), fg);
                let hint = if avail { f.hint().to_string() } else { "Not installed on this system".into() };
                ui.painter().text(Pos2::new(x, rect.top() + 22.0), egui::Align2::LEFT_TOP, hint, egui::FontId::proportional(11.5), sub);
                if resp.clicked() {
                    *format = f;
                }
            }
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().button_padding = Vec2::new(14.0, 5.0);
                let create = egui::Button::new(egui::RichText::new("Create").color(v.selection.stroke.color)).fill(v.selection.bg_fill);
                if ui.add_enabled(!name.trim().is_empty() && format.available(), create).clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if cancel || r.should_close() {
            return true;
        }
        if !ok {
            return false;
        }
        let name = name.trim().to_string();
        if name.contains('/') {
            self.dialog = Dialog::Message("Invalid name".into(), "Names cannot contain “/”.".into());
            return false;
        }
        let (paths, format) = (paths.clone(), *format);
        let Some(dir) = paths.first().and_then(|p| p.parent()).map(|d| d.to_path_buf()) else { return true };
        let archive = crate::fsops::unique_target(&dir, &dir.join(format!("{name}{}", format.ext())));
        self.s.compress_format = format.ext().trim_start_matches('.').to_string();
        self.dirty_settings = true;
        self.ops.push(crate::archive::spawn_compress(paths, archive, format));
        true
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let mut next: Option<Dialog> = None;
        let dir = self.current_dir();
        match &mut self.dialog {
            Dialog::None => {}
            Dialog::Rename { path, text, focus } => {
                let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(super::r_lg());
                let r = egui::Modal::new(Id::unique("rename")).frame(frame).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("Rename");
                    ui.add_space(12.0);
                    let v = ui.visuals().clone();
                    let field_bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
                    let resp = egui::Frame::NONE
                        .fill(field_bg)
                        .corner_radius(super::r_md())
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .show(ui, |ui| ui.add(egui::TextEdit::singleline(text).frame(egui::Frame::NONE).desired_width(f32::INFINITY)))
                        .inner;
                    if *focus {
                        resp.request_focus();
                        select_stem(ctx, resp.id, text, path.is_dir());
                        *focus = false;
                    }
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.add_space(18.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.spacing_mut().button_padding = Vec2::new(14.0, 5.0);
                        let ok = success_button(ui, "Rename").clicked() || enter;
                        let cancel = ui.button("Cancel").clicked();
                        (ok, cancel)
                    })
                    .inner
                });
                let (ok, cancel) = r.inner;
                if ok {
                    let new = text.trim();
                    if new.is_empty() || new.contains('/') {
                        next = Some(Dialog::Message("Invalid name".into(), "Names cannot be empty or contain “/”.".into()));
                    } else {
                        let target = path.with_file_name(new);
                        if target != *path {
                            if target.exists() {
                                next = Some(Dialog::Message("Name already used".into(), format!("“{new}” already exists in this folder.")));
                            } else if let Err(e) = std::fs::rename(&*path, &target) {
                                next = Some(Dialog::Message("Could not rename".into(), e.to_string()));
                            } else {
                                self.history.record(crate::undo::Step::Renamed(path.clone(), target.clone()));
                                let t = &mut self.tabs[self.active];
                                t.reload(&self.mime, &self.s);
                                t.selected.clear();
                                t.selected.insert(target.clone());
                                t.scroll_to = Some(target);
                            }
                        }
                        close = true;
                    }
                }
                if cancel || r.should_close() {
                    close = true;
                }
            }
            Dialog::NewItem { folder, text, focus, template } => {
                let r = egui::Modal::new(Id::unique("newitem")).show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.heading(if *folder { "New Folder" } else { "New Document" });
                    ui.add_space(6.0);
                    let resp = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                    if *focus {
                        resp.request_focus();
                        select_stem(ctx, resp.id, text, *folder);
                        *focus = false;
                    }
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| (ui.button("Create").clicked() || enter, ui.button("Cancel").clicked())).inner
                });
                let (ok, cancel) = r.inner;
                if ok {
                    if let Some(d) = &dir {
                        let name = text.trim();
                        let p = d.join(name);
                        let res = if name.is_empty() || name.contains('/') {
                            Err(std::io::Error::other("Names cannot be empty or contain “/”."))
                        } else if p.exists() {
                            Err(std::io::Error::other(format!("“{name}” already exists.")))
                        } else if *folder {
                            std::fs::create_dir(&p)
                        } else if let Some(t) = template {
                            std::fs::copy(t, &p).map(|_| ())
                        } else {
                            std::fs::File::create(&p).map(|_| ())
                        };
                        match res {
                            Ok(()) => {
                                self.history.record(crate::undo::Step::Created(p.clone()));
                                let t = &mut self.tabs[self.active];
                                t.reload(&self.mime, &self.s);
                                t.selected.clear();
                                t.selected.insert(p.clone());
                                t.scroll_to = Some(p);
                            }
                            Err(e) => next = Some(Dialog::Message("Could not create".into(), e.to_string())),
                        }
                    }
                    close = true;
                }
                if cancel || r.should_close() {
                    close = true;
                }
            }
            Dialog::ConfirmDelete(_) => {
                close = self.delete_dialog(ctx);
            }
            Dialog::Compress { .. } => {
                close = self.compress_dialog(ctx);
            }
            Dialog::BulkRename(_) => {
                close = self.bulk_rename_dialog(ctx);
            }
            Dialog::Convert(_) => {
                close = self.convert_dialog(ctx);
            }
            Dialog::Resize(_) => {
                close = self.resize_dialog(ctx);
            }
            Dialog::ConfirmEmptyTrash => {
                let r = egui::Modal::new(Id::unique("emptytrash")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("Empty all items from Trash?");
                    ui.add_space(4.0);
                    ui.label("All items in the Trash will be permanently deleted.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| (danger_button(ui, "Empty Trash"), ui.button("Cancel").clicked())).inner
                });
                if r.inner.0 {
                    self.ops.push(fsops::spawn_op(OpKind::EmptyTrash, vec![], None));
                    close = true;
                }
                if r.inner.1 || r.should_close() {
                    close = true;
                }
            }
            Dialog::Message(title, body) => {
                let r = egui::Modal::new(Id::unique("msg")).show(ctx, |ui| {
                    ui.set_max_width(440.0);
                    ui.heading(title.as_str());
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| ui.label(body.as_str()));
                    ui.add_space(8.0);
                    ui.button("OK").clicked()
                });
                if r.inner || r.should_close() {
                    close = true;
                }
            }
            Dialog::RenameBookmark { idx, text } => {
                let r = egui::Modal::new(Id::unique("renamebm")).show(ctx, |ui| {
                    ui.set_width(320.0);
                    ui.heading("Rename Bookmark");
                    let resp = ui.text_edit_singleline(text);
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.horizontal(|ui| (ui.button("Rename").clicked() || enter, ui.button("Cancel").clicked())).inner
                });
                if r.inner.0 {
                    if let Some(b) = self.bookmarks.get_mut(*idx) {
                        b.label = text.trim().to_string();
                        let _ = places::save_bookmarks(&self.bookmarks);
                    }
                    close = true;
                }
                if r.inner.1 || r.should_close() {
                    close = true;
                }
            }
            Dialog::OpenWith { .. } => {
                close = self.open_with_dialog(ctx);
            }
            Dialog::Shortcuts => {
                const GROUPS: [(&str, &[(&str, &str)]); 5] = [
                    ("Navigation", &[
                        ("Alt+Left / Alt+Right", "Back / forward"),
                        ("Alt+Up or Backspace", "Parent folder"),
                        ("Alt+Home", "Home folder"),
                        ("Ctrl+L", "Type a location"),
                        ("Ctrl+F or just type", "Filter; Enter searches subfolders"),
                        ("Ctrl+T / Ctrl+W", "New tab / close tab"),
                        ("Ctrl+Tab", "Next tab"),
                    ]),
                    ("Files", &[
                        ("Enter", "Open"),
                        ("Ctrl+C / X / V", "Copy / cut / paste"),
                        ("Ctrl+Z / Ctrl+Shift+Z", "Undo / redo"),
                        ("F2", "Rename"),
                        ("Delete", "Move to Trash or delete (asks)"),
                        ("Shift+Delete", "Delete permanently"),
                        ("Ctrl+Shift+N", "New folder"),
                        ("Ctrl+A", "Select all"),
                        ("Ctrl+I or Alt+Enter", "Properties"),
                        ("Ctrl+D", "Bookmark this folder"),
                    ]),
                    ("View", &[
                        ("Ctrl+1 / 2 / 3", "Icon / list / compact view"),
                        ("Ctrl+ + / Ctrl+ −", "Zoom in / out"),
                        ("Ctrl+H", "Show hidden files"),
                        ("F5 or Ctrl+R", "Reload"),
                    ]),
                    ("Window", &[
                        ("Ctrl+,", "Preferences"),
                        ("F8", "Show or hide the status bar"),
                        ("F9", "Show or hide the sidebar"),
                        ("F11", "Show or hide the Peek panel"),
                        ("F12", "Zen mode"),
                        ("F7", "Terminal mode"),
                        ("Ctrl+P", "Quick Open: go to any folder"),
                        ("Ctrl+Shift+P", "Quick Actions: every command and option"),
                        ("Ctrl+Shift+F", "Filters"),
                        ("Ctrl+M or F10", "Show or hide the menu bar"),
                        ("F1", "This list"),
                        ("Ctrl+Q", "Close the window"),
                    ]),
                    ("Vim keys (Preferences → Behavior)", &[
                        ("j / k", "Move down / up"),
                        ("h / l", "Parent folder / open"),
                        ("gg / G", "First / last item"),
                        ("H / L", "Back / forward"),
                        ("yy / dd / p", "Copy / cut / paste"),
                        ("/", "Search this folder"),
                        (":", "Quick Actions"),
                        ("~", "Home"),
                    ]),
                ];
                let r = egui::Modal::new(Id::unique("shortcuts")).show(ctx, |ui| {
                    ui.set_width(460.0);
                    ui.heading("Keyboard Shortcuts");
                    ui.add_space(4.0);
                    let weak = ui.visuals().weak_text_color();
                    egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 180.0).max(240.0)).show(ui, |ui| {
                        for (gi, (title, rows)) in GROUPS.iter().enumerate() {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(*title).strong().color(weak));
                            egui::Grid::new(("keys", gi)).num_columns(2).spacing([24.0, 6.0]).min_col_width(170.0).show(ui, |ui| {
                                for (k, d) in rows.iter() {
                                    ui.label(egui::RichText::new(*k).monospace());
                                    ui.label(*d);
                                    ui.end_row();
                                }
                            });
                        }
                    });
                    ui.add_space(10.0);
                    ui.button("Close").clicked()
                });
                if r.inner || r.should_close() {
                    close = true;
                }
            }
        }
        if let Some(n) = next {
            self.dialog = n;
        } else if close {
            self.dialog = Dialog::None;
        }
    }

    /// The delete confirmation: what's affected, how big it is, and a safe default.
    fn delete_dialog(&mut self, ctx: &egui::Context) -> bool {
        // Icons first (needs &mut self), then the dialog itself.
        let preview: Vec<(String, Option<egui::TextureHandle>)> = {
            let Dialog::ConfirmDelete(d) = &self.dialog else { return true };
            d.paths.iter().take(5).cloned().collect::<Vec<_>>()
        }
        .into_iter()
        .map(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string());
            let tex = crate::model::Entry::from_path(p, &self.mime).and_then(|e| self.entry_texture(ctx, &e, 48.0));
            (name, tex)
        })
        .collect();
        let Dialog::ConfirmDelete(d) = &self.dialog else { return true };
        let n = d.paths.len();
        let title = if n == 1 { format!("Delete “{}”?", preview[0].0) } else { format!("Delete {n} items?") };
        let mut parts = Vec::new();
        if d.folders > 0 {
            parts.push(format!("{} folder{}", d.folders, if d.folders == 1 { "" } else { "s" }));
        }
        if d.files > 0 {
            parts.push(format!("{} file{}", d.files, if d.files == 1 { "" } else { "s" }));
        }
        let size = format!("{}{}", if d.partial { "at least " } else { "" }, fsops::human_size(d.bytes));
        let summary = format!("{} · {size}", parts.join(", "));
        let (allow_trash, permanent_default) = (d.allow_trash, d.permanent_default);

        let mut choice: Option<bool> = None; // Some(true) = permanently, Some(false) = trash
        // A roomier box than the default dialog frame.
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(super::r_lg());
        let r = egui::Modal::new(Id::unique("confirmdel")).frame(frame).show(ctx, |ui| {
            ui.set_width(440.0);
            let v = ui.visuals().clone();
            ui.horizontal(|ui| {
                if let Some(Some(t)) = preview.first().map(|p| p.1.clone()) {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                    super::paint_tex(ui.painter(), &t, r.shrink(2.0), Color32::WHITE);
                    if n > 1 {
                        // A count badge on the icon.
                        let c = r.right_bottom() - Vec2::splat(6.0);
                        ui.painter().circle_filled(c, 10.0, v.selection.bg_fill);
                        ui.painter().text(c, egui::Align2::CENTER_CENTER, n.min(99).to_string(), crate::fonts::ui_font(11.0), v.selection.stroke.color);
                    }
                    ui.add_space(8.0);
                }
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&title).heading()).truncate());
                    ui.label(egui::RichText::new(&summary).color(v.weak_text_color()));
                });
            });
            if n > 1 {
                ui.add_space(8.0);
                egui::Frame::NONE.fill(v.faint_bg_color).corner_radius(super::r_sm()).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (name, tex) in &preview {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                            if let Some(t) = tex {
                                super::paint_tex(ui.painter(), t, r, Color32::WHITE);
                            }
                            ui.add(egui::Label::new(name.as_str()).truncate());
                        });
                    }
                    if n > preview.len() {
                        ui.label(egui::RichText::new(format!("…and {} more", n - preview.len())).color(v.weak_text_color()));
                    }
                });
            }
            ui.add_space(10.0);
            let danger = super::status(ctx).danger;
            let warn = danger;
            if allow_trash {
                ui.label(egui::RichText::new("Move to Trash keeps them recoverable. Deleting permanently can’t be undone.").color(v.weak_text_color()));
            } else {
                ui.label(egui::RichText::new("These will be deleted permanently. This can’t be undone.").color(warn));
            }
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().button_padding = Vec2::new(12.0, 5.0);
                // Rightmost = the default (Enter).
                let red = egui::Button::new(egui::RichText::new("Delete Permanently").color(if permanent_default { super::StatusColors::on(danger) } else { danger }));
                let red = if permanent_default { red.fill(danger) } else { red.stroke(Stroke::new(1.0, danger.gamma_multiply(0.6))) };
                let trash = egui::Button::new(egui::RichText::new("Move to Trash").color(if permanent_default { v.text_color() } else { v.selection.stroke.color }));
                let trash = if permanent_default { trash } else { trash.fill(v.selection.bg_fill) };
                let mut clicked_cancel = false;
                if permanent_default {
                    if ui.add(red).clicked() {
                        choice = Some(true);
                    }
                    if allow_trash && ui.add(trash).clicked() {
                        choice = Some(false);
                    }
                } else {
                    if ui.add(trash).clicked() {
                        choice = Some(false);
                    }
                    if ui.add(red).clicked() {
                        choice = Some(true);
                    }
                }
                if ui.button("Cancel").clicked() {
                    clicked_cancel = true;
                }
                clicked_cancel
            })
            .inner
        });
        if choice.is_none() && ctx.input(|i| i.key_pressed(Key::Enter)) {
            choice = Some(permanent_default);
        }
        let paths = match &self.dialog {
            Dialog::ConfirmDelete(d) => d.paths.clone(),
            _ => return true,
        };
        match choice {
            Some(true) => {
                self.ops.push(fsops::spawn_op(OpKind::Delete, paths, None));
                true
            }
            Some(false) => {
                self.ops.push(fsops::spawn_op(OpKind::Trash, paths, None));
                true
            }
            None => r.inner || r.should_close(),
        }
    }

    fn open_with_dialog(&mut self, ctx: &egui::Context) -> bool {
        // Gather everything first (needs &mut self for icons), then draw.
        let (paths, mime, show_all, filter) = match &self.dialog {
            Dialog::OpenWith { paths, mime, show_all, filter, .. } => (paths.clone(), mime.clone(), *show_all, filter.to_lowercase()),
            _ => return true,
        };
        let desc = self.mime.describe(&mime);
        let default_id = launch::default_app(&self.apps, &mime).map(|a| a.id.clone());
        let mut recommended: Vec<usize> = launch::apps_for(&self.apps, &mime)
            .into_iter()
            .filter_map(|a| self.apps.iter().position(|x| x.id == a.id))
            .collect();
        // The current default app is always offered, even when it doesn't list this type.
        if let Some(d) = default_id.as_ref().and_then(|id| self.apps.iter().position(|x| &x.id == id)) {
            if !recommended.contains(&d) {
                recommended.insert(0, d);
            }
        }
        let matches = |i: &usize| filter.is_empty() || self.apps[*i].name.to_lowercase().contains(&filter);
        let mut rec: Vec<usize> = recommended.iter().copied().filter(matches).collect();
        // The current default goes first.
        rec.sort_by_key(|i| Some(&self.apps[*i].id) != default_id.as_ref());
        let others: Vec<usize> = if show_all || rec.is_empty() && !filter.is_empty() {
            (0..self.apps.len()).filter(|i| !recommended.contains(i)).filter(matches).take(300).collect()
        } else {
            Vec::new()
        };
        let ppp = ctx.pixels_per_point();
        let px = (28.0 * ppp) as u32;
        let mut icons: std::collections::HashMap<usize, Option<egui::TextureHandle>> = Default::default();
        for i in rec.iter().chain(others.iter()) {
            let icon = self.apps[*i].icon.clone();
            let t = if icon.starts_with('/') {
                self.icons.texture_from_path(ctx, Path::new(&icon), px)
            } else {
                self.icons.texture(ctx, &[icon.as_str(), "application-x-executable"], px)
            };
            icons.insert(*i, t);
        }
        let file_icon = paths.first().and_then(|p| crate::model::Entry::from_path(p.clone(), &self.mime)).and_then(|e| self.entry_texture(ctx, &e, 48.0));
        let search_icon = self.icon(ctx, &["edit-find-symbolic", "system-search-symbolic"], 16.0);
        let names: Vec<(usize, String, bool)> = rec
            .iter()
            .chain(others.iter())
            .map(|i| (*i, self.apps[*i].name.clone(), Some(&self.apps[*i].id) == default_id.as_ref()))
            .collect();
        let n_rec = rec.len();

        let Dialog::OpenWith { show_all, filter, chosen, remember, focus, .. } = &mut self.dialog else { return true };
        let mut launch_now = false;
        let mut cancel = false;
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(super::r_lg());
        let r = egui::Modal::new(Id::unique("openwith")).frame(frame).show(ctx, |ui| {
            ui.set_width(460.0);
            let v = ui.visuals().clone();
            // ---- header: the file and its type
            ui.horizontal(|ui| {
                if let Some(t) = &file_icon {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                    super::paint_tex(ui.painter(), t, r, Color32::WHITE);
                    ui.add_space(8.0);
                }
                ui.vertical(|ui| {
                    let what = if paths.len() == 1 {
                        format!("Open “{}”", paths[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
                    } else {
                        format!("Open {} files", paths.len())
                    };
                    ui.add(egui::Label::new(egui::RichText::new(what).heading()).truncate());
                    ui.label(egui::RichText::new(&desc).color(v.weak_text_color()));
                });
            });
            ui.add_space(12.0);
            // ---- search
            let field = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
            egui::Frame::NONE.fill(field).corner_radius(super::r_md()).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(t) = &search_icon {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(18.0, 24.0), Sense::hover());
                        super::paint_tex(ui.painter(), t, Rect::from_center_size(r.center(), Vec2::splat(15.0)), v.weak_text_color());
                    }
                    let resp = ui.add(egui::TextEdit::singleline(filter).frame(egui::Frame::NONE).hint_text("Search applications").desired_width(f32::INFINITY));
                    if *focus {
                        resp.request_focus();
                        *focus = false;
                    }
                });
            });
            ui.add_space(10.0);
            // ---- list
            egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if names.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| ui.label(egui::RichText::new("No matching applications").color(v.weak_text_color())));
                    ui.add_space(20.0);
                }
                for (row, (i, name, is_default)) in names.iter().enumerate() {
                    if row == 0 && n_rec > 0 {
                        ui.label(egui::RichText::new("Recommended").small().strong().color(v.weak_text_color()));
                    }
                    if row == n_rec {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Other applications").small().strong().color(v.weak_text_color()));
                    }
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                    let selected = *chosen == Some(*i);
                    if selected {
                        let (fill, stroke) = super::selected_tile(&v, v.window_fill);
                        ui.painter().rect_filled(rect, super::r_sm(), fill);
                        ui.painter().rect_stroke(rect, super::r_sm(), stroke, egui::StrokeKind::Inside);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, super::r_sm(), v.widgets.hovered.weak_bg_fill);
                    }
                    let ir = Rect::from_center_size(Pos2::new(rect.left() + 24.0, rect.center().y), Vec2::splat(28.0));
                    if let Some(Some(t)) = icons.get(i) {
                        super::paint_tex(ui.painter(), t, ir, Color32::WHITE);
                    }
                    let galley = ui.painter().layout_no_wrap(name.clone(), crate::fonts::ui_font(13.5), v.text_color());
                    ui.painter().galley(Pos2::new(ir.right() + 12.0, rect.center().y - galley.size().y / 2.0), galley, v.text_color());
                    if *is_default {
                        let tag = ui.painter().layout_no_wrap("Default".into(), crate::fonts::ui_font(11.0), v.weak_text_color());
                        let tr = Rect::from_min_size(Pos2::new(rect.right() - tag.size().x - 22.0, rect.center().y - 9.0), Vec2::new(tag.size().x + 14.0, 18.0));
                        ui.painter().rect_filled(tr, 9.0, crate::palette::mix(v.window_fill, v.text_color(), 0.08));
                        ui.painter().galley(Pos2::new(tr.left() + 7.0, tr.center().y - tag.size().y / 2.0), tag, v.weak_text_color());
                    }
                    if resp.clicked() {
                        *chosen = Some(*i);
                    }
                    if resp.double_clicked() {
                        *chosen = Some(*i);
                        launch_now = true;
                    }
                }
            });
            ui.add_space(12.0);
            // ---- footer
            ui.horizontal(|ui| {
                ui.checkbox(remember, "Always use for this file type");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.spacing_mut().button_padding = Vec2::new(14.0, 5.0);
                    let open = egui::Button::new(egui::RichText::new("Open").color(v.selection.stroke.color)).fill(v.selection.bg_fill);
                    if ui.add_enabled(chosen.is_some(), open).clicked() {
                        launch_now = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
            ui.add_space(2.0);
            let label = if *show_all { "Show recommended only" } else { "Show all applications" };
            if ui.add(egui::Label::new(egui::RichText::new(label).small().color(v.hyperlink_color)).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                *show_all = !*show_all;
            }
        });
        if ctx.input(|i| i.key_pressed(Key::Enter)) && chosen.is_some() {
            launch_now = true;
        }
        if launch_now {
            if let Some(i) = *chosen {
                if *remember {
                    let id = self.apps[i].id.clone();
                    match launch::set_default_app(&id, &mime) {
                        Ok(()) => self.flash(&format!("{} is now the default for {desc}", self.apps[i].name)),
                        Err(e) => self.flash(&e),
                    }
                }
                self.pending.push(Action::Launch(i, paths));
                return true;
            }
        }
        cancel || r.should_close()
    }

    /// All open Properties windows.
    pub(super) fn properties_windows(&mut self, ctx: &egui::Context) {
        let mut list = std::mem::take(&mut self.props);
        list.retain_mut(|(win, st)| self.properties_window(ctx, win, st));
        // Windows opened meanwhile (none normally) go after the existing ones.
        list.append(&mut self.props);
        self.props = list;
    }

    /// One Properties window; returns false once it is closed.
    fn properties_window(&mut self, ctx: &egui::Context, win: &mut super::windows::WinState, st: &mut PropsState) -> bool {
        let tex = {
            st.entry.clone().and_then(|e| {
                let names = if e.is_dir {
                    let s = places::special_folder_icon(&e.path, &self.user_dirs).unwrap_or("folder");
                    vec![s.to_string(), "folder".into()]
                } else {
                    self.mime.icon_names(&e.mime)
                };
                let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                let ppp = ctx.pixels_per_point();
                self.icons.texture(ctx, &refs, (64.0 * ppp) as u32)
            })
        };
        let desc = st.entry.as_ref().map(|e| self.mime.describe(&e.mime)).unwrap_or_default();
        let mut apply_mode: Option<u32> = None;
        let mut rename: Option<String> = None;
        // Just "Properties": the name is right below in the window, and long names
        // would crowd the title bar.
        let title = "Properties".to_string();
        let mut chrome = self.window_chrome(&title, &["document-properties-symbolic", "dialog-information-symbolic"], ctx);
        if let Some(t) = &tex {
            chrome.icon = Some(t.clone());
        }
        let area = self.window_area(ctx);
        let created = std::fs::metadata(&st.path).ok().and_then(|m| m.created().ok());
        let accessed = std::fs::metadata(&st.path).ok().and_then(|m| m.accessed().ok());
        let shown = super::windows::show(ctx, win, &chrome, area, Vec2::new(420.0, 300.0), |ui| {
            let v = ui.visuals().clone();
            let weak = v.weak_text_color();
            let card_fill = v.faint_bg_color;
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
            ui.add_space(6.0);
            // ---- header: icon, editable name, one-line summary
            ui.horizontal(|ui| {
                if let Some(t) = &tex {
                    ui.add(egui::Image::from_texture(t).fit_to_exact_size(Vec2::splat(56.0)));
                }
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let field_bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
                    let r = egui::Frame::NONE
                        .fill(field_bg)
                        .corner_radius(super::r_md())
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.add(egui::TextEdit::singleline(&mut st.name).frame(egui::Frame::NONE).font(egui::FontId::proportional(15.0)).desired_width(f32::INFINITY))
                        })
                        .inner;
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        rename = Some(st.name.clone());
                    }
                    let summary = match &st.entry {
                        Some(e) if e.is_dir => match st.size {
                            Some((bytes, count, _)) => format!("{desc} · {count} item{} · {}", if count == 1 { "" } else { "s" }, fsops::human_size(bytes)),
                            None => desc.clone(),
                        },
                        Some(e) => format!("{desc} · {}", fsops::human_size(e.size)),
                        None => desc.clone(),
                    };
                    ui.label(egui::RichText::new(summary).color(weak));
                });
            });
            ui.add_space(14.0);
            // ---- segmented Basic / Permissions switch
            let (track, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
            ui.painter().rect_filled(track, super::r_md(), card_fill);
            let half = track.width() / 2.0;
            for (i, label) in ["Basic", "Permissions"].into_iter().enumerate() {
                let r = Rect::from_min_size(track.min + Vec2::new(half * i as f32, 0.0), Vec2::new(half, track.height())).shrink(3.0);
                let resp = ui.interact(r, ui.scope_id().with(("props-tab", i)), Sense::click());
                if st.tab == i as u8 {
                    let (fill, stroke) = super::selected_tile(&v, card_fill);
                    ui.painter().rect_filled(r, super::r_sm(), fill);
                    ui.painter().rect_stroke(r, super::r_sm(), stroke, egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, super::r_sm(), v.widgets.hovered.weak_bg_fill);
                }
                let fg = if st.tab == i as u8 { v.strong_text_color() } else { v.text_color() };
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, crate::fonts::ui_font(13.5), fg);
                if resp.clicked() {
                    st.tab = i as u8;
                }
            }
            ui.add_space(14.0);
            // A titled card of rows.
            let card = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
                ui.label(egui::RichText::new(title.to_uppercase()).size(10.5).strong().color(weak));
                egui::Frame::NONE.fill(card_fill).corner_radius(super::r_md()).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    body(ui);
                });
                ui.add_space(12.0);
            };
            let rows = |ui: &mut egui::Ui, id: &str, rows: Vec<(&str, String, Option<String>)>| {
                egui::Grid::new(id).num_columns(2).spacing([18.0, 10.0]).min_col_width(92.0).show(ui, |ui| {
                    for (k, val, sub) in rows {
                        // Label and value both top-aligned, so two-line values line up.
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| ui.label(egui::RichText::new(k).color(weak)));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.add(egui::Label::new(val).wrap());
                            if let Some(sub) = sub {
                                ui.label(egui::RichText::new(sub).small().color(weak));
                            }
                        });
                        ui.end_row();
                    }
                });
            };
            let fmt = |t: std::time::SystemTime| chrono::DateTime::<chrono::Local>::from(t).format("%A, %-d %B %Y, %H:%M").to_string();
            egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                if st.tab == 0 {
                    card(ui, "General", &mut |ui| {
                        let mut list: Vec<(&str, String, Option<String>)> = Vec::new();
                        if let Some(e) = &st.entry {
                            list.push(("Type", desc.clone(), (!e.is_dir).then(|| e.mime.clone())));
                            if e.is_dir {
                                let val = match st.size {
                                    Some((bytes, count, complete)) => format!(
                                        "{count} item{}, {}{}",
                                        if count == 1 { "" } else { "s" },
                                        fsops::human_size(bytes),
                                        if complete { "" } else { " or more" }
                                    ),
                                    None => "Counting…".into(),
                                };
                                list.push(("Contents", val, None));
                            } else {
                                list.push(("Size", fsops::human_size(e.size), (e.size >= 1000).then(|| format!("{} bytes", e.size))));
                            }
                            if e.is_link {
                                list.push(("Link to", std::fs::read_link(&e.path).map(|p| p.display().to_string()).unwrap_or_default(), None));
                            }
                        }
                        list.push(("Location", st.path.parent().map(|p| p.display().to_string()).unwrap_or_else(|| "/".into()), None));
                        rows(ui, "props-general", list);
                    });
                    card(ui, "Dates", &mut |ui| {
                        let mut list: Vec<(&str, String, Option<String>)> = Vec::new();
                        if let Some(m) = st.entry.as_ref().and_then(|e| e.modified) {
                            list.push(("Modified", fmt(m), None));
                        }
                        if let Some(c) = created {
                            list.push(("Created", fmt(c), None));
                        }
                        if let Some(a) = accessed {
                            list.push(("Last opened", fmt(a), None));
                        }
                        rows(ui, "props-dates", list);
                    });
                } else {
                    card(ui, "Ownership", &mut |ui| {
                        rows(ui, "props-owner", vec![("Owner", st.owner.clone(), None), ("Group", st.group.clone(), None)]);
                    });
                    let is_dir = st.entry.as_ref().is_some_and(|e| e.is_dir);
                    card(ui, "Access", &mut |ui| {
                        let mut mode = st.mode;
                        egui::Grid::new("props-perms").num_columns(4).spacing([32.0, 12.0]).min_col_width(64.0).show(ui, |ui| {
                            ui.label("");
                            for h in ["Read", "Write", if is_dir { "Open" } else { "Run" }] {
                                ui.label(egui::RichText::new(h).color(weak));
                            }
                            ui.end_row();
                            for (who, shift) in [("Owner", 6), ("Group", 3), ("Others", 0)] {
                                ui.label(who);
                                for bit in [4u32, 2, 1] {
                                    let mask = bit << shift;
                                    let mut on = mode & mask != 0;
                                    if ui.checkbox(&mut on, "").changed() {
                                        if on {
                                            mode |= mask
                                        } else {
                                            mode &= !mask
                                        }
                                    }
                                }
                                ui.end_row();
                            }
                        });
                        if mode != st.mode {
                            st.mode = mode;
                            apply_mode = Some(mode);
                        }
                        ui.add_space(4.0);
                        let rwx: String = [6, 3, 0]
                            .iter()
                            .flat_map(|s| [(4, 'r'), (2, 'w'), (1, 'x')].map(|(b, c)| if st.mode >> s & b != 0 { c } else { '-' }))
                            .collect();
                        ui.label(egui::RichText::new(format!("{rwx}  ·  {:04o}", st.mode & 0o7777)).small().monospace().color(weak));
                    });
                }
            });
        });
        if let Some(r) = shown.rect {
            self.window_rects.push(r);
        }
        let open = shown.open;
        let path = st.path.clone();
        if let Some(m) = apply_mode {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(m & 0o7777)) {
                self.message("Could not change permissions", &e.to_string());
                return open;
            }
            self.reload_all();
        }
        if let Some(n) = rename {
            let n = n.trim().to_string();
            if !n.is_empty() && !n.contains('/') {
                let target = path.with_file_name(&n);
                if target != path {
                    if target.exists() {
                        self.message("Name already used", &format!("“{n}” already exists."));
                        return open;
                    }
                    match std::fs::rename(&path, &target) {
                        Ok(()) => {
                            self.history.record(crate::undo::Step::Renamed(path.clone(), target.clone()));
                            st.path = target;
                            self.reload_all();
                        }
                        Err(e) => {
                            self.message("Could not rename", &e.to_string());
                            return open;
                        }
                    }
                }
            }
        }
        open
    }

    pub(super) fn prefs_window(&mut self, ctx: &egui::Context) {
        let mut open = self.prefs_open;
        // Keyboard: while Preferences has focus (and no text field or dialog does), ↑/↓ move
        // between its pages, Home/End jump to the first/last, Esc closes it. Taken here so
        // the file view behind doesn't also react.
        // Esc in a text field (or an open list) first leaves that; egui drops the focus before we
        // get here, so look at whether one was active last frame.
        let busy = self.prefs_busy_last || ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx);
        if self.prefs_focus && matches!(self.dialog, Dialog::None) && !busy {
            let none = egui::Modifiers::NONE;
            let (up, down, home, end, esc) = ctx.input_mut(|i| {
                (
                    i.consume_key(none, Key::ArrowUp),
                    i.consume_key(none, Key::ArrowDown),
                    i.consume_key(none, Key::Home),
                    i.consume_key(none, Key::End),
                    i.consume_key(none, Key::Escape),
                )
            });
            let all = PrefsTab::ALL;
            let at = all.iter().position(|t| *t == self.prefs_tab).unwrap_or(0);
            if up && at > 0 {
                self.prefs_tab = all[at - 1];
            }
            if down && at + 1 < all.len() {
                self.prefs_tab = all[at + 1];
            }
            if home {
                self.prefs_tab = all[0];
            }
            if end {
                self.prefs_tab = all[all.len() - 1];
            }
            if esc {
                open = false;
            }
        }
        if self.font_families.is_empty() {
            self.font_families = crate::fonts::families();
        }
        if self.mono_families.is_empty() {
            self.mono_families = crate::fonts::mono_families();
        }
        if self.installed_themes.is_empty() {
            self.installed_themes = icons::installed_themes();
        }
        let mut new_theme: Option<String> = None;
        let mut matugen_msg: Option<String> = None;
        let mut system_msg: Option<String> = None;
        let mut link_result: Option<Result<String, String>> = None;
        let mut changed = false;
        let mut resort = false;
        let mut chrome = self.window_chrome("Preferences", &["preferences-system-symbolic", "emblem-system-symbolic", "preferences-system"], ctx);
        // The page sidebar runs to the window's edges.
        chrome.flush = true;
        let area = self.window_area(ctx);
        let mut win = std::mem::replace(&mut self.prefs_win, super::windows::WinState::new(0, Vec2::ZERO));
        // Test hook: RIVET_PREFS_SCROLL=PX opens Preferences scrolled down (for screenshots).
        let scroll_to = std::env::var("RIVET_PREFS_SCROLL").ok().and_then(|v| v.parse::<f32>().ok());
        let tab_now = self.prefs_tab;
        let mut pick_tab: Option<PrefsTab> = None;
        let mut show_shortcuts = false;
        self.app_icon_texture(ctx);
        let about_icon = self.app_icon.clone();
        let tab_icons: Vec<Option<egui::TextureHandle>> = PrefsTab::ALL.iter().map(|t| self.icon(ctx, t.icons(), 16.0)).collect();
        let shown = super::windows::show(ctx, &mut win, &chrome, area, Vec2::new(640.0, 380.0), |ui| {
            let v = ui.visuals().clone();
            let weak = v.weak_text_color();
            let full_h = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                // ---- the sidebar of pages
                let side_fill = crate::palette::mix(v.window_fill, v.panel_fill, 0.6);
                egui::Frame::NONE
                    .fill(side_fill)
                    .corner_radius(egui::CornerRadius { sw: super::r_lg(), ..Default::default() })
                    .inner_margin(egui::Margin { left: 10, right: 10, top: 6, bottom: 4 })
                    .show(ui, |ui| {
                        ui.set_width(168.0);
                        ui.set_min_height(full_h - 10.0);
                        ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (i, t) in PrefsTab::ALL.iter().enumerate() {
                            if *t == PrefsTab::About {
                                // About sits apart at the bottom of the list.
                                ui.add_space(10.0);
                            }
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
                            let sel = *t == tab_now;
                            if sel {
                                let (fill, stroke) = super::selected_tile(&v, side_fill);
                                ui.painter().rect_filled(r, super::r_sm(), fill);
                                ui.painter().rect_stroke(r, super::r_sm(), stroke, egui::StrokeKind::Inside);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, super::r_sm(), v.widgets.hovered.weak_bg_fill);
                            }
                            let fg = if sel { v.strong_text_color() } else { v.text_color() };
                            if let Some(tex) = &tab_icons[i] {
                                super::paint_tex(ui.painter(), tex, Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), Vec2::splat(16.0)), fg);
                            }
                            ui.painter().text(Pos2::new(r.left() + 40.0, r.center().y), egui::Align2::LEFT_CENTER, t.label(), crate::fonts::ui_font(13.5), fg);
                            if resp.clicked() {
                                pick_tab = Some(*t);
                            }
                        }
                        });
                    });
                // ---- the page
                ui.vertical(|ui| {
                    let mut sa = egui::ScrollArea::vertical().id_salt(("prefs-page", tab_now as u8)).auto_shrink([false, false]);
                    if let Some(y) = scroll_to {
                        sa = sa.vertical_scroll_offset(y);
                    }
                    sa.show(ui, |ui| {
                        egui::Frame::NONE.inner_margin(egui::Margin { left: 26, right: 22, top: 18, bottom: 22 }).show(ui, |ui| {
                ui.set_width((ui.available_width()).max(380.0));
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
                ui.spacing_mut().button_padding = Vec2::new(10.0, 4.0);
                let card_fill = ui.visuals().faint_bg_color;
                // One titled, roomy card per group of settings.
                let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
                    ui.label(egui::RichText::new(title).strong().size(14.5));
                    ui.add_space(4.0);
                    egui::Frame::NONE.fill(card_fill).corner_radius(super::r_md()).inner_margin(egui::Margin::symmetric(18, 16)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        body(ui);
                    });
                    ui.add_space(22.0);
                };
                let hint = |ui: &mut egui::Ui, text: &str| {
                    ui.add(egui::Label::new(egui::RichText::new(text).small().color(weak)).wrap());
                };
                if tab_now != PrefsTab::About {
                    ui.label(egui::RichText::new(tab_now.title()).size(20.0).strong());
                    ui.add_space(14.0);
                }
                match tab_now {
                    PrefsTab::Appearance => {
                        section(ui, "Desktop colors", &mut |ui| {
                    changed |= ui.checkbox(&mut self.s.follow_desktop_colors, egui::RichText::new("Follow desktop colors").strong()).changed();
                    ui.add_space(6.0);
                    match (&self.palette, self.s.follow_desktop_colors) {
                        (Some(p), true) => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for c in [p.accent, p.window_bg, p.view_bg, p.window_fg].into_iter().flatten() {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                                    ui.painter().rect_filled(r, super::r_xs(), c);
                                    ui.painter().rect_stroke(r, super::r_xs(), Stroke::new(1.0, weak), egui::StrokeKind::Inside);
                                }
                                ui.add_space(4.0);
                                let name = p.source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                ui.label(egui::RichText::new(format!("from {name}")).small().color(weak)).on_hover_text(p.source.display().to_string());
                            });
                        }
                        (None, true) => hint(ui, "No desktop color file found"),
                        _ => {}
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add Rivet template to matugen").on_hover_text("Writes a template into matugen's templates folder and adds it to matugen's config, so matugen writes Rivet's own color file.").clicked() {
                            match palette::install_matugen_template() {
                                Ok(msg) => matugen_msg = Some(msg),
                                Err(e) => matugen_msg = Some(format!("Could not update matugen's config: {e}")),
                            }
                        }
                        if ui.button("Copy template").on_hover_text("Copy the matugen template text").clicked() {
                            ui.ctx().copy_text(palette::MATUGEN_TEMPLATE.to_string());
                            matugen_msg = Some("Template copied.".into());
                        }
                    });
                        });
                        let manual = !(self.s.follow_desktop_colors && self.palette.is_some());
                        section(ui, "Style and accent", &mut |ui| {
                            egui::Grid::new("prefs-look").num_columns(2).spacing([24.0, 18.0]).min_col_width(120.0).show(ui, |ui| {
                        ui.label("Style");
                        ui.add_enabled_ui(manual, |ui| {
                            ui.horizontal(|ui| {
                                changed |= ui.selectable_value(&mut self.s.theme, ThemeChoice::System, "Follow system").changed();
                                changed |= ui.selectable_value(&mut self.s.theme, ThemeChoice::Light, "Light").changed();
                                changed |= ui.selectable_value(&mut self.s.theme, ThemeChoice::Dark, "Dark").changed();
                            });
                        });
                        ui.end_row();
                        // Terminal mode: the same layout, dressed like a terminal program.
                        ui.label("Mode");
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                changed |= ui.selectable_value(&mut self.s.terminal, false, "Standard").changed();
                                changed |= ui.selectable_value(&mut self.s.terminal, true, "Terminal").changed();
                            });
                            // Only when something's missing: without a Nerd Font the icons stay as they are.
                            if self.s.terminal && self.nerd_font.is_none() {
                                hint(ui, "No Nerd Font found, so icons stay as they are. Install one (or pick one as the Monospace Font) for terminal-style icons.");
                            }
                        });
                        ui.end_row();
                        ui.label("Accent");
                        ui.add_enabled_ui(manual, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                for (c, name) in ACCENTS {
                                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                                    let col = Color32::from_rgb(c[0], c[1], c[2]);
                                    let col = if manual { col } else { col.gamma_multiply(0.4) };
                                    ui.painter().circle_filled(r.center(), 9.0, col);
                                    if self.s.accent == c {
                                        ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(2.0, ui.visuals().strong_text_color()));
                                    }
                                    if resp.on_hover_text(name).clicked() && manual {
                                        self.s.accent = c;
                                        changed = true;
                                    }
                                }
                            });
                        });
                        ui.end_row();
                            });
                        });
                        section(ui, "Icons", &mut |ui| {
                    egui::Grid::new("prefs-look-icons").num_columns(2).spacing([24.0, 16.0]).min_col_width(120.0).show(ui, |ui| {
                        ui.label("Icon theme");
                        let sys = format!("System ({})", self.system_icon_theme);
                        let builtin_label = "Rivet (follows your colors)".to_string();
                        let current = if self.s.builtin_icons {
                            builtin_label.clone()
                        } else if self.s.icon_theme.is_empty() {
                            sys.clone()
                        } else {
                            self.s.icon_theme.clone()
                        };
                        egui::ComboBox::from_id_salt("icontheme").selected_text(current).width(260.0).show_ui(ui, |ui| {
                            if ui.selectable_label(self.s.builtin_icons, builtin_label).on_hover_text("Rivet's own icons, recolored from matugen / your desktop colors").clicked() {
                                self.s.builtin_icons = true;
                                changed = true;
                            }
                            ui.separator();
                            if ui.selectable_label(!self.s.builtin_icons && self.s.icon_theme.is_empty(), sys).clicked() {
                                self.s.builtin_icons = false;
                                new_theme = Some(String::new());
                            }
                            for t in &self.installed_themes {
                                if ui.selectable_label(!self.s.builtin_icons && self.s.icon_theme == *t, t).clicked() {
                                    self.s.builtin_icons = false;
                                    new_theme = Some(t.clone());
                                }
                            }
                        });
                        ui.end_row();
                        if self.s.builtin_icons {
                            ui.label("Icon color");
                            ui.horizontal(|ui| {
                                use crate::settings::IconColor;
                                let primary = super::accent_color(&self.s);
                                let pal = if self.s.follow_desktop_colors { self.palette.clone() } else { None };
                                for choice in IconColor::ALL {
                                    let sw = match choice {
                                        IconColor::Primary => primary,
                                        IconColor::Secondary => pal.as_ref().and_then(|p| p.secondary).unwrap_or_else(|| crate::builtin_icons::derive_secondary(primary)),
                                        IconColor::Tertiary => pal.as_ref().and_then(|p| p.tertiary).unwrap_or_else(|| crate::builtin_icons::derive_tertiary(primary)),
                                    };
                                    let sel = self.s.icon_color == choice;
                                    let resp = ui.add(egui::Button::new(format!("      {}", choice.label())).selected(sel));
                                    // A swatch dot inside the button, before the label.
                                    let r = resp.rect;
                                    ui.painter().circle_filled(r.left_center() + Vec2::new(13.0, 0.0), 5.5, sw);
                                    if resp.clicked() {
                                        self.s.icon_color = choice;
                                        changed = true;
                                    }
                                }
                            });
                            ui.end_row();
                        }
                    });
                        });
                        section(ui, "Typography", &mut |ui| {
                    egui::Grid::new("prefs-look-text").num_columns(2).spacing([24.0, 16.0]).min_col_width(120.0).show(ui, |ui| {
                        // UI font family and size.
                        ui.label("Font");
                        let system_name = crate::fonts::desktop_font_cached().map(|(f, _)| f).unwrap_or_else(|| "Sans".into());
                        changed |= font_picker(ui, "uifont", &mut self.s.ui_font, &self.font_families, &mut self.font_filter, &format!("System ({system_name})"));
                        ui.end_row();
                        // The monospace font: text previews, and everything in terminal mode.
                        ui.label("Monospace Font");
                        let mono_system = format!("System ({})", crate::fonts::resolve_mono("").map(|(f, _)| f).unwrap_or_else(|| "Monospace".into()));
                        changed |= font_picker(ui, "monofont", &mut self.s.mono_font, &self.mono_families, &mut self.mono_filter, &mono_system);
                        ui.end_row();
                        ui.label("Text size");
                        ui.horizontal(|ui| {
                            let system_size = crate::fonts::desktop_font_cached().map(|(_, pt)| crate::fonts::points_to_ui(pt)).unwrap_or(crate::fonts::BASE_SIZE);
                            let mut size = if self.s.ui_font_size > 0.0 { self.s.ui_font_size } else { system_size };
                            if ui.add(egui::Slider::new(&mut size, 10.0..=22.0).step_by(0.5).show_value(true)).changed() {
                                self.s.ui_font_size = size;
                                changed = true;
                            }
                            let following = self.s.ui_font_size == 0.0;
                            if ui.add_enabled(!following, egui::Button::new("Use system size")).clicked() {
                                self.s.ui_font_size = 0.0;
                                changed = true;
                            }
                        });
                        ui.end_row();
                    });
                        });
                        section(ui, "Window", &mut |ui| {
                    egui::Grid::new("prefs-window").num_columns(2).spacing([24.0, 16.0]).min_col_width(120.0).show(ui, |ui| {
                        ui.label("Title bar");
                        ui.horizontal(|ui| {
                            changed |= ui.selectable_value(&mut self.s.decorations, true, "System").changed();
                            changed |= ui.selectable_value(&mut self.s.decorations, false, "App").changed();
                        });
                        ui.end_row();
                        ui.label("Toolbar");
                        ui.horizontal(|ui| {
                            changed |= ui.selectable_value(&mut self.s.toolbar_bottom, false, "Top").changed();
                            changed |= ui.selectable_value(&mut self.s.toolbar_bottom, true, "Bottom").changed();
                        });
                        ui.end_row();
                        ui.label("Status bar");
                        ui.horizontal(|ui| {
                            let mut pick = if !self.s.show_statusbar { 0 } else if self.s.statusbar_top { 1 } else { 2 };
                            let before = pick;
                            ui.selectable_value(&mut pick, 0, "Off");
                            ui.selectable_value(&mut pick, 1, "Top");
                            ui.selectable_value(&mut pick, 2, "Bottom");
                            if pick != before {
                                self.s.show_statusbar = pick != 0;
                                if pick != 0 {
                                    self.s.statusbar_top = pick == 1;
                                }
                                changed = true;
                            }
                        });
                        ui.end_row();
                        ui.label("Sidebar");
                        ui.horizontal(|ui| {
                            changed |= ui.selectable_value(&mut self.s.sidebar_right, false, "Left").changed();
                            changed |= ui.selectable_value(&mut self.s.sidebar_right, true, "Right").changed();
                        });
                        ui.end_row();
                        ui.label("Peek panel");
                        ui.horizontal(|ui| {
                            let mut pick = if !self.s.show_peek { 0 } else if self.s.peek_left { 1 } else { 2 };
                            let before = pick;
                            ui.selectable_value(&mut pick, 0, "Off");
                            ui.selectable_value(&mut pick, 1, "Left");
                            ui.selectable_value(&mut pick, 2, "Right");
                            if pick != before {
                                self.s.show_peek = pick != 0;
                                if pick != 0 {
                                    self.s.peek_left = pick == 1;
                                }
                                changed = true;
                            }
                        });
                        ui.end_row();
                    });
                        });
                    }
                    PrefsTab::Views => {
                        section(ui, "Folders", &mut |ui| {
                    egui::Grid::new("prefs-views").num_columns(2).spacing([18.0, 10.0]).min_col_width(110.0).show(ui, |ui| {
                        ui.label("Default view");
                        ui.horizontal(|ui| {
                            changed |= ui.selectable_value(&mut self.s.view, ViewMode::Icons, "Icons").changed();
                            changed |= ui.selectable_value(&mut self.s.view, ViewMode::List, "List").changed();
                            changed |= ui.selectable_value(&mut self.s.view, ViewMode::Compact, "Compact").changed();
                        });
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    resort |= ui.checkbox(&mut self.s.show_hidden, "Show hidden files").changed();
                    resort |= ui.checkbox(&mut self.s.folders_first, "Sort folders before files").changed();
                    changed |= ui.checkbox(&mut self.s.show_thumbnails, "Show picture thumbnails").changed();
                    ui.horizontal(|ui| {
                        ui.label("Sidebar icons");
                        ui.add_space(10.0);
                        for (size, label) in [(0.0, "Off"), (16.0, "Small"), (20.0, "Medium"), (24.0, "Large")] {
                            changed |= ui.selectable_value(&mut self.s.sidebar_icon, size, label).changed();
                        }
                    });
                        });
                    }
                    PrefsTab::Behavior => {
                        section(ui, "Opening and deleting", &mut |ui| {
                    changed |= ui.checkbox(&mut self.s.single_click, "Single click to open items").changed();
                    changed |= ui.checkbox(&mut self.s.confirm_trash, "Ask before moving files to the Trash").changed();
                        });
                        section(ui, "Keyboard", &mut |ui| {
                            changed |= ui.checkbox(&mut self.s.vim_keys, "Vim keys").changed();
                            if self.s.vim_keys {
                                hint(ui, "j k move · h parent · l open · gg G top / bottom · H L back / forward · yy dd p copy, cut, paste · / search · : Quick Actions");
                            }
                        });
                    }
                    PrefsTab::Menu => {
                        section(ui, "Menu bar", &mut |ui| {
                            changed |= ui.checkbox(&mut self.s.show_menubar, "Show the menu bar").changed();
                        });
                        section(ui, "Toolbar", &mut |ui| {
                    ui.add_space(2.0);
                    let mut op: Option<(usize, i32)> = None; // (index, -1 up / +1 down / 0 remove)
                    let n = self.s.toolbar.len();
                    let fg = ui.visuals().text_color();
                    for (i, item) in self.s.toolbar.clone().into_iter().enumerate() {
                        let path = item == ToolItem::PathBar;
                        // The location bar and the menu button can be moved but not removed.
                        let fixed = path || item == ToolItem::Menu;
                        let row_fill = if path { ui.visuals().selection.bg_fill.gamma_multiply(0.18) } else { Color32::TRANSPARENT };
                        egui::Frame::NONE.fill(row_fill).corner_radius(super::r_sm()).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.set_width(ui.available_width());
                                let (icons, fb) = item.icons();
                                let px = (16.0 * ui.ctx().pixels_per_point()) as u32;
                                let tex = if icons.is_empty() { None } else { self.icons.texture(ui.ctx(), icons, px) };
                                let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                                match tex {
                                    Some(t) => super::paint_tex(ui.painter(), &t, r.shrink(1.0), fg),
                                    None if item == ToolItem::Menu => {
                                        let c = r.center();
                                        for dy in [-4.0f32, 0.0, 4.0] {
                                            ui.painter().line_segment([c + Vec2::new(-6.0, dy), c + Vec2::new(6.0, dy)], Stroke::new(1.6, fg));
                                        }
                                    }
                                    None if item == ToolItem::Filters => {
                                        super::filterview::paint_funnel(ui.painter(), r.center(), 14.0, fg);
                                    }
                                    None if item == ToolItem::Separator => {
                                        let c = r.center();
                                        ui.painter().line_segment([c - Vec2::new(0.0, 7.0), c + Vec2::new(0.0, 7.0)], Stroke::new(1.5, weak));
                                    }
                                    None => {
                                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, fb, crate::fonts::ui_font(12.0), fg);
                                    }
                                }
                                let label = if path { egui::RichText::new(item.label()).strong() } else { egui::RichText::new(item.label()) };
                                ui.label(label);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    // A fixed-width slot keeps the arrows lined up in every row.
                                    ui.allocate_ui_with_layout(Vec2::new(28.0, 22.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.set_min_width(28.0);
                                        if !fixed {
                                            if trash_button(ui).on_hover_text("Remove from the toolbar").clicked() {
                                                op = Some((i, 0));
                                            }
                                        } else {
                                        }
                                    });
                                    if arrow_button(ui, false, i + 1 < n).on_hover_text("Move down").clicked() {
                                        op = Some((i, 1));
                                    }
                                    if arrow_button(ui, true, i > 0).on_hover_text("Move up").clicked() {
                                        op = Some((i, -1));
                                    }
                                });
                            });
                        });
                    }
                    if let Some((i, d)) = op {
                        match d {
                            0 => {
                                self.s.toolbar.remove(i);
                            }
                            -1 => self.s.toolbar.swap(i, i - 1),
                            _ => self.s.toolbar.swap(i, i + 1),
                        }
                        changed = true;
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let missing: Vec<ToolItem> = ToolItem::ALL
                            .into_iter()
                            .filter(|t| *t == ToolItem::Separator || (*t != ToolItem::Menu && !self.s.toolbar.contains(t)))
                            .collect();
                        egui::ComboBox::from_id_salt("toolbar-add").selected_text("Add a button…").width(200.0).show_ui(ui, |ui| {
                            for t in missing {
                                if ui.selectable_label(false, t.label()).clicked() {
                                    // New buttons go just left of the location bar's right side, at the end.
                                    self.s.toolbar.push(t);
                                    changed = true;
                                }
                            }
                        });
                        if ui.button("Reset to default").clicked() {
                            self.s.toolbar = ToolItem::default_layout();
                            changed = true;
                        }
                    });
                        });
                    }
                    PrefsTab::Modules => {
                        // One list: each module is a name, a line about it, and a switch.
                        // Look again while this page is open, so installing ffmpeg is noticed.
                        let ffmpeg = crate::media::recheck();
                        if ffmpeg.is_none() && self.s.media_tools {
                            // The module can't work without ffmpeg: switch it off.
                            self.s.media_tools = false;
                            changed = true;
                        }
                        let warn = super::status(ui.ctx()).warning;
                        let card_fill = ui.visuals().faint_bg_color;
                        egui::Frame::NONE.fill(card_fill).corner_radius(super::r_md()).inner_margin(egui::Margin::symmetric(18, 6)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let row = |ui: &mut egui::Ui, title: &str, about: egui::RichText, on: &mut bool, enabled: bool| -> bool {
                                let mut flipped = false;
                                ui.horizontal(|ui| {
                                    ui.set_min_height(58.0);
                                    ui.vertical(|ui| {
                                        ui.add_space(10.0);
                                        ui.spacing_mut().item_spacing.y = 3.0;
                                        ui.label(egui::RichText::new(title).strong().size(14.0));
                                        ui.label(about);
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        flipped = switch(ui, on, enabled).changed();
                                    });
                                });
                                flipped
                            };
                            changed |= row(
                                ui,
                                "Bulk Rename",
                                egui::RichText::new("Rename many items at once — F2 with several selected").color(weak),
                                &mut self.s.bulk_rename,
                                true,
                            );
                            ui.separator();
                            let about = match ffmpeg {
                                Some(_) => egui::RichText::new("Convert pictures, videos and audio, and resize pictures — uses ffmpeg").color(weak),
                                None => egui::RichText::new("Needs ffmpeg, which isn't installed — add it from your distribution's packages").color(warn),
                            };
                            if row(ui, "Convert & Resize", about, &mut self.s.media_tools, ffmpeg.is_some()) {
                                changed = true;
                            }
                        });
                    }
                    PrefsTab::System => {
                        // One row per item: name · status · actions on the right.
                        let row = |ui: &mut egui::Ui, name: &str, sub: Option<String>, ok: Option<bool>, status: &str, actions: &mut dyn FnMut(&mut egui::Ui)| {
                            ui.horizontal(|ui| {
                                ui.set_min_height(34.0);
                                let col = if sub.is_some() { egui::Layout::top_down(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Center) };
                                ui.allocate_ui_with_layout(Vec2::new(190.0, 34.0), col, |ui| {
                                    ui.set_width(190.0);
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(name);
                                    if let Some(sub) = &sub {
                                        ui.label(egui::RichText::new(sub).small().monospace().color(weak));
                                    }
                                });
                                match ok {
                                    Some(true) => check_label(ui, status),
                                    _ => {
                                        ui.label(egui::RichText::new(status).color(weak));
                                    }
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| actions(ui));
                            });
                        };
                        section(ui, "Desktop", &mut |ui| {
                            let installed = desktop::is_installed();
                            row(ui, "App launcher", None, Some(installed), if installed { "Added" } else { "Not added" }, &mut |ui| {
                                if installed {
                                    if ui.button("Remove").clicked() {
                                        system_msg = Some(desktop::uninstall().map(|_| "Removed from the app launcher.".to_string()).unwrap_or_else(|e| e));
                                    }
                                    if ui.button("Update").on_hover_text("Point the launcher at this copy of Rivet").clicked() {
                                        system_msg = Some(desktop::install().map(|_| "Launcher entry updated.".to_string()).unwrap_or_else(|e| e));
                                    }
                                } else if ui.button("Add").clicked() {
                                    system_msg = Some(desktop::install().map(|_| "Rivet is now in your app launcher.".to_string()).unwrap_or_else(|e| e));
                                }
                            });
                            ui.separator();
                            let default = desktop::is_default_file_manager();
                            row(ui, "Default file manager", None, Some(default), if default { "Rivet" } else { "Another app" }, &mut |ui| {
                                if !default && ui.button("Make default").clicked() {
                                    system_msg = Some(
                                        desktop::set_default_file_manager()
                                            .map(|_| "Rivet is now your default file manager.".to_string())
                                            .unwrap_or_else(|e| e),
                                    );
                                }
                            });
                        });
                        section(ui, "Terminal command", &mut |ui| {
                            let spots = [(desktop::LinkSpot::User, "Just for you", "Link"), (desktop::LinkSpot::System, "Everyone", "Link")];
                            for (n, (spot, name, button)) in spots.into_iter().enumerate() {
                                if n > 0 {
                                    ui.separator();
                                }
                                let path = spot.path();
                                let home = crate::places::home();
                                let shown = match path.strip_prefix(&home) {
                                    Ok(r) => format!("~/{}", r.display()),
                                    Err(_) => path.display().to_string(),
                                };
                                let state = desktop::link_state(spot);
                                let (ok, status) = match &state {
                                    desktop::LinkState::ThisBuild => (Some(true), "Linked"),
                                    desktop::LinkState::NotALink => (None, "Installed by a package"),
                                    desktop::LinkState::Elsewhere(_) => (Some(false), "Points to another copy"),
                                    desktop::LinkState::Missing => (Some(false), "Not linked"),
                                };
                                row(ui, name, Some(shown), ok, status, &mut |ui| match &state {
                                    desktop::LinkState::ThisBuild => {
                                        if ui.button("Remove").clicked() {
                                            link_result = Some(desktop::unlink_command(spot).map(|_| format!("Removed {}", spot.path().display())));
                                        }
                                    }
                                    desktop::LinkState::NotALink => {}
                                    _ => {
                                        let tip = if spot == desktop::LinkSpot::System { "Asks for your administrator password" } else { "No password needed" };
                                        if ui.button(button).on_hover_text(tip).clicked() {
                                            link_result = Some(desktop::link_command_here(spot));
                                            if matches!(link_result, Some(Err(_))) {
                                                ui.ctx().copy_text(desktop::link_command(spot));
                                            }
                                        }
                                    }
                                });
                            }
                        });
                    }
                    PrefsTab::About => {
                        ui.add_space(18.0);
                        ui.vertical_centered(|ui| {
                            if let Some(t) = &about_icon {
                                ui.add(egui::Image::from_texture(t).fit_to_exact_size(Vec2::splat(112.0)));
                            }
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Rivet Files").size(24.0).strong());
                            ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(weak));
                            ui.add_space(14.0);
                            ui.set_max_width(380.0);
                            ui.add(egui::Label::new("Rivet is a powerful traditional file manager that can be as bloated or as minimal as you need.").halign(egui::Align::Center).wrap());
                            ui.add_space(14.0);
                            ui.hyperlink_to("github.com/ulinaaron/rivet-files", "https://github.com/ulinaaron/rivet-files");
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Licensed under MIT or Apache-2.0").small().color(weak));
                            ui.label(egui::RichText::new(format!("Icon theme in use: {}", self.icons.name)).small().color(weak));
                            ui.add_space(16.0);
                            if ui.button("Keyboard Shortcuts").clicked() {
                                show_shortcuts = true;
                            }
                        });
                    }
                }
                        });
                    });
                });
            });
        });
        if let Some(t) = pick_tab {
            self.prefs_tab = t;
        }
        if show_shortcuts {
            self.dialog = Dialog::Shortcuts;
        }
        self.prefs_win = win;
        if let Some(r) = shown.rect {
            self.window_rects.push(r);
            self.prefs_rect = Some(r);
        }
        if !shown.open {
            open = false;
        }
        if let Some(t) = new_theme {
            self.set_theme_name(t);
        }
        if let Some(m) = matugen_msg {
            self.flash(&m);
        }
        match link_result {
            Some(Ok(m)) => self.flash(&m),
            Some(Err(e)) => self.message("Couldn't create the rivet command", &e),
            None => {}
        }
        if let Some(m) = system_msg {
            self.flash(&m);
            self.apps = crate::launch::all_apps();
        }
        if resort {
            self.resort_all();
        }
        if changed {
            self.dirty_settings = true;
        }
        self.prefs_open = open;
        self.prefs_busy_last = ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(ctx);
    }
}
