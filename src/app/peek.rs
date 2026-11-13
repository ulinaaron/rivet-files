//! The Peek panel: an optional side panel showing a large preview and the details
//! of the selected item (or the current folder). Pictures show big, videos play
//! inline (muted), music shows its cover, text files show their first lines.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::SystemTime;

use super::{fmt_time, user_name, Action, RivetApp, R_MD, R_SM};
use crate::fsops;
use crate::model::Entry;
use crate::preview::{self, Kind, Player, Preview};

enum Loaded {
    File(Preview),
    /// (bytes, items, complete)
    Folder(u64, u64, bool),
}

#[derive(Default)]
pub(super) struct PeekState {
    /// What is shown: the path and its modification time (a change reloads it).
    key: Option<(PathBuf, Option<SystemTime>)>,
    rx: Option<Receiver<(PathBuf, Loaded)>>,
    preview: Option<Preview>,
    folder: Option<(u64, u64, bool)>,
    tex: Option<egui::TextureHandle>,
    player: Option<Player>,
    video_pos: f64,
    autoplay: bool,
}

impl PeekState {
    fn reset(&mut self) {
        *self = PeekState::default();
    }

    /// Stop video playback (when the panel closes or the window is busy elsewhere).
    pub(super) fn stop(&mut self) {
        if let Some(p) = self.player.take() {
            self.video_pos = p.position();
        }
    }
}

/// "rwxr-xr-x"
fn mode_string(mode: u32) -> String {
    let mut s = String::new();
    for shift in [6, 3, 0] {
        for (bit, c) in [(4, 'r'), (2, 'w'), (1, 'x')] {
            s.push(if mode >> shift & bit != 0 { c } else { '-' });
        }
    }
    s
}

