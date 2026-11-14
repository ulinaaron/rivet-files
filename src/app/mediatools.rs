//! The Media tools windows (need ffmpeg): Convert To… and Resize…

use eframe::egui::{self, Id, Key, Vec2};
use std::path::PathBuf;

use super::{Dialog, RivetApp, r_lg, r_md};
use crate::media::{self, Keep, Size};
use crate::preview::Kind;

pub(super) struct ConvertState {
    pub files: Vec<PathBuf>,
    pub kind: Kind,
    pub ext: String,
    pub quality: u32,
    pub keep: Keep,
}

pub(super) struct ResizeState {
    /// (path, width, height)
    pub files: Vec<(PathBuf, u32, u32)>,
    pub by_percent: bool,
    pub percent: u32,
    pub width: u32,
    pub height: u32,
    pub keep: Keep,
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

/// Title on the left, a close button on the right. Returns true when close was clicked.
fn header(ui: &mut egui::Ui, title: &str, close_icon: &Option<egui::TextureHandle>) -> bool {
    let danger = super::status(ui.ctx()).danger;
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).heading());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::click());
            let resp = resp.on_hover_text("Close (Esc)");
            let fg = if resp.hovered() {
                ui.painter().circle_filled(r.center(), 13.0, danger);
                super::StatusColors::on(danger)
            } else {
                ui.visuals().text_color()
            };
            match close_icon {
                Some(t) => super::paint_tex(ui.painter(), t, egui::Rect::from_center_size(r.center(), Vec2::splat(14.0)), fg),
                None => {
                    let c = r.center();
                    let st = egui::Stroke::new(1.6, fg);
                    ui.painter().line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], st);
                    ui.painter().line_segment([c + Vec2::new(4.5, -4.5), c + Vec2::new(-4.5, 4.5)], st);
                }
            }
            clicked = resp.clicked();
        });
    });
    clicked
}

/// Cancel and a success-colored action button on the right; returns (ok, cancel).
fn footer(ui: &mut egui::Ui, note: &str, action: &str, enabled: bool) -> (bool, bool) {
    let success = super::status(ui.ctx()).success;
    let weak = ui.visuals().weak_text_color();
    let mut ok = false;
    let mut cancel = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(note).color(weak));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().button_padding = Vec2::new(14.0, 5.0);
            let btn = egui::Button::new(egui::RichText::new(action).color(if enabled { super::StatusColors::on(success) } else { weak }))
                .fill(if enabled { success } else { ui.visuals().widgets.inactive.weak_bg_fill });
            ok = ui.add_enabled(enabled, btn).clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    (ok, cancel)
}

fn card(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    let fill = ui.visuals().faint_bg_color;
    egui::Frame::NONE.fill(fill).corner_radius(r_md()).inner_margin(egui::Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui);
    });
}

fn frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(r_lg())
}

