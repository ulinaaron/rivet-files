//! The Bulk Rename window: pick a method (insert, date/time, numbering, remove, search &
//! replace, case), set it up, and see every new name before anything changes.

use eframe::egui::{self, Id, Key, Vec2};
use std::path::PathBuf;
use std::time::SystemTime;

use super::{Dialog, RivetApp, r_lg, r_md};
use crate::rename::{self, CaseMode, DateSource, From, Mode, Part, Rules};

pub(super) struct BulkState {
    /// (path, is folder, modified), in the order shown in the folder.
    pub items: Vec<(PathBuf, bool, Option<SystemTime>)>,
    pub rules: Rules,
    pub focus: bool,
}

impl BulkState {
    pub fn new(items: Vec<(PathBuf, bool, Option<SystemTime>)>) -> Self {
        BulkState { items, rules: Rules::default(), focus: true }
    }
}

/// A row of pill choices (the same look as Preferences).
fn pills<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, choices: &[(T, &str)]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
        for (v, label) in choices {
            ui.selectable_value(value, *v, *label);
        }
    });
}

/// A text field in the app's shaded style.
fn field(ui: &mut egui::Ui, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let v = ui.visuals().clone();
    let bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
    egui::Frame::NONE
        .fill(bg)
        .corner_radius(r_md())
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .frame(egui::Frame::NONE)
                    .hint_text(hint)
                    .align(egui::Align2::LEFT_CENTER)
                    .min_size(Vec2::new(0.0, 22.0))
                    .desired_width(width),
            )
        })
        .inner
}

