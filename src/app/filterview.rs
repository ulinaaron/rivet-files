//! The Filters sidebar view and the strip above the files that says a filter is on.

use eframe::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};

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
        let Some(dir) = self.tab().loc.dir().map(|d| d.to_path_buf()) else {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Filters work in folders.").color(weak));
            return;
        };
        let active = self.s.filters.active_for(&dir).map(|(s, _)| s);
        let scopes = [Scope::Global, Scope::Folder(dir.clone()), Scope::Tree(dir.clone())];
        // Edit the scope picked before, else the one in use here, else this folder.
        let mut pick = self
            .filter_scope
            .clone()
            .filter(|s| scopes.contains(s) || matches!(s, Scope::Tree(_)))
            .or_else(|| active.clone())
            .unwrap_or(Scope::Folder(dir.clone()));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            paint_funnel(ui.painter(), ui.cursor().min + Vec2::new(8.0, 9.0), 14.0, v.text_color());
            ui.add_space(20.0);
            ui.label(egui::RichText::new("Filters").strong().size(14.0));
        });
        ui.add_space(6.0);

        // ---- which filter: global / this folder / this folder & subfolders (or a parent's)
        let mut rows: Vec<(Scope, String)> = vec![
            (Scope::Global, "Global".into()),
            (Scope::Folder(dir.clone()), "This folder".into()),
            (Scope::Tree(dir.clone()), "This folder & subfolders".into()),
        ];
        if let Some(Scope::Tree(p)) = &active {
            if *p != dir {
                rows.push((Scope::Tree(p.clone()), format!("{} (parent)", Scope::Tree(p.clone()).label())));
            }
        }
        ui.spacing_mut().item_spacing.y = 2.0;
        for (scope, label) in &rows {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
            let sel = *scope == pick;
            if sel {
                let (fill, stroke) = super::selected_tile(&v, v.panel_fill);
                ui.painter().rect_filled(r, R_SM, fill);
                ui.painter().rect_stroke(r, R_SM, stroke, egui::StrokeKind::Inside);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, R_SM, v.widgets.hovered.weak_bg_fill);
            }
            let has = self.s.filters.get(scope).is_some_and(|f| f.is_active());
            // A dot for filters that are set; "in use" for the one applying here.
            let dot_c = if has { super::accent_color(&self.s) } else { v.widgets.inactive.bg_fill };
            ui.painter().circle_filled(Pos2::new(r.left() + 12.0, r.center().y), 3.5, dot_c);
            let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), crate::fonts::ui_font(13.0), if sel { v.strong_text_color() } else { v.text_color() });
            job.wrap = egui::text::TextWrapping::truncate_at_width(r.width() - 70.0);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(Pos2::new(r.left() + 24.0, r.center().y - g.size().y / 2.0), g, v.text_color());
            if active.as_ref() == Some(scope) {
                ui.painter().text(Pos2::new(r.right() - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, "in use", crate::fonts::ui_font(11.0), super::accent_color(&self.s));
            }
            if resp.clicked() {
                pick = scope.clone();
            }
        }
        self.filter_scope = Some(pick.clone());
        ui.add_space(10.0);

        // ---- the picked filter
        let before = self.s.filters.get(&pick).cloned();
        let mut f = before.clone().unwrap_or_default();
        let card = v.faint_bg_color;
        egui::Frame::NONE.fill(card).corner_radius(R_MD).inner_margin(egui::Margin::symmetric(10, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("On").color(weak));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    super::dialogs::switch(ui, &mut f.enabled, true);
                });
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("TYPE").size(10.5).strong().color(weak));
            ui.horizontal_wrapped(|ui| {
                for t in TypeGroup::ALL {
                    let mut on = f.types.contains(&t);
                    if ui.toggle_value(&mut on, t.label()).changed() {
                        if on {
                            f.types.push(t);
                        } else {
                            f.types.retain(|x| *x != t);
                        }
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new("NAME").size(10.5).strong().color(weak));
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut f.name_mode, NameMode::Contains, "Contains");
                ui.selectable_value(&mut f.name_mode, NameMode::Excludes, "Excludes");
                ui.selectable_value(&mut f.name_mode, NameMode::Includes, "Includes");
            });
            let hint = match f.name_mode {
                NameMode::Includes => "*.jpg, IMG_*",
                _ => "Text in the name",
            };
            let bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
            egui::Frame::NONE.fill(bg).corner_radius(R_MD).inner_margin(egui::Margin::symmetric(8, 3)).show(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut f.name).frame(egui::Frame::NONE).hint_text(hint).align(egui::Align2::LEFT_CENTER).min_size(Vec2::new(0.0, 22.0)).desired_width(f32::INFINITY));
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new("MODIFIED").size(10.5).strong().color(weak));
            ui.horizontal_wrapped(|ui| {
                for d in DateRange::ALL {
                    ui.selectable_value(&mut f.date, d, d.label());
                }
            });
            ui.add_space(8.0);
            if before.is_some() && ui.button("Clear this filter").clicked() {
                self.s.filters.remove(&pick);
                f = Filter::default();
                self.s.filters.save();
                self.resort_all();
            }
        });
        if Some(&f) != before.as_ref() && (before.is_some() || f != Filter::default()) && self.s.filters.get(&pick).is_some() == before.is_some() {
            *self.s.filters.get_mut(&pick) = f;
            self.s.filters.save();
            self.resort_all();
        }
        ui.add_space(8.0);
        ui.add(egui::Label::new(egui::RichText::new("The most specific filter is used: this folder, then a parent's “& subfolders”, then global.").small().color(weak)).wrap());
    }
}
