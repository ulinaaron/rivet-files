//! The Filters sidebar view and the strip above the files that says a filter is on.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use super::{RivetApp, R_MD, R_SM};
use crate::filters::{DateRange, Filter, NameMode, Scope, TypeGroup};

/// A small funnel, drawn so it looks the same in every icon theme.
pub(super) fn paint_funnel(p: &egui::Painter, c: Pos2, size: f32, color: Color32) {
    let s = size / 16.0;
    let pts = vec![
        c + Vec2::new(-6.5, -5.5) * s,
        c + Vec2::new(6.5, -5.5) * s,
        c + Vec2::new(1.5, 0.5) * s,
        c + Vec2::new(1.5, 5.5) * s,
        c + Vec2::new(-1.5, 3.8) * s,
        c + Vec2::new(-1.5, 0.5) * s,
    ];
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

impl RivetApp {
    /// Open the sidebar on the Filters view.
    pub(super) fn show_filters(&mut self) {
        self.s.show_sidebar = true;
        self.s.sidebar_filters = true;
        self.s.sidebar_tree = false;
        self.dirty_settings = true;
    }

    /// The thin strip at the top of the file area while a filter is on here.
    pub(super) fn filter_strip(&mut self, ui: &mut egui::Ui) {
        let Some(dir) = self.tab().loc.dir().map(|d| d.to_path_buf()) else { return };
        let Some((scope, rule)) = self.s.filters.active_for(&dir).map(|(s, r)| (s, r.clone())) else { return };
        let overridden = self.s.filters.overridden(&dir);
        let hidden = self.tab().filtered_out;
        let v = ui.visuals().clone();
        let accent = super::accent_color(&self.s);
        let fill = crate::palette::mix(v.extreme_bg_color, accent, 0.16);
        let mut open = false;
        let mut off = false;
        egui::Frame::NONE.fill(fill).corner_radius(R_MD).inner_margin(egui::Margin { left: 6, right: 8, top: 3, bottom: 3 }).show(ui, |ui| {
            ui.set_width(ui.available_width() - 6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // The funnel opens the Filters sidebar.
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, R_SM, v.widgets.hovered.weak_bg_fill);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                paint_funnel(ui.painter(), r.center(), 16.0, accent);
                open |= resp.on_hover_text("Show the filters").clicked();
                let mut text = format!("{} — {}", scope.label(), rule.summary());
                if !overridden.is_empty() {
                    let names: Vec<String> = overridden.iter().map(|s| s.label()).collect();
                    text.push_str(&format!(" (instead of {})", names.join(", ")));
                }
                ui.add(egui::Label::new(egui::RichText::new(text).color(v.text_color())).truncate());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
                    off = ui.add(egui::Button::new("Turn off").frame_when_inactive(false)).on_hover_text("Switch this filter off (its settings are kept)").clicked();
                    open |= ui.add(egui::Button::new("Edit").frame_when_inactive(false)).clicked();
                    if hidden > 0 {
                        ui.label(egui::RichText::new(format!("{hidden} hidden")).color(v.weak_text_color()));
                    }
                });
            });
        });
        ui.add_space(4.0);
        if open {
            self.filter_scope = Some(scope.clone());
            self.show_filters();
        }
        if off {
            self.s.filters.get_mut(&scope).enabled = false;
            self.s.filters.save();
            self.resort_all();
        }
    }

    /// The Filters view of the sidebar.
    pub(super) fn filters_view(&mut self, ui: &mut egui::Ui) {
        let v = ui.visuals().clone();
        let weak = v.weak_text_color();
        let accent = super::accent_color(&self.s);
        let w = ui.available_width();
        // ---- header, like the other sidebar sections, with the folder underneath
        ui.add_space(10.0);
        let (hr, _) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::hover());
        paint_funnel(ui.painter(), Pos2::new(hr.left() + 12.0, hr.center().y), 13.0, v.text_color());
        ui.painter().text(Pos2::new(hr.left() + 24.0, hr.center().y), egui::Align2::LEFT_CENTER, "Filters", crate::fonts::ui_font(13.5), v.text_color());
        let Some(dir) = self.tab().loc.dir().map(|d| d.to_path_buf()) else {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.add(egui::Label::new(egui::RichText::new("Open a folder to filter it.").color(weak)).wrap());
            });
            return;
        };
        let active = self.s.filters.active_for(&dir).map(|(s, _)| s);
        let folder_name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());

        // ---- which filter to edit
        let mut rows: Vec<(Scope, String)> = vec![
            (Scope::Global, "Global".into()),
            (Scope::Folder(dir.clone()), "This folder".into()),
            (Scope::Tree(dir.clone()), "This folder & below".into()),
        ];
        if let Some(Scope::Tree(p)) = &active {
            if *p != dir {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());
                rows.push((Scope::Tree(p.clone()), format!("“{name}” & below")));
            }
        }
        let mut pick = self
            .filter_scope
            .clone()
            .filter(|s| rows.iter().any(|(r, _)| r == s))
            .or_else(|| active.clone())
            .unwrap_or(Scope::Folder(dir.clone()));
        // Three icon buttons: global, this folder, this folder & below.
        ui.add_space(4.0);
        let (bar, _) = ui.allocate_exact_size(Vec2::new(w, 40.0), Sense::hover());
        ui.painter().rect_filled(bar, R_MD, v.widgets.inactive.weak_bg_fill);
        let bw = bar.width() / 3.0;
        for (i, (scope, _)) in rows.iter().take(3).enumerate() {
            let r = Rect::from_min_size(bar.min + Vec2::new(bw * i as f32, 0.0), Vec2::new(bw, bar.height())).shrink(3.0);
            let resp = ui.interact(r, ui.scope_id().with(("filter-scope", i)), Sense::click());
            let sel = *scope == pick;
            if sel {
                let (fill, stroke) = super::selected_tile(&v, v.widgets.inactive.weak_bg_fill);
                ui.painter().rect_filled(r, R_SM, fill);
                ui.painter().rect_stroke(r, R_SM, stroke, egui::StrokeKind::Inside);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, R_SM, v.widgets.hovered.weak_bg_fill);
            }
            let fg = if sel { v.strong_text_color() } else { v.text_color() };
            match i {
                0 => paint_globe(ui.painter(), r.center(), 17.0, fg),
                1 => paint_folder(ui.painter(), r.center(), 18.0, fg, false),
                _ => paint_folder(ui.painter(), r.center(), 18.0, fg, true),
            }
            // A dot when this scope has a filter set; an accent underline for the one in use.
            if self.s.filters.get(scope).is_some_and(|f| f.is_active()) {
                ui.painter().circle_filled(Pos2::new(r.right() - 7.0, r.top() + 7.0), 3.0, accent);
            }
            if active.as_ref() == Some(scope) {
                let u = Rect::from_center_size(Pos2::new(r.center().x, r.bottom() - 3.0), Vec2::new(16.0, 2.5));
                ui.painter().rect_filled(u, 1.5, accent);
            }
            let tip = match i {
                0 => "Global — for every folder that has no filter of its own".to_string(),
                1 => format!("This folder — only “{folder_name}”"),
                _ => format!("This folder & below — “{folder_name}” and every folder inside it"),
            };
            let in_use = if active.as_ref() == Some(scope) { "\nIn use here" } else { "" };
            if resp.on_hover_text(format!("{tip}{in_use}")).clicked() {
                pick = scope.clone();
            }
        }
        // A parent folder's "& below" filter that applies here can be edited too.
        if let Some((scope, label)) = rows.get(3) {
            ui.add_space(4.0);
            let sel = *scope == pick;
            let text = egui::RichText::new(format!("In use: {label}")).size(12.0).color(if sel { v.strong_text_color() } else { accent });
            if ui.add(egui::Button::new(text).frame_when_inactive(sel)).on_hover_text("A filter set on a folder above this one; click to edit it").clicked() {
                pick = scope.clone();
            }
        }
        self.filter_scope = Some(pick.clone());
        ui.add_space(10.0);

        // ---- the picked filter
        let before = self.s.filters.get(&pick).cloned();
        let mut f = before.clone().unwrap_or_default();
        let mut reset = false;
        let card = v.faint_bg_color;
        egui::Frame::NONE.fill(card).corner_radius(R_MD).inner_margin(egui::Margin::symmetric(10, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.spacing_mut().button_padding = Vec2::new(7.0, 2.0);
            // Scope name and its switch.
            ui.horizontal(|ui| {
                let title = rows.iter().find(|(s, _)| *s == pick).map(|(_, l)| l.clone()).unwrap_or_default();
                ui.add(egui::Label::new(egui::RichText::new(title).strong()).truncate());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    super::dialogs::switch(ui, &mut f.enabled, true).on_hover_text(if f.enabled { "On" } else { "Off" });
                });
            });
            let heading = |ui: &mut egui::Ui, t: &str| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(t).size(10.5).strong().color(weak));
                ui.add_space(2.0);
            };
            heading(ui, "TYPE");
            // Two even columns of chips, with the app's soft "selected" look.
            let cw = (ui.available_width() - 4.0) / 2.0;
            for pair in TypeGroup::ALL.chunks(2) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for t in pair {
                        let on = f.types.contains(t);
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(cw, 26.0), Sense::click());
                        if on {
                            let (fill, stroke) = super::selected_tile(&v, card);
                            ui.painter().rect_filled(r, R_SM, fill);
                            ui.painter().rect_stroke(r, R_SM, stroke, egui::StrokeKind::Inside);
                        } else {
                            let fill = if resp.hovered() { v.widgets.hovered.weak_bg_fill } else { v.widgets.inactive.weak_bg_fill };
                            ui.painter().rect_filled(r, R_SM, fill);
                        }
                        let fg = if on { v.strong_text_color() } else { v.text_color() };
                        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, t.label(), crate::fonts::ui_font(12.5), fg);
                        if resp.clicked() {
                            if on {
                                f.types.retain(|x| x != t);
                            } else {
                                f.types.push(*t);
                            }
                        }
                    }
                });
            }
            if !f.types.is_empty() {
                ui.add_space(2.0);
                info_note(ui, "Only the picked types show");
            }
            heading(ui, "NAME");
            // A segmented control: three equal parts.
            let (seg, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
            ui.painter().rect_filled(seg, R_SM, v.widgets.inactive.weak_bg_fill);
            let parts = [(NameMode::Contains, "Contains"), (NameMode::Excludes, "Excludes"), (NameMode::Includes, "Includes")];
            let pw = seg.width() / parts.len() as f32;
            for (i, (mode, label)) in parts.iter().enumerate() {
                let r = Rect::from_min_size(seg.min + Vec2::new(pw * i as f32, 0.0), Vec2::new(pw, seg.height())).shrink(2.0);
                let resp = ui.interact(r, ui.scope_id().with(("name-mode", i)), Sense::click());
                if f.name_mode == *mode {
                    let (fill, stroke) = super::selected_tile(&v, v.widgets.inactive.weak_bg_fill);
                    ui.painter().rect_filled(r, R_SM - 1, fill);
                    ui.painter().rect_stroke(r, R_SM - 1, stroke, egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, R_SM - 1, v.widgets.hovered.weak_bg_fill);
                }
                let fg = if f.name_mode == *mode { v.strong_text_color() } else { v.text_color() };
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *label, crate::fonts::ui_font(11.5), fg);
                if resp.clicked() {
                    f.name_mode = *mode;
                }
            }
            ui.add_space(4.0);
            let hint = match f.name_mode {
                NameMode::Includes => "*.jpg, IMG_*",
                NameMode::Excludes => "Hide names with…",
                NameMode::Contains => "Show names with…",
            };
            let bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
            egui::Frame::NONE.fill(bg).corner_radius(R_SM).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut f.name).frame(egui::Frame::NONE).hint_text(hint).align(egui::Align2::LEFT_CENTER).min_size(Vec2::new(0.0, 22.0)).desired_width(f32::INFINITY));
            });
            heading(ui, "MODIFIED");
            egui::ComboBox::from_id_salt("filter-date").selected_text(f.date.label()).width(ui.available_width() - 2.0).show_ui(ui, |ui| {
                for d in DateRange::ALL {
                    ui.selectable_value(&mut f.date, d, d.label());
                }
            });
            if before.is_some() {
                ui.add_space(6.0);
                let row_w = ui.available_width();
                ui.allocate_ui_with_layout(Vec2::new(row_w, 22.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Reset").color(weak).size(12.0)).frame_when_inactive(false)).on_hover_text("Remove this filter").clicked() {
                        reset = true;
                    }
                });
            }
        });
        if reset {
            self.s.filters.remove(&pick);
            self.s.filters.save();
            self.resort_all();
        } else if Some(&f) != before.as_ref() && (before.is_some() || f != Filter::default()) {
            *self.s.filters.get_mut(&pick) = f;
            self.s.filters.save();
            self.resort_all();
        }
        // Plain-language note on which filter is used.
        ui.add_space(8.0);
        info_note(ui, "Only one filter is used at a time. A folder's own filter comes first, then one set on a folder above it, then Global.");
        ui.add_space(8.0);
    }
}

