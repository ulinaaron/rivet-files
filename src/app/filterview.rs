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
            (Scope::Global, "Everywhere".into()),
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
        ui.spacing_mut().item_spacing.y = 2.0;
        for (scope, label) in &rows {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
            let sel = *scope == pick;
            if sel {
                let (fill, stroke) = super::selected_tile(&v, v.panel_fill);
                ui.painter().rect_filled(r, R_SM, fill);
                ui.painter().rect_stroke(r, R_SM, stroke, egui::StrokeKind::Inside);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, R_SM, v.widgets.hovered.weak_bg_fill);
            }
            // A filled dot when this scope has a filter set, a ring when it doesn't.
            let has = self.s.filters.get(scope).is_some_and(|f| f.is_active());
            let dc = Pos2::new(r.left() + 12.0, r.center().y);
            if has {
                ui.painter().circle_filled(dc, 4.0, accent);
            } else {
                ui.painter().circle_stroke(dc, 3.5, Stroke::new(1.2, weak));
            }
            let in_use = active.as_ref() == Some(scope);
            let badge_w = if in_use { 44.0 } else { 0.0 };
            let fg = if sel { v.strong_text_color() } else { v.text_color() };
            let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), crate::fonts::ui_font(13.0), fg);
            job.wrap = egui::text::TextWrapping::truncate_at_width(r.width() - 30.0 - badge_w);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(Pos2::new(r.left() + 24.0, r.center().y - g.size().y / 2.0), g, fg);
            if in_use {
                let br = Rect::from_min_size(Pos2::new(r.right() - badge_w - 4.0, r.center().y - 8.0), Vec2::new(badge_w, 16.0));
                ui.painter().rect_filled(br, 8.0, crate::palette::mix(v.panel_fill, accent, 0.35));
                ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, "in use", crate::fonts::ui_font(10.5), v.strong_text_color());
            }
            let resp = resp.on_hover_text(match scope {
                Scope::Global => "Applies in every folder without its own filter".to_string(),
                Scope::Folder(_) => format!("Only “{folder_name}”"),
                Scope::Tree(p) => format!("{} and every folder inside it", p.display()),
            });
            if resp.clicked() {
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
            ui.horizontal_wrapped(|ui| {
                for t in TypeGroup::ALL {
                    let mut on = f.types.contains(&t);
                    if ui.toggle_value(&mut on, egui::RichText::new(t.label()).size(12.5)).changed() {
                        if on {
                            f.types.push(t);
                        } else {
                            f.types.retain(|x| *x != t);
                        }
                    }
                }
            });
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
                    ui.painter().rect_filled(r, R_SM - 1, v.selection.bg_fill);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, R_SM - 1, v.widgets.hovered.weak_bg_fill);
                }
                let fg = if f.name_mode == *mode { v.selection.stroke.color } else { v.text_color() };
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *label, crate::fonts::ui_font(12.0), fg);
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
        ui.add_space(8.0);
        ui.add(egui::Label::new(egui::RichText::new("This folder's filter wins, then the nearest “& below”, then Everywhere.").size(11.0).color(weak)).wrap());
    }
}
