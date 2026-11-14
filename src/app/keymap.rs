//! The keyboard map (F1): every shortcut, as key caps in tidy cards, with a search field.
//! Vim keys get their own card at the top while they're switched on.
//!
//! Key specs: `|` separates alternatives (shown as "or"), `/` pairs (shown as "/"),
//! `+` joins keys pressed together, and a space keys pressed one after another (`g g`).

use eframe::egui::{self, Id, Pos2, Sense, Stroke, Vec2};

use super::RivetApp;

type Group = (&'static str, &'static [(&'static str, &'static str)]);

const NAVIGATION: Group = ("Getting around", &[
    ("Up/Down/Left/Right", "Move between items"),
    ("Shift+Up/Shift+Down", "Extend the selection"),
    ("Home/End", "First / last item"),
    ("Enter", "Open"),
    ("Alt+Left/Alt+Right", "Back / forward"),
    ("Alt+Up|Backspace", "Parent folder"),
    ("Alt+Home", "Home folder"),
    ("Ctrl+L", "Type a location"),
    ("Tab", "Complete the suggested folder"),
    ("Ctrl+P", "Quick Open: go to any folder"),
]);

const FILES: Group = ("Files", &[
    ("Ctrl+C/Ctrl+X/Ctrl+V", "Copy / cut / paste"),
    ("Ctrl+Z/Ctrl+Shift+Z", "Undo / redo"),
    ("F2", "Rename (Bulk Rename with several)"),
    ("Delete", "Move to Trash"),
    ("Shift+Delete", "Delete permanently"),
    ("Ctrl+Shift+N", "New folder"),
    ("Ctrl+A", "Select all"),
    ("Esc", "Clear the selection"),
    ("Ctrl+I|Alt+Enter", "Properties"),
    ("Ctrl+D", "Bookmark this folder"),
    ("Space", "Preview (Left / Right step, Enter opens)"),
]);

const FIND: Group = ("Search and filter", &[
    ("Ctrl+F", "Filter this folder (or just type)"),
    ("Enter", "Search subfolders too"),
    ("Ctrl+Shift+F", "Filters"),
    ("Ctrl+Shift+P", "Quick Actions: every command"),
]);

const VIEW: Group = ("View", &[
    ("Ctrl+1/Ctrl+2/Ctrl+3", "Icon / list / compact view"),
    ("Ctrl+Plus/Ctrl+Minus", "Zoom in / out"),
    ("Ctrl+H", "Hidden files"),
    ("F5|Ctrl+R", "Reload"),
]);

const WINDOW: Group = ("Panels and modes", &[
    ("F7", "Terminal mode"),
    ("F8", "Status bar"),
    ("F9", "Sidebar"),
    ("F11", "Peek"),
    ("F12", "Zen mode"),
    ("Ctrl+M|F10", "Menu bar"),
]);

const APP: Group = ("Tabs and window", &[
    ("Ctrl+T/Ctrl+W", "New tab / close tab"),
    ("Ctrl+Tab/Ctrl+Shift+Tab", "Next / previous tab"),
    ("Ctrl+Comma", "Preferences"),
    ("F1", "This map"),
    ("Ctrl+Q", "Close the window"),
]);

const VIM: Group = ("Vim keys", &[
    ("j/k", "Move down / up"),
    ("h/l", "Parent folder / open"),
    ("g g/G", "First / last item"),
    ("H/L", "Back / forward"),
    ("y y", "Copy"),
    ("d d", "Cut"),
    ("p", "Paste"),
    ("Slash", "Search this folder"),
    ("Colon", "Quick Actions"),
    ("Tilde", "Home"),
]);

/// How a key name reads on its cap.
fn cap_text(k: &str) -> &str {
    match k {
        "Plus" => "+",
        "Minus" => "−",
        "Comma" => ",",
        "Slash" => "/",
        "Colon" => ":",
        "Tilde" => "~",
        other => other,
    }
}

/// Words a search can match for a spec ("ctrl c", arrows by name too).
fn spec_words(spec: &str) -> String {
    spec.replace(['|', '/', '+'], " ").to_lowercase()
}