impl RivetApp {
    /// Convert To…: returns true when the window should close.
    pub(super) fn convert_dialog(&mut self, ctx: &egui::Context) -> bool {
        let close_icon = self.icon(ctx, &["window-close-symbolic"], 14.0);
        let Dialog::Convert(st) = &mut self.dialog else { return true };
        let mut ok = false;
        let mut cancel = false;
        let r = egui::Modal::new(Id::unique("convert")).frame(frame(ctx)).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
            ui.spacing_mut().button_padding = Vec2::new(10.0, 4.0);
            let weak = ui.visuals().weak_text_color();
            let what = match st.files.len() {
                1 => format!("“{}”", st.files[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                n => format!("{n} files"),
            };
            cancel |= header(ui, &format!("Convert {what}"), &close_icon);
            ui.add_space(10.0);
            card(ui, |ui| {
                egui::Grid::new("convert-opts").num_columns(2).spacing([18.0, 14.0]).min_col_width(110.0).show(ui, |ui| {
                    ui.label(egui::RichText::new("Convert to").color(weak));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
                        for (ext, label) in media::formats(st.kind) {
                            if ui.selectable_label(st.ext == *ext, *label).clicked() {
                                st.ext = ext.to_string();
                            }
                        }
                    });
                    ui.end_row();
                    if matches!(st.ext.as_str(), "jpg" | "webp" | "mp3" | "ogg") {
                        ui.label(egui::RichText::new("Quality").color(weak));
                        ui.add(egui::Slider::new(&mut st.quality, 10..=100).suffix("%"));
                        ui.end_row();
                    }
                    ui.label(egui::RichText::new("Originals").color(weak));
                    pills(ui, &mut st.keep, &[(Keep::Originals, "Keep them"), (Keep::Replace, "Replace them")]);
                    ui.end_row();
                });
                if st.keep == Keep::Replace {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Originals go to the Trash once each one is converted.").small().color(weak));
                }
            });
            ui.add_space(14.0);
            let note = format!("Saved next to the originals as .{}", st.ext);
            let (o, c) = footer(ui, &note, "Convert", !st.ext.is_empty());
            ok |= o;
            cancel |= c;
        });
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            ok = true;
        }
        if cancel || (r.should_close() && !ok) {
            return true;
        }
        if !ok {
            return false;
        }
        let st = match std::mem::replace(&mut self.dialog, Dialog::None) {
            Dialog::Convert(st) => st,
            _ => return true,
        };
        self.ops.push(media::spawn_convert(st.files, st.kind, st.ext, st.quality, st.keep));
        true
    }

    /// Resize…: returns true when the window should close.
    pub(super) fn resize_dialog(&mut self, ctx: &egui::Context) -> bool {
        let close_icon = self.icon(ctx, &["window-close-symbolic"], 14.0);
        let Dialog::Resize(st) = &mut self.dialog else { return true };
        let mut ok = false;
        let mut cancel = false;
        let size = if st.by_percent { Size::Percent(st.percent.max(1)) } else { Size::Fit(st.width.max(1), st.height.max(1)) };
        let r = egui::Modal::new(Id::unique("resize")).frame(frame(ctx)).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
            ui.spacing_mut().button_padding = Vec2::new(10.0, 4.0);
            let v = ui.visuals().clone();
            let weak = v.weak_text_color();
            let what = match st.files.len() {
                1 => format!("“{}”", st.files[0].0.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                n => format!("{n} pictures"),
            };
            cancel |= header(ui, &format!("Resize {what}"), &close_icon);
            ui.add_space(10.0);
            card(ui, |ui| {
                egui::Grid::new("resize-opts").num_columns(2).spacing([18.0, 14.0]).min_col_width(110.0).show(ui, |ui| {
                    ui.label(egui::RichText::new("Resize by").color(weak));
                    pills(ui, &mut st.by_percent, &[(true, "Percentage"), (false, "Fit within")]);
                    ui.end_row();
                    if st.by_percent {
                        ui.label(egui::RichText::new("Scale").color(weak));
                        ui.horizontal(|ui| {
                            ui.add(egui::Slider::new(&mut st.percent, 5..=200).suffix("%"));
                            for p in [25, 50, 75] {
                                if ui.selectable_label(st.percent == p, format!("{p}%")).clicked() {
                                    st.percent = p;
                                }
                            }
                        });
                    } else {
                        ui.label(egui::RichText::new("Box").color(weak));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut st.width).range(1..=20000).suffix(" px"));
                            ui.label(egui::RichText::new("×").color(weak));
                            ui.add(egui::DragValue::new(&mut st.height).range(1..=20000).suffix(" px"));
                            for (w, h) in [(1920, 1080), (1280, 1280), (800, 800)] {
                                if ui.selectable_label(st.width == w && st.height == h, format!("{w}×{h}")).clicked() {
                                    st.width = w;
                                    st.height = h;
                                }
                            }
                        });
                    }
                    ui.end_row();
                    ui.label(egui::RichText::new("Originals").color(weak));
                    pills(ui, &mut st.keep, &[(Keep::Originals, "Keep them"), (Keep::Replace, "Overwrite them")]);
                    ui.end_row();
                });
            });
            // A short preview of the new sizes.
            ui.add_space(10.0);
            ui.label(egui::RichText::new("PREVIEW").size(10.5).strong().color(weak));
            card(ui, |ui| {
                egui::ScrollArea::vertical().max_height(160.0).auto_shrink([false, true]).show(ui, |ui| {
                    egui::Grid::new("resize-preview").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                        for (p, w, h) in &st.files {
                            let (nw, nh) = size.apply(*w, *h);
                            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            ui.allocate_ui_with_layout(Vec2::new(220.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_width(220.0);
                                ui.add(egui::Label::new(egui::RichText::new(name).color(weak)).truncate());
                            });
                            ui.label(egui::RichText::new(format!("{w} × {h}")).color(weak));
                            ui.horizontal(|ui| {
                                super::arrow_label(ui, weak);
                                ui.label(egui::RichText::new(format!("{nw} × {nh}")).color(v.strong_text_color()));
                            });
                            ui.end_row();
                        }
                    });
                });
            });
            ui.add_space(14.0);
            let note = if st.keep == Keep::Replace { "Pictures are overwritten in place".to_string() } else { "New copies are named with their size".to_string() };
            let (o, c) = footer(ui, &note, "Resize", true);
            ok |= o;
            cancel |= c;
        });
        if ctx.input(|i| i.key_pressed(Key::Enter)) && !ctx.egui_wants_keyboard_input() {
            ok = true;
        }
        if cancel || (r.should_close() && !ok) {
            return true;
        }
        if !ok {
            return false;
        }
        let st = match std::mem::replace(&mut self.dialog, Dialog::None) {
            Dialog::Resize(st) => st,
            _ => return true,
        };
        let files = st.files.into_iter().map(|(p, ..)| p).collect();
        self.ops.push(media::spawn_resize(files, size, st.keep));
        true
    }

    /// Whether the media tools can be offered (enabled in Preferences and ffmpeg installed).
    pub(super) fn media_tools_on(&self) -> bool {
        self.s.media_tools && media::available()
    }
}