impl RivetApp {
    pub(super) fn peek_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let fill = ctx.global_style().visuals.panel_fill;
        let margin = egui::Margin { left: 10, right: 12, top: 8, bottom: 10 };
        let panel = if self.s.peek_left { egui::Panel::left("peek") } else { egui::Panel::right("peek") };
        let resp = panel
            .show_separator_line(false)
            .resizable(true)
            .default_size(self.s.peek_width)
            .size_range(220.0..=640.0)
            .frame(egui::Frame::NONE.fill(fill).inner_margin(margin))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.peek_contents(ui, ctx));
            });
        let w = resp.response.rect.width();
        if (w - self.s.peek_width).abs() > 1.0 && ctx.input(|i| !i.pointer.any_down()) {
            self.s.peek_width = w;
            self.dirty_settings = true;
        }
    }

    fn peek_load(&mut self, ctx: &egui::Context, e: &Entry) {
        let key = (e.path.clone(), e.modified);
        if self.peek.key.as_ref() == Some(&key) {
            return;
        }
        self.peek.reset();
        self.peek.key = Some(key);
        let (tx, rx) = channel();
        let (path, mime, size, is_dir) = (e.path.clone(), e.mime.clone(), e.size, e.is_dir);
        let c = ctx.clone();
        std::thread::spawn(move || {
            let loaded = if is_dir {
                let (bytes, items, done) = fsops::dir_size(&path, std::time::Instant::now() + std::time::Duration::from_millis(1500));
                Loaded::Folder(bytes, items, done)
            } else {
                Loaded::File(preview::load(&path, &mime, size))
            };
            let _ = tx.send((path, loaded));
            c.request_repaint();
        });
        self.peek.rx = Some(rx);
    }

    fn peek_poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.peek.rx else { return };
        let Ok((path, loaded)) = rx.try_recv() else { return };
        self.peek.rx = None;
        if self.peek.key.as_ref().map(|k| &k.0) != Some(&path) {
            return;
        }
        match loaded {
            Loaded::Folder(b, n, done) => self.peek.folder = Some((b, n, done)),
            Loaded::File(mut p) => {
                if let Some(img) = p.image.take() {
                    self.peek.tex = Some(ctx.load_texture(format!("peek:{}", path.display()), img, egui::TextureOptions::LINEAR));
                }
                self.peek.preview = Some(p);
                // Test hook for checks: start a video as soon as its frame is ready.
                if std::env::var_os("RIVET_PEEK_PLAY").is_some() && self.peek.tex.is_some() {
                    self.peek.autoplay = true;
                }
            }
        }
    }

    fn peek_contents(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.peek_poll(ctx);
        let v = ui.visuals().clone();
        let weak = v.weak_text_color();
        let sel: Vec<Entry> = self.tab().selected_entries().into_iter().cloned().collect();
        let width = ui.available_width();
        ui.spacing_mut().item_spacing.y = 6.0;
        let card_fill = crate::palette::mix(v.panel_fill, v.extreme_bg_color, 0.55);

        if sel.len() > 1 {
            self.peek.stop();
            self.peek_many(ui, ctx, &sel, card_fill);
            return;
        }
        // One item, or the folder you're in when nothing is selected.
        let here = sel.is_empty();
        let entry = match sel.into_iter().next() {
            Some(e) => Some(e),
            None => self.current_dir().and_then(|d| Entry::from_path(d, &self.mime)),
        };
        let Some(e) = entry else {
            self.peek.stop();
            ui.add_space(30.0);
            ui.vertical_centered(|ui| ui.label(egui::RichText::new("Select an item to see its details").color(weak)));
            return;
        };
        if here {
            // The open folder: no folder-size walk (it may be your whole home).
            if self.peek.key.as_ref().map(|k| &k.0) != Some(&e.path) {
                self.peek.reset();
                self.peek.key = Some((e.path.clone(), e.modified));
            }
        } else {
            self.peek_load(ctx, &e);
        }
        let kind = if e.is_dir { Kind::Other } else { preview::kind_of(&e.mime) };

        // ---- the preview card
        let tex = self.peek.tex.clone();
        let text = self.peek.preview.as_ref().and_then(|p| p.text.clone());
        if let Some(t) = &tex {
            let [tw, th] = t.size();
            let aspect = tw as f32 / th.max(1) as f32;
            let h = (width / aspect).min(width * 1.25).max(60.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, h), if kind == Kind::Video { Sense::click() } else { Sense::hover() });
            ui.painter().rect_filled(rect, R_MD, card_fill);
            let img_w = (h * aspect).min(width);
            let img_rect = Rect::from_center_size(rect.center(), Vec2::new(img_w, img_w / aspect));
            egui::Image::from_texture(t).corner_radius(R_MD).paint_at(ui, img_rect);
            if kind == Kind::Video {
                self.peek_video_controls(ui, ctx, &e, rect, resp);
            }
        } else if let Some(text) = &text {
            egui::Frame::NONE.fill(card_fill).corner_radius(R_MD).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(width - 20.0);
                egui::ScrollArea::both().id_salt("peek-text").max_height(260.0).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.0).color(v.text_color())).wrap_mode(egui::TextWrapMode::Extend));
                });
            });
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
            ui.painter().rect_filled(rect, R_MD, card_fill);
            if let Some(t) = self.entry_icon(ctx, &e, 96.0, false) {
                super::paint_tex(ui.painter(), &t, Rect::from_center_size(rect.center(), Vec2::splat(96.0)), Color32::WHITE);
            }
            if self.peek.rx.is_some() && kind != Kind::Other {
                ui.put(Rect::from_min_size(rect.right_bottom() - Vec2::splat(26.0), Vec2::splat(18.0)), egui::Spinner::new().size(14.0));
            }
        }
        if let Some(note) = self.peek.preview.as_ref().and_then(|p| p.note.clone()) {
            ui.label(egui::RichText::new(note).small().color(weak));
        }

        // ---- name, type, actions
        ui.add_space(6.0);
        ui.add(egui::Label::new(egui::RichText::new(&e.name).size(15.0).strong()).wrap());
        let desc = if e.is_dir { "Folder".to_string() } else { self.mime.describe(&e.mime) };
        ui.label(egui::RichText::new(desc).color(weak));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = Vec2::new(12.0, 4.0);
            if !here {
                let open = egui::Button::new(egui::RichText::new("Open").color(v.selection.stroke.color)).fill(v.selection.bg_fill);
                if ui.add(open).clicked() {
                    self.pending.push(Action::Open(e.path.clone()));
                }
            }
            if ui.button("Properties").clicked() {
                self.pending.push(Action::Properties(Some(e.path.clone())));
            }
        });
        ui.add_space(10.0);

        // ---- details
        let mut rows: Vec<(String, String)> = Vec::new();
        if e.is_dir {
            if here {
                let n = self.tab().visible.len();
                rows.push(("Contents".into(), format!("{n} item{}", if n == 1 { "" } else { "s" })));
            } else {
                match self.peek.folder {
                    Some((bytes, items, done)) => rows.push((
                        "Contents".into(),
                        format!("{items} item{}, {}{}", if items == 1 { "" } else { "s" }, fsops::human_size(bytes), if done { "" } else { " or more" }),
                    )),
                    None => rows.push(("Contents".into(), match e.child_count {
                        Some(c) => format!("{c} item{}…", if c == 1 { "" } else { "s" }),
                        None => "Counting…".into(),
                    })),
                }
            }
        } else {
            rows.push(("Size".into(), fsops::human_size(e.size)));
        }
        if let Some(p) = &self.peek.preview {
            rows.extend(p.facts.iter().cloned());
        }
        if e.is_link {
            rows.push(("Link to".into(), std::fs::read_link(&e.path).map(|p| p.display().to_string()).unwrap_or_default()));
        }
        if let Some((origin, when)) = &e.trash_origin {
            rows.push(("From".into(), origin.parent().map(|p| p.display().to_string()).unwrap_or_default()));
            rows.push(("Deleted".into(), when.replace('T', " ")));
        }
        rows.push(("Modified".into(), fmt_time(e.modified)));
        if let Ok(meta) = std::fs::metadata(&e.path) {
            if let Ok(c) = meta.created() {
                rows.push(("Created".into(), fmt_time(Some(c))));
            }
            if let Ok(a) = meta.accessed() {
                rows.push(("Opened".into(), fmt_time(Some(a))));
            }
        }
        rows.push(("Permissions".into(), mode_string(e.mode)));
        rows.push(("Owner".into(), user_name(e.uid)));
        let loc = if here { e.path.clone() } else { e.path.parent().map(|p| p.to_path_buf()).unwrap_or_default() };
        let home = crate::places::home();
        let loc_text = match loc.strip_prefix(&home) {
            Ok(r) if r.as_os_str().is_empty() => "Home".to_string(),
            Ok(r) => format!("~/{}", r.display()),
            Err(_) => loc.display().to_string(),
        };
        rows.push(("Location".into(), loc_text));
        if here {
            if let Some((total, free)) = crate::places::disk_space(&e.path) {
                rows.push(("Free space".into(), format!("{} of {}", fsops::human_size(free), fsops::human_size(total))));
            }
        }
        egui::Grid::new("peek-details").num_columns(2).spacing([12.0, 7.0]).min_col_width(78.0).max_col_width(width - 90.0).show(ui, |ui| {
            for (k, val) in rows {
                ui.label(egui::RichText::new(k).color(weak));
                ui.add(egui::Label::new(val).wrap());
                ui.end_row();
            }
        });
    }

    /// Play / pause, a seek bar and the time, over and under the video's preview card.
    fn peek_video_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, e: &Entry, card: Rect, card_resp: egui::Response) {
        let duration = self.peek.preview.as_ref().and_then(|p| p.duration).unwrap_or(0.0);
        // Pick up decoded frames.
        if let Some(player) = &mut self.peek.player {
            if let Some(frame) = player.frame() {
                if let Some(t) = &mut self.peek.tex {
                    t.set(frame, egui::TextureOptions::LINEAR);
                }
            }
            self.peek.video_pos = player.position().min(duration.max(0.0));
            if player.ended {
                self.peek.player = None;
                self.peek.video_pos = 0.0;
            } else {
                ctx.request_repaint_after(Player::next_frame_in());
            }
        }
        let playing = self.peek.player.is_some();
        let mut toggle = card_resp.clicked() || std::mem::take(&mut self.peek.autoplay);
        // A round play button in the middle while paused.
        if !playing {
            let c = card.center();
            let hover = card_resp.hovered();
            ui.painter().circle_filled(c, 24.0, Color32::from_black_alpha(if hover { 190 } else { 140 }));
            let tri = vec![c + Vec2::new(-7.0, -11.0), c + Vec2::new(12.0, 0.0), c + Vec2::new(-7.0, 11.0)];
            ui.painter().add(egui::Shape::convex_polygon(tri, Color32::WHITE, Stroke::NONE));
        }
        // Under the card: play/pause, a seek bar, the time.
        let v = ui.visuals().clone();
        let mut seek: Option<f64> = None;
        ui.horizontal(|ui| {
            let (br, bresp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            if bresp.hovered() {
                ui.painter().rect_filled(br, R_SM, v.widgets.hovered.weak_bg_fill);
            }
            let c = br.center();
            let col = v.text_color();
            if playing {
                for dx in [-3.0, 3.0] {
                    ui.painter().rect_filled(Rect::from_center_size(c + Vec2::new(dx, 0.0), Vec2::new(3.0, 11.0)), 1.0, col);
                }
            } else {
                let tri = vec![c + Vec2::new(-4.0, -6.0), c + Vec2::new(6.0, 0.0), c + Vec2::new(-4.0, 6.0)];
                ui.painter().add(egui::Shape::convex_polygon(tri, col, Stroke::NONE));
            }
            toggle |= bresp.on_hover_text(if playing { "Pause" } else { "Play (muted)" }).clicked();
            let time = format!("{} / {}", preview::duration_text(self.peek.video_pos), preview::duration_text(duration));
            let tw = ui.painter().layout_no_wrap(time.clone(), egui::FontId::proportional(11.5), col).size().x;
            let bar_w = (ui.available_width() - tw - 12.0).max(30.0);
            let (bar, bar_resp) = ui.allocate_exact_size(Vec2::new(bar_w, 22.0), Sense::click_and_drag());
            let track = Rect::from_center_size(bar.center(), Vec2::new(bar.width(), 4.0));
            ui.painter().rect_filled(track, 2.0, v.widgets.inactive.bg_fill);
            let frac = if duration > 0.0 { (self.peek.video_pos / duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
            let done = Rect::from_min_max(track.min, Pos2::new(track.left() + track.width() * frac, track.bottom()));
            ui.painter().rect_filled(done, 2.0, v.selection.bg_fill);
            if bar_resp.hovered() || bar_resp.dragged() {
                ui.painter().circle_filled(Pos2::new(done.right(), track.center().y), 6.0, v.selection.bg_fill);
            }
            if bar_resp.clicked() || bar_resp.drag_stopped() {
                if let Some(p) = bar_resp.interact_pointer_pos() {
                    seek = Some((((p.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64) * duration);
                }
            }
            ui.label(egui::RichText::new(time).size(11.5).color(v.weak_text_color()));
        });
        if let Some(pos) = seek {
            self.peek.video_pos = pos;
            if playing {
                self.peek_play(ctx, e);
            }
        } else if toggle {
            if playing {
                self.peek.stop();
            } else {
                self.peek_play(ctx, e);
            }
        }
    }

    fn peek_play(&mut self, ctx: &egui::Context, e: &Entry) {
        let Some(t) = &self.peek.tex else { return };
        let [w, h] = t.size();
        let (w, h) = (w & !1, h & !1);
        self.peek.player = None;
        self.peek.player = Player::start(&e.path, self.peek.video_pos, w.max(2), h.max(2), ctx);
        ctx.request_repaint();
    }

    /// Several items: their icons fanned out, and totals.
    fn peek_many(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, sel: &[Entry], card_fill: Color32) {
        let v = ui.visuals().clone();
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
        ui.painter().rect_filled(rect, R_MD, card_fill);
        let shown: Vec<&Entry> = sel.iter().take(3).collect();
        let n = shown.len() as f32;
        for (i, e) in shown.iter().enumerate().rev() {
            let off = (i as f32 - (n - 1.0) / 2.0) * 34.0;
            if let Some(t) = self.entry_texture(ctx, e, 80.0) {
                let r = Rect::from_center_size(rect.center() + Vec2::new(off, (i as f32) * -4.0), Vec2::splat(80.0));
                super::paint_tex(ui.painter(), &t, r, Color32::WHITE);
            }
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(format!("{} items selected", sel.len())).size(15.0).strong());
        let folders = sel.iter().filter(|e| e.is_dir).count();
        let files: Vec<&Entry> = sel.iter().filter(|e| !e.is_dir).collect();
        let bytes: u64 = files.iter().map(|e| e.size).sum();
        ui.add_space(8.0);
        egui::Grid::new("peek-many").num_columns(2).spacing([12.0, 7.0]).min_col_width(78.0).show(ui, |ui| {
            let mut row = |k: &str, val: String| {
                ui.label(egui::RichText::new(k).color(v.weak_text_color()));
                ui.label(val);
                ui.end_row();
            };
            if folders > 0 {
                row("Folders", folders.to_string());
            }
            if !files.is_empty() {
                row("Files", files.len().to_string());
                row("Size", format!("{}{}", fsops::human_size(bytes), if folders > 0 { " (files only)" } else { "" }));
            }
            let newest = sel.iter().filter_map(|e| e.modified).max();
            row("Newest", fmt_time(newest));
        });
    }
}