/// One key cap. Arrow keys are drawn (not every font has arrow characters).
fn key_cap(ui: &mut egui::Ui, text: &str) {
    let v = ui.visuals().clone();
    let font = egui::FontId::monospace(12.0 * crate::fonts::scale());
    let arrow = match text {
        "Up" => Some(Vec2::new(0.0, -1.0)),
        "Down" => Some(Vec2::new(0.0, 1.0)),
        "Left" => Some(Vec2::new(-1.0, 0.0)),
        "Right" => Some(Vec2::new(1.0, 0.0)),
        _ => None,
    };
    let g = ui.painter().layout_no_wrap(if arrow.is_some() { String::new() } else { text.to_string() }, font, v.text_color());
    let w = (g.size().x + 12.0).max(22.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
    let fill = crate::palette::mix(v.window_fill, v.text_color(), if v.dark_mode { 0.10 } else { 0.06 });
    let edge = crate::palette::mix(v.window_fill, v.text_color(), 0.22);
    let rad = super::r_xs();
    // A slightly deeper bottom edge, like a real key.
    ui.painter().rect_filled(r.translate(Vec2::new(0.0, 1.5)), rad, edge);
    ui.painter().rect_filled(r, rad, fill);
    ui.painter().rect_stroke(r, rad, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    if let Some(d) = arrow {
        let c = r.center();
        let s = Stroke::new(1.4, v.text_color());
        let side = Vec2::new(-d.y, d.x);
        ui.painter().line_segment([c - d * 5.0, c + d * 5.0], s);
        ui.painter().line_segment([c + d * 5.0, c + d * 1.5 + side * 3.5], s);
        ui.painter().line_segment([c + d * 5.0, c + d * 1.5 - side * 3.5], s);
    } else {
        ui.painter().galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, v.text_color());
    }
}

/// The caps for a whole spec, with "or", "/" and "+" between them.
fn key_caps(ui: &mut egui::Ui, spec: &str) {
    let weak = ui.visuals().weak_text_color();
    let small = |ui: &mut egui::Ui, t: &str| {
        ui.label(egui::RichText::new(t).size(11.5).color(weak));
    };
    ui.spacing_mut().item_spacing.x = 3.0;
    for (ai, alt) in spec.split('|').enumerate() {
        if ai > 0 {
            small(ui, " or ");
        }
        for (pi, pair) in alt.split('/').enumerate() {
            if pi > 0 {
                small(ui, "/");
            }
            for (ci, chord) in pair.split('+').enumerate() {
                if ci > 0 {
                    small(ui, "+");
                }
                for k in chord.split(' ').filter(|k| !k.is_empty()) {
                    key_cap(ui, cap_text(k));
                }
            }
        }
    }
}

impl RivetApp {
    /// The keyboard map. Returns true when it should close.
    pub(super) fn keymap_dialog(&mut self, ctx: &egui::Context) -> bool {
        let close_icon = self.icon(ctx, &["window-close-symbolic"], 14.0);
        let vim = self.s.vim_keys;
        let screen = ctx.content_rect();
        let width = (screen.width() - 80.0).clamp(420.0, 900.0);
        let two_cols = width >= 760.0;
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(super::r_lg());
        let mut close = false;
        let r = egui::Modal::new(Id::unique("shortcuts")).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            let v = ui.visuals().clone();
            close |= super::mediatools::header(ui, "Keyboard Shortcuts", &close_icon);
            ui.add_space(10.0);
            // Search: matches what a key does and the key itself ("rename", "ctrl d").
            let field_bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
            egui::Frame::NONE.fill(field_bg).corner_radius(super::r_sm()).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                let resp = ui.add(egui::TextEdit::singleline(&mut self.keymap_filter).frame(egui::Frame::NONE).hint_text("Find a shortcut…").desired_width(f32::INFINITY));
                if ui.memory(|m| m.focused().is_none()) {
                    resp.request_focus();
                }
            });
            ui.add_space(14.0);
            let q = self.keymap_filter.trim().to_lowercase();
            let matches = |spec: &str, what: &str| q.is_empty() || what.to_lowercase().contains(&q) || spec_words(spec).contains(&q);
            let card_fill = v.faint_bg_color;
            let card = |ui: &mut egui::Ui, g: &Group, accent: bool, split: bool| -> bool {
                let rows: Vec<&(&str, &str)> = g.1.iter().filter(|(k, d)| matches(k, d)).collect();
                if rows.is_empty() {
                    return false;
                }
                let title_color = if accent { super::accent_color(&self.s) } else { v.strong_text_color() };
                ui.label(egui::RichText::new(g.0).strong().size(14.0).color(title_color));
                ui.add_space(4.0);
                egui::Frame::NONE.fill(card_fill).corner_radius(super::r_md()).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let list = |ui: &mut egui::Ui, rows: &[&(&str, &str)]| {
                        let keys_w = (ui.available_width() * 0.5).min(230.0);
                        for (k, d) in rows {
                            ui.horizontal(|ui| {
                                ui.set_min_height(34.0);
                                ui.allocate_ui_with_layout(Vec2::new(keys_w, 34.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.set_min_width(keys_w);
                                    key_caps(ui, k);
                                });
                                ui.add(egui::Label::new(*d).wrap());
                            });
                        }
                    };
                    if split && rows.len() > 1 {
                        let half = rows.len().div_ceil(2);
                        ui.columns(2, |cols| {
                            list(&mut cols[0], &rows[..half]);
                            list(&mut cols[1], &rows[half..]);
                        });
                    } else {
                        list(ui, &rows);
                    }
                });
                ui.add_space(16.0);
                true
            };
            let max_h = (screen.height() - 230.0).max(240.0);
            egui::ScrollArea::vertical().max_height(max_h).min_scrolled_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                // Keep the same height while the list narrows down, so the window doesn't jump.
                ui.set_min_height(max_h);
                let mut any = false;
                // Vim keys, when on, come first in their own card.
                if vim {
                    any |= card(ui, &VIM, true, two_cols);
                }
                let left = [NAVIGATION, FILES];
                let right = [FIND, VIEW, WINDOW, APP];
                if two_cols {
                    ui.columns(2, |cols| {
                        for g in &left {
                            any |= card(&mut cols[0], g, false, false);
                        }
                        for g in &right {
                            any |= card(&mut cols[1], g, false, false);
                        }
                    });
                } else {
                    for g in left.iter().chain(right.iter()) {
                        any |= card(ui, g, false, false);
                    }
                }
                if !any {
                    ui.label(egui::RichText::new("No matching shortcut").color(v.weak_text_color()));
                }
                if !vim && q.is_empty() {
                    ui.label(egui::RichText::new("Prefer Vim keys? Turn them on in Preferences → Behavior.").small().color(v.weak_text_color()));
                }
            });
        });
        if r.should_close() {
            close = true;
        }
        if close {
            self.keymap_filter.clear();
        }
        close
    }
}