/// A globe: a circle with an equator and a meridian.
fn paint_globe(p: &egui::Painter, c: Pos2, size: f32, color: Color32) {
    let r = size / 2.0 - 1.0;
    let st = Stroke::new(1.5, color);
    p.circle_stroke(c, r, st);
    p.line_segment([c + Vec2::new(-r, 0.0), c + Vec2::new(r, 0.0)], st);
    // The meridian as an ellipse.
    let pts: Vec<Pos2> = (0..=24).map(|k| {
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        c + Vec2::new(a.cos() * r * 0.45, a.sin() * r)
    }).collect();
    p.add(egui::Shape::line(pts, st));
}

/// A folder outline; with `plus`, a small "+" badge for "and everything below".
fn paint_folder(p: &egui::Painter, c: Pos2, size: f32, color: Color32, plus: bool) {
    let s = size / 18.0;
    let st = Stroke::new(1.5, color);
    let o = c + Vec2::new(if plus { -2.0 } else { 0.0 }, 0.0) * s;
    let pts = vec![
        o + Vec2::new(-8.0, -6.0) * s,
        o + Vec2::new(-3.0, -6.0) * s,
        o + Vec2::new(-1.5, -4.0) * s,
        o + Vec2::new(8.0, -4.0) * s,
        o + Vec2::new(8.0, 6.0) * s,
        o + Vec2::new(-8.0, 6.0) * s,
    ];
    p.add(egui::Shape::closed_line(pts, st));
    if plus {
        let b = c + Vec2::new(7.5, 5.0) * s;
        p.circle_filled(b, 5.0 * s, color);
        // The "+" in the badge contrasts with it on light and dark themes.
        let bg = if crate::palette::luminance(color) > 0.5 { Color32::from_black_alpha(230) } else { Color32::WHITE };
        let pst = Stroke::new(1.6, bg);
        p.line_segment([b + Vec2::new(-2.8, 0.0) * s, b + Vec2::new(2.8, 0.0) * s], pst);
        p.line_segment([b + Vec2::new(0.0, -2.8) * s, b + Vec2::new(0.0, 2.8) * s], pst);
    }
}

/// A quiet info box: a faint rounded background, a small "i" and wrapped text.
fn info_note(ui: &mut egui::Ui, text: &str) {
    let v = ui.visuals().clone();
    let weak = v.weak_text_color();
    let fill = crate::palette::mix(v.panel_fill, v.text_color(), 0.035);
    egui::Frame::NONE.fill(fill).corner_radius(R_SM).inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 6 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            let (r, _) = ui.allocate_exact_size(Vec2::new(12.0, 15.0), Sense::hover());
            let c = Pos2::new(r.center().x, r.top() + 7.5);
            ui.painter().circle_stroke(c, 5.5, Stroke::new(1.0, weak));
            ui.painter().circle_filled(c + Vec2::new(0.0, -2.4), 0.8, weak);
            ui.painter().line_segment([c + Vec2::new(0.0, -0.6), c + Vec2::new(0.0, 2.8)], Stroke::new(1.1, weak));
            ui.add(egui::Label::new(egui::RichText::new(text).size(11.0).color(weak)).wrap());
        });
    });
}