impl RivetApp {
    /// Returns true when the window should close.
    pub(super) fn bulk_rename_dialog(&mut self, ctx: &egui::Context) -> bool {
        let mode_icons: Vec<Option<egui::TextureHandle>> = Mode::ALL.iter().map(|m| self.icon(ctx, mode_icon(*m), 16.0)).collect();
        let close_icon = self.icon(ctx, &["window-close-symbolic"], 14.0);
        let Dialog::BulkRename(st) = &mut self.dialog else { return true };
        let plan = rename::plan(&st.items, &st.rules);
        let changes = plan.iter().filter(|p| p.changes()).count();
        let problems = plan.iter().filter(|p| p.problem.is_some()).count();
        let danger = super::status(ctx).danger;
        let success = super::status(ctx).success;
        let mut ok = false;
        let mut cancel = false;

        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(r_lg());
        let r = egui::Modal::new(Id::unique("bulk-rename")).frame(frame).show(ctx, |ui| {
            ui.set_width(760.0);
            let v = ui.visuals().clone();
            let weak = v.weak_text_color();
            let card = v.faint_bg_color;
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
            ui.spacing_mut().button_padding = Vec2::new(10.0, 4.0);
            // ---- title and a close button
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Rename {} items", st.items.len())).heading());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::click());
                    let resp = resp.on_hover_text("Close (Esc)");
                    let fg = if resp.hovered() {
                        ui.painter().circle_filled(r.center(), 13.0, danger);
                        super::StatusColors::on(danger)
                    } else {
                        v.text_color()
                    };
                    match &close_icon {
                        Some(t) => super::paint_tex(ui.painter(), t, egui::Rect::from_center_size(r.center(), Vec2::splat(14.0)), fg),
                        None => {
                            let c = r.center();
                            let st = egui::Stroke::new(1.6, fg);
                            ui.painter().line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], st);
                            ui.painter().line_segment([c + Vec2::new(4.5, -4.5), c + Vec2::new(-4.5, 4.5)], st);
                        }
                    }
                    if resp.clicked() {
                        cancel = true;
                    }
                });
            });
            ui.add_space(10.0);

            let rl = &mut st.rules;
            let focus = std::mem::take(&mut st.focus);
            ui.horizontal_top(|ui| {
            // ---- the methods, as a list like the Preferences pages
            let list_w = 196.0;
            egui::Frame::NONE.fill(card).corner_radius(r_md()).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                ui.set_width(list_w - 12.0);
                ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, m) in Mode::ALL.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::click());
                    let sel = rl.mode == *m;
                    if sel {
                        let (fill, stroke) = super::selected_tile(&v, card);
                        ui.painter().rect_filled(r, super::r_sm(), fill);
                        ui.painter().rect_stroke(r, super::r_sm(), stroke, egui::StrokeKind::Inside);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, super::r_sm(), v.widgets.hovered.weak_bg_fill);
                    }
                    let fg = if sel { v.strong_text_color() } else { v.text_color() };
                    if let Some(t) = &mode_icons[i] {
                        super::paint_tex(ui.painter(), t, egui::Rect::from_center_size(egui::Pos2::new(r.left() + 18.0, r.center().y), Vec2::splat(16.0)), fg);
                    }
                    ui.painter().text(egui::Pos2::new(r.left() + 36.0, r.center().y), egui::Align2::LEFT_CENTER, m.label(), crate::fonts::ui_font(13.0), fg);
                    if resp.clicked() {
                        rl.mode = *m;
                    }
                }
                });
            });
            // ---- its settings
            egui::Frame::NONE.fill(card).corner_radius(r_md()).inner_margin(egui::Margin::symmetric(16, 14)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(6.0 * 34.0 - 16.0);
                egui::Grid::new("bulk-opts").num_columns(2).spacing([18.0, 12.0]).min_col_width(110.0).show(ui, |ui| {
                    let label = |ui: &mut egui::Ui, t: &str| {
                        ui.label(egui::RichText::new(t).color(weak));
                    };
                    match rl.mode {
                        Mode::Insert => {
                            label(ui, "Text");
                            let f = field(ui, &mut rl.insert_text, "Text to insert", 280.0);
                            if focus {
                                f.request_focus();
                            }
                            ui.end_row();
                            label(ui, "Position");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut rl.insert_at).range(0..=255).suffix(" characters"));
                                ui.label(egui::RichText::new("from").color(weak));
                                pills(ui, &mut rl.insert_from, &[(From::Start, "Start"), (From::End, "End")]);
                            });
                            ui.end_row();
                            label(ui, "Mode");
                            pills(ui, &mut rl.overwrite, &[(false, "Insert"), (true, "Overwrite")]);
                            ui.end_row();
                        }
                        Mode::DateTime => {
                            label(ui, "Format");
                            ui.vertical(|ui| {
                                field(ui, &mut rl.date_format, "%Y-%m-%d", 280.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    for (fmt, show) in [("%Y-%m-%d ", "2026-05-17"), ("%Y%m%d_", "20260517"), ("%Y-%m-%d %H.%M ", "2026-05-17 14.30"), ("%d.%m.%Y ", "17.05.2026")] {
                                        if ui.selectable_label(rl.date_format == fmt, show).clicked() {
                                            rl.date_format = fmt.into();
                                        }
                                    }
                                });
                            });
                            ui.end_row();
                            label(ui, "Date of");
                            pills(ui, &mut rl.date_source, &[(DateSource::Modified, "Last modified"), (DateSource::Now, "Now")]);
                            ui.end_row();
                            label(ui, "Position");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut rl.date_at).range(0..=255).suffix(" characters"));
                                ui.label(egui::RichText::new("from").color(weak));
                                pills(ui, &mut rl.date_from, &[(From::Start, "Start"), (From::End, "End")]);
                            });
                            ui.end_row();
                        }
                        Mode::Number => {
                            label(ui, "Start at");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut rl.number_start).range(-9999..=999_999));
                                ui.label(egui::RichText::new("step").color(weak));
                                ui.add(egui::DragValue::new(&mut rl.number_step).range(-999..=999));
                                ui.label(egui::RichText::new("digits").color(weak));
                                ui.add(egui::DragValue::new(&mut rl.number_digits).range(1..=8));
                            });
                            ui.end_row();
                            label(ui, "Text between");
                            field(ui, &mut rl.number_text, "e.g. “ - ” or “_”", 160.0);
                            ui.end_row();
                            label(ui, "Number goes");
                            pills(ui, &mut rl.number_after, &[(false, "Before the name"), (true, "After the name")]);
                            ui.end_row();
                        }
                        Mode::Remove => {
                            label(ui, "Remove");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut rl.remove_count).range(0..=255).suffix(" characters"));
                            });
                            ui.end_row();
                            label(ui, "Starting at");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut rl.remove_from).range(0..=255));
                                ui.label(egui::RichText::new("from").color(weak));
                                pills(ui, &mut rl.remove_side, &[(From::Start, "Start"), (From::End, "End")]);
                            });
                            ui.end_row();
                        }
                        Mode::Replace => {
                            label(ui, "Find");
                            let f = field(ui, &mut rl.find, "Text to look for", 280.0);
                            if focus {
                                f.request_focus();
                            }
                            ui.end_row();
                            label(ui, "Replace with");
                            field(ui, &mut rl.replace, "Leave empty to remove it", 280.0);
                            ui.end_row();
                            label(ui, "");
                            ui.checkbox(&mut rl.match_case, "Match case");
                            ui.end_row();
                        }
                        Mode::Case => {
                            label(ui, "Change to");
                            pills(
                                ui,
                                &mut rl.case,
                                &[(CaseMode::Lower, "lowercase"), (CaseMode::Upper, "UPPERCASE"), (CaseMode::Title, "Title Case"), (CaseMode::Sentence, "Sentence case")],
                            );
                            ui.end_row();
                        }
                    }
                    label(ui, "Apply to");
                    pills(ui, &mut rl.part, &[(Part::Name, "Name"), (Part::Extension, "Extension"), (Part::Both, "Name and extension")]);
                    ui.end_row();
                });
            });

            });

            // ---- preview
            ui.add_space(10.0);
            ui.label(egui::RichText::new("PREVIEW").size(10.5).strong().color(weak));
            egui::Frame::NONE.fill(card).corner_radius(r_md()).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                    let col = (ui.available_width() - 40.0) / 2.0;
                    egui::Grid::new("bulk-preview").num_columns(3).spacing([10.0, 6.0]).min_col_width(20.0).show(ui, |ui| {
                        for p in &plan {
                            let old = p.from.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            let new = p.to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            let cell = |ui: &mut egui::Ui, text: egui::RichText| {
                                ui.allocate_ui_with_layout(Vec2::new(col, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.set_width(col);
                                    ui.add(egui::Label::new(text).truncate())
                                })
                                .inner
                            };
                            cell(ui, egui::RichText::new(&old).color(weak));
                            super::arrow_label(ui, weak);
                            let colr = if p.problem.is_some() { danger } else if p.changes() { v.strong_text_color() } else { weak };
                            let resp = cell(ui, egui::RichText::new(&new).color(colr));
                            if let Some(why) = &p.problem {
                                resp.on_hover_text(why);
                            }
                            ui.end_row();
                        }
                    });
                });
            });

            // ---- footer
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if problems > 0 {
                    let first = plan.iter().find_map(|p| p.problem.clone()).unwrap_or_default();
                    ui.label(egui::RichText::new(format!("{problems} can't be renamed — {first}")).color(danger));
                } else {
                    ui.label(egui::RichText::new(format!("{changes} of {} will be renamed", plan.len())).color(weak));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.spacing_mut().button_padding = Vec2::new(14.0, 5.0);
                    let can = changes > 0 && problems == 0;
                    let btn = egui::Button::new(egui::RichText::new("Rename").color(if can { super::StatusColors::on(success) } else { weak })).fill(if can { success } else { v.widgets.inactive.weak_bg_fill });
                    if ui.add_enabled(can, btn).clicked() {
                        ok = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Enter)) && changes > 0 && problems == 0 && !ctx.egui_wants_keyboard_input() {
            ok = true;
        }
        if cancel || (r.should_close() && !ok) {
            return true;
        }
        if !ok {
            return false;
        }
        match rename::apply(&plan) {
            Ok(done) => {
                let n = done.len();
                self.history.record(crate::undo::Step::RenamedMany(done.clone()));
                self.reload_all();
                let t = self.tab_mut();
                t.selected = done.iter().map(|(_, new)| new.clone()).collect();
                t.scroll_to = done.first().map(|(_, new)| new.clone());
                self.flash(&format!("Renamed {n} item{}", if n == 1 { "" } else { "s" }));
            }
            Err(e) => {
                self.reload_all();
                self.message("Some items couldn't be renamed", &e);
                return false;
            }
        }
        true
    }
}

/// A small icon for each rename method (first one found in the icon theme).
fn mode_icon(m: Mode) -> &'static [&'static str] {
    match m {
        Mode::Insert => &["insert-text-symbolic", "format-text-bold-symbolic", "document-edit-symbolic"],
        Mode::DateTime => &["x-office-calendar-symbolic", "office-calendar-symbolic", "alarm-symbolic"],
        Mode::Number => &["view-list-ordered-symbolic", "format-ordered-list-symbolic", "view-list-symbolic"],
        Mode::Remove => &["edit-clear-symbolic", "edit-cut-symbolic", "edit-delete-symbolic"],
        Mode::Replace => &["edit-find-replace-symbolic", "edit-find-symbolic"],
        Mode::Case => &["format-text-uppercase-symbolic", "font-x-generic-symbolic", "preferences-desktop-font-symbolic"],
    }
}
