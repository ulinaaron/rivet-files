//! Pop-up dialogs: rename, new item, delete confirmations, open with, properties, preferences.

use eframe::egui::{self, Color32, Id, Key, Sense, Stroke, Vec2};
use std::path::Path;

use super::{Action, Dialog, RivetApp, ACCENTS};
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

/// Window titles in the regular UI size rather than the large heading size.
fn window_title(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(13.5).strong()
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

fn danger_button(ui: &mut egui::Ui, label: &str) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(label).color(Color32::WHITE)).fill(Color32::from_rgb(0xc0, 0x1c, 0x28))).clicked()
}

impl RivetApp {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        let mut close = false;
        let mut next: Option<Dialog> = None;
        let dir = self.current_dir();
        match &mut self.dialog {
            Dialog::None => {}
            Dialog::Rename { path, text, focus } => {
                let r = egui::Modal::new(Id::unique("rename")).show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.heading("Rename");
                    ui.add_space(6.0);
                    let resp = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                    if *focus {
                        resp.request_focus();
                        select_stem(ctx, resp.id, text, path.is_dir());
                        *focus = false;
                    }
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| (ui.button("Rename").clicked() || enter, ui.button("Cancel").clicked())).inner
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
            Dialog::ConfirmDelete(paths) => {
                let n = paths.len();
                let r = egui::Modal::new(Id::unique("confirmdel")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading(if n == 1 {
                        format!("Permanently delete “{}”?", paths[0].file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default())
                    } else {
                        format!("Permanently delete {n} items?")
                    });
                    ui.add_space(4.0);
                    ui.label("If you delete an item, it will be permanently lost.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| (danger_button(ui, "Delete"), ui.button("Cancel").clicked())).inner
                });
                if r.inner.0 {
                    self.ops.push(fsops::spawn_op(OpKind::Delete, paths.clone(), None));
                    close = true;
                }
                if r.inner.1 || r.should_close() {
                    close = true;
                }
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
            Dialog::Properties(_) => {
                close = self.properties_window(ctx);
            }
            Dialog::About => {
                let theme = self.icons.name.clone();
                let r = egui::Modal::new(Id::unique("about")).show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.heading("Rivet Files");
                    ui.label("A file manager written in Rust. No GTK or Qt — it reads your desktop's icon theme, file types, bookmarks and default applications directly.");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Keyboard shortcuts").strong());
                    egui::Grid::new("keys").striped(true).show(ui, |ui| {
                        for (k, d) in [
                            ("Ctrl+L", "Type a location"),
                            ("Ctrl+F or just type", "Filter; Enter searches subfolders"),
                            ("Ctrl+T / Ctrl+W", "New tab / close tab"),
                            ("Ctrl+H", "Show hidden files"),
                            ("Ctrl+1 / 2 / 3", "Icon / list / compact view"),
                            ("Ctrl+C / X / V", "Copy / cut / paste"),
                            ("F2", "Rename"),
                            ("Delete / Shift+Delete", "Trash / delete permanently"),
                            ("Alt+Left / Right / Up", "Back / forward / parent"),
                            ("Ctrl+Shift+N", "New folder"),
                            ("Ctrl+D", "Bookmark this folder"),
                            ("F9", "Show or hide the sidebar"),
                            ("Ctrl+M or F10", "Show or hide the menu bar"),
                        ] {
                            ui.label(k);
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(format!("Icon theme in use: {theme}"));
                    ui.add_space(6.0);
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

    fn open_with_dialog(&mut self, ctx: &egui::Context) -> bool {
        let Dialog::OpenWith { mime, show_all, filter, .. } = &self.dialog else { return true };
        let desc = self.mime.describe(mime);
        let list: Vec<(usize, String, String)> = if *show_all {
            self.apps.iter().enumerate().map(|(i, a)| (i, a.name.clone(), a.icon.clone())).collect()
        } else {
            launch::apps_for(&self.apps, mime)
                .into_iter()
                .filter_map(|a| self.apps.iter().position(|x| x.id == a.id).map(|i| (i, a.name.clone(), a.icon.clone())))
                .collect()
        };
        let f = filter.to_lowercase();
        let mut list: Vec<_> = list.into_iter().filter(|(_, n, _)| f.is_empty() || n.to_lowercase().contains(&f)).collect();
        list.truncate(300);
        let ppp = ctx.pixels_per_point();
        let px = (24.0 * ppp) as u32;
        let textures: Vec<_> = list
            .iter()
            .map(|(_, _, icon)| {
                if icon.starts_with('/') {
                    self.icons.texture_from_path(ctx, Path::new(icon), px)
                } else {
                    self.icons.texture(ctx, &[icon.as_str(), "application-x-executable"], px)
                }
            })
            .collect();
        let Dialog::OpenWith { paths, show_all, filter, .. } = &mut self.dialog else { return true };
        let mut launch_idx = None;
        let r = egui::Modal::new(Id::unique("openwith")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("Open With");
            let what = if paths.len() == 1 { paths[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() } else { format!("{} files", paths.len()) };
            ui.label(format!("Choose an application for “{what}” ({desc})"));
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(filter).hint_text("Filter applications").desired_width(f32::INFINITY));
            ui.checkbox(show_all, "Show all applications");
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                if list.is_empty() {
                    ui.label("No suggested applications. Tick “Show all applications”.");
                }
                for ((i, name, _), tex) in list.iter().zip(textures.iter()) {
                    let btn = match tex {
                        Some(t) => egui::Button::image_and_text(egui::Image::from_texture(t).fit_to_exact_size(Vec2::splat(24.0)), name.as_str()),
                        None => egui::Button::new(name.as_str()),
                    };
                    if ui.add(btn.frame(false).min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                        launch_idx = Some(*i);
                    }
                }
            });
            ui.add_space(6.0);
            ui.button("Cancel").clicked()
        });
        if let Some(i) = launch_idx {
            let p = paths.clone();
            self.pending.push(Action::Launch(i, p));
            return true;
        }
        r.inner || r.should_close()
    }

    fn properties_window(&mut self, ctx: &egui::Context) -> bool {
        let tex = {
            let Dialog::Properties(st) = &self.dialog else { return true };
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
        let Dialog::Properties(st) = &mut self.dialog else { return true };
        let desc = st.entry.as_ref().map(|e| self.mime.describe(&e.mime)).unwrap_or_default();
        let mut open = true;
        let mut apply_mode: Option<u32> = None;
        let mut rename: Option<String> = None;
        let (serial, centre) = (self.popup_serial, ctx.content_rect().center());
        let title = format!("{} — Properties", st.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into()));
        egui::Window::new(window_title(&title)).id(Id::unique(("props", serial))).pivot(egui::Align2::CENTER_CENTER).default_pos(centre).collapsible(false).resizable(false).open(&mut open).default_width(400.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.tab, 0, "Basic");
                ui.selectable_value(&mut st.tab, 1, "Permissions");
            });
            ui.add_space(8.0);
            if st.tab == 0 {
                ui.horizontal(|ui| {
                    if let Some(t) = &tex {
                        ui.add(egui::Image::from_texture(t).fit_to_exact_size(Vec2::splat(64.0)));
                    }
                    ui.vertical(|ui| {
                        ui.label("Name:");
                        let r = ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(260.0));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            rename = Some(st.name.clone());
                        }
                    });
                });
                ui.add_space(6.0);
                egui::Grid::new("propgrid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, k: &str, v: String| {
                        ui.label(egui::RichText::new(k).strong());
                        ui.label(v);
                        ui.end_row();
                    };
                    row(ui, "Type:", desc.clone());
                    if let Some(e) = &st.entry {
                        if e.is_dir {
                            let v = match st.size {
                                Some((bytes, count, complete)) => format!(
                                    "{count} item{}, totalling {}{}",
                                    if count == 1 { "" } else { "s" },
                                    fsops::human_size(bytes),
                                    if complete { "" } else { " (partial count)" }
                                ),
                                None => "—".into(),
                            };
                            row(ui, "Contents:", v);
                        } else {
                            row(ui, "Size:", format!("{} ({} bytes)", fsops::human_size(e.size), e.size));
                        }
                        if e.is_link {
                            row(ui, "Link target:", std::fs::read_link(&e.path).map(|p| p.display().to_string()).unwrap_or_default());
                        }
                    }
                    row(ui, "Location:", st.path.parent().map(|p| p.display().to_string()).unwrap_or_else(|| "/".into()));
                    let fmt = |t: std::time::SystemTime| chrono::DateTime::<chrono::Local>::from(t).format("%A, %-d %B %Y, %H:%M").to_string();
                    if let Some(m) = st.entry.as_ref().and_then(|e| e.modified) {
                        row(ui, "Modified:", fmt(m));
                    }
                    if let Some(acc) = std::fs::metadata(&st.path).ok().and_then(|m| m.accessed().ok()) {
                        row(ui, "Accessed:", fmt(acc));
                    }
                    if let Some((total, free)) = places::disk_space(&st.path) {
                        row(ui, "Free space:", format!("{} of {}", fsops::human_size(free), fsops::human_size(total)));
                    }
                });
            } else {
                egui::Grid::new("permgrid").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(egui::RichText::new("Owner:").strong());
                    ui.label(&st.owner);
                    ui.end_row();
                    ui.label(egui::RichText::new("Group:").strong());
                    ui.label(&st.group);
                    ui.end_row();
                    ui.label("");
                    ui.label("Read");
                    ui.label("Write");
                    ui.label(if st.entry.as_ref().is_some_and(|e| e.is_dir) { "Open" } else { "Execute" });
                    ui.end_row();
                    let mut mode = st.mode;
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
                    if mode != st.mode {
                        st.mode = mode;
                        apply_mode = Some(mode);
                    }
                });
                ui.label(egui::RichText::new(format!("Mode: {:04o}", st.mode & 0o7777)).small().weak());
            }
        });
        let path = st.path.clone();
        if let Some(m) = apply_mode {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(m & 0o7777)) {
                self.message("Could not change permissions", &e.to_string());
                return false;
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
                        return false;
                    }
                    match std::fs::rename(&path, &target) {
                        Ok(()) => {
                            if let Dialog::Properties(st) = &mut self.dialog {
                                st.path = target;
                            }
                            self.reload_all();
                        }
                        Err(e) => {
                            self.message("Could not rename", &e.to_string());
                            return false;
                        }
                    }
                }
            }
        }
        !open
    }

    pub(super) fn prefs_window(&mut self, ctx: &egui::Context) {
        let mut open = self.prefs_open;
        if self.font_families.is_empty() {
            self.font_families = crate::fonts::families();
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
        let (serial, centre) = (self.popup_serial, ctx.content_rect().center());
        egui::Window::new(window_title("Preferences"))
            .id(Id::unique(("prefs", serial)))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(centre)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let max_h = (ctx.content_rect().height() - 140.0).max(240.0);
                egui::ScrollArea::vertical().max_height(max_h).min_scrolled_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(520.0);
                ui.add_space(6.0);
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
                ui.spacing_mut().button_padding = Vec2::new(10.0, 4.0);
                let card_fill = ui.visuals().faint_bg_color;
                let weak = ui.visuals().weak_text_color();
                // One titled, padded card per group of settings.
                let section = |ui: &mut egui::Ui, title: &str, body: &mut dyn FnMut(&mut egui::Ui)| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(title).strong().size(14.5));
                    ui.add_space(2.0);
                    egui::Frame::NONE.fill(card_fill).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        body(ui);
                    });
                    ui.add_space(10.0);
                };
                let hint = |ui: &mut egui::Ui, text: &str| {
                    ui.add(egui::Label::new(egui::RichText::new(text).small().color(weak)).wrap());
                };

                section(ui, "Views", &mut |ui| {
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
                });

                section(ui, "Behaviour", &mut |ui| {
                    changed |= ui.checkbox(&mut self.s.single_click, "Single click to open items").changed();
                    changed |= ui.checkbox(&mut self.s.confirm_trash, "Ask before moving files to the Trash").changed();
                });

                section(ui, "System", &mut |ui| {
                    let installed = desktop::is_installed();
                    ui.horizontal(|ui| {
                        if installed {
                            check_label(ui, "In your app launcher");
                            if ui.button("Update entry").on_hover_text("Point the launcher at this copy of Rivet").clicked() {
                                system_msg = Some(desktop::install().map(|_| "Launcher entry updated.".to_string()).unwrap_or_else(|e| e));
                            }
                            if ui.button("Remove").clicked() {
                                system_msg = Some(desktop::uninstall().map(|_| "Removed from the app launcher.".to_string()).unwrap_or_else(|e| e));
                            }
                        } else if ui.button("Add Rivet to the app launcher").clicked() {
                            system_msg = Some(desktop::install().map(|_| "Rivet is now in your app launcher.".to_string()).unwrap_or_else(|e| e));
                        }
                    });
                    ui.horizontal(|ui| {
                        if desktop::is_default_file_manager() {
                            check_label(ui, "Default file manager");
                        } else if ui.button("Make Rivet the default file manager").on_hover_text("Folders opened from other apps and the desktop will open in Rivet").clicked() {
                            system_msg = Some(
                                desktop::set_default_file_manager()
                                    .map(|_| "Rivet is now your default file manager.".to_string())
                                    .unwrap_or_else(|e| e),
                            );
                        }
                    });
                    hint(ui, "Adds a standard desktop entry so Rivet appears in your launcher, menus and “Open With” lists.");
                    ui.add_space(6.0);
                    // A global `rivet` command that always opens this build (rebuilds included).
                    for (spot, title, button, tip) in [
                        (desktop::LinkSpot::System, "rivet command for everyone", "Link in /usr/bin…", "Asks for your administrator password"),
                        (desktop::LinkSpot::User, "rivet command just for you", "Link in ~/.local/bin", "No password needed"),
                    ] {
                        ui.horizontal(|ui| {
                            let state = desktop::link_state(spot);
                            match &state {
                                desktop::LinkState::ThisBuild => {
                                    check_label(ui, title);
                                    if ui.button("Remove").clicked() {
                                        link_result = Some(desktop::unlink_command(spot).map(|_| format!("Removed {}", spot.path().display())));
                                    }
                                }
                                desktop::LinkState::NotALink => {
                                    ui.label(egui::RichText::new(format!("{} is a packaged program; left alone", spot.path().display())).color(weak));
                                }
                                other => {
                                    let label = if matches!(other, desktop::LinkState::Elsewhere(_)) { format!("{button} (replaces an older link)") } else { button.to_string() };
                                    if ui.button(label).on_hover_text(tip).clicked() {
                                        link_result = Some(desktop::link_command_here(spot));
                                        if matches!(link_result, Some(Err(_))) {
                                            ui.ctx().copy_text(desktop::link_command(spot));
                                        }
                                    }
                                }
                            }
                        });
                    }
                    hint(ui, "Lets you type rivet in a terminal or launcher. It points at this build, so every rebuild is picked up.");
                });

                section(ui, "Window", &mut |ui| {
                    changed |= ui.checkbox(&mut self.s.decorations, "Use the system title bar and borders").changed();
                    hint(ui, "When off, Rivet draws its own title bar with minimise, maximise and close buttons.");
                });

                section(ui, "Appearance", &mut |ui| {
                    changed |= ui.checkbox(&mut self.s.follow_desktop_colors, "Follow desktop colours (matugen, Noctalia, GTK theme)").changed();
                    match (&self.palette, self.s.follow_desktop_colors) {
                        (Some(p), true) => {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for c in [p.accent, p.window_bg, p.view_bg, p.window_fg].into_iter().flatten() {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                                    ui.painter().rect_filled(r, 4.0, c);
                                    ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0, weak), egui::StrokeKind::Inside);
                                }
                                ui.add_space(4.0);
                                let name = p.source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                ui.label(egui::RichText::new(format!("from {name}")).small().color(weak)).on_hover_text(p.source.display().to_string());
                            });
                            hint(ui, "Colours update by themselves within a second of a change.");
                        }
                        (None, true) => hint(ui, "No desktop colour file found yet — using the style below."),
                        _ => {}
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Add Rivet template to matugen").on_hover_text("Writes a template into matugen's templates folder and adds it to matugen's config, so matugen writes Rivet's own colour file.").clicked() {
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
                    ui.add_space(6.0);
                    let manual = !(self.s.follow_desktop_colors && self.palette.is_some());
                    egui::Grid::new("prefs-look").num_columns(2).spacing([18.0, 12.0]).min_col_width(110.0).show(ui, |ui| {
                        ui.label("Style");
                        ui.add_enabled_ui(manual, |ui| {
                            ui.horizontal(|ui| {
                                changed |= ui.selectable_value(&mut self.s.theme, ThemeChoice::System, "Follow system").changed();
                                changed |= ui.selectable_value(&mut self.s.theme, ThemeChoice::Light, "Light").changed();
                                changed |= ui.selectable_value(&mut self.s.theme, ThemeChoice::Dark, "Dark").changed();
                            });
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
                        ui.label("Icon theme");
                        let sys = format!("System ({})", self.system_icon_theme);
                        let builtin_label = "Rivet (follows your colours)".to_string();
                        let current = if self.s.builtin_icons {
                            builtin_label.clone()
                        } else if self.s.icon_theme.is_empty() {
                            sys.clone()
                        } else {
                            self.s.icon_theme.clone()
                        };
                        egui::ComboBox::from_id_salt("icontheme").selected_text(current).width(260.0).show_ui(ui, |ui| {
                            if ui.selectable_label(self.s.builtin_icons, builtin_label).on_hover_text("Rivet's own icons, recoloured from matugen / your desktop colours").clicked() {
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
                            ui.label("Icon colour");
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
                        // UI font family and size.
                        ui.label("Font");
                        let system_name = crate::fonts::desktop_font_cached().map(|(f, _)| f).unwrap_or_else(|| "Sans".into());
                        let current = if self.s.ui_font.is_empty() { format!("System ({system_name})") } else { self.s.ui_font.clone() };
                        egui::ComboBox::from_id_salt("uifont").selected_text(current).width(260.0).height(320.0).show_ui(ui, |ui| {
                            if ui.selectable_label(self.s.ui_font.is_empty(), format!("System ({system_name})")).clicked() {
                                self.s.ui_font.clear();
                                changed = true;
                            }
                            ui.separator();
                            for fam in &self.font_families {
                                if ui.selectable_label(self.s.ui_font == *fam, fam).clicked() {
                                    self.s.ui_font = fam.clone();
                                    changed = true;
                                }
                            }
                        });
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
                    if !manual {
                        hint(ui, "Style and accent come from your desktop colours while following them.");
                    }
                });

                section(ui, "Toolbar", &mut |ui| {
                    hint(ui, "Choose the buttons and their order. Items above the location bar sit on its left, items below it on its right.");
                    ui.add_space(2.0);
                    let mut op: Option<(usize, i32)> = None; // (index, -1 up / +1 down / 0 remove)
                    let n = self.s.toolbar.len();
                    let fg = ui.visuals().text_color();
                    for (i, item) in self.s.toolbar.clone().into_iter().enumerate() {
                        let path = item == ToolItem::PathBar;
                        // The location bar and the menu button can be moved but not removed.
                        let fixed = path || item == ToolItem::Menu;
                        let row_fill = if path { ui.visuals().selection.bg_fill.gamma_multiply(0.18) } else { Color32::TRANSPARENT };
                        egui::Frame::NONE.fill(row_fill).corner_radius(6.0).inner_margin(egui::Margin::symmetric(6, 2)).show(ui, |ui| {
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
                                    if !fixed {
                                        if ui.small_button("Remove").clicked() {
                                            op = Some((i, 0));
                                        }
                                    } else {
                                        ui.label(egui::RichText::new(if path { "always shown" } else { "keeps menus reachable" }).small().color(weak));
                                    }
                                    if ui.add_enabled(i + 1 < n, egui::Button::new("Down").small()).clicked() {
                                        op = Some((i, 1));
                                    }
                                    if ui.add_enabled(i > 0, egui::Button::new("Up").small()).clicked() {
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
                });
            });
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
    }
}
