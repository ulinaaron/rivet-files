//! Preview: a full-window light box for pictures, videos, music covers and text.
//! Opened from the right-click menu (or Space). ← / → (or the side arrows) step through
//! the previewable files in the folder; Esc, Space or a click outside closes it.

use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use super::{Action, RivetApp, R_LG, R_MD};
use crate::fsops;
use crate::model::Entry;
use crate::preview::{self, Kind, Player, Preview};

/// Pictures in the light box are loaded up to this size (longest edge, pixels).
const QUICK_PX: u32 = 2048;
const CACHE: usize = 7;

struct Loaded {
    preview: Preview,
    tex: Option<egui::TextureHandle>,
}

pub(super) struct QuickView {
    /// The previewable files of the folder, in view order.
    items: Vec<Entry>,
    idx: usize,
    cache: HashMap<PathBuf, Option<Loaded>>,
    tx: Sender<(PathBuf, Preview)>,
    rx: Receiver<(PathBuf, Preview)>,
    player: Option<Player>,
    video_pos: f64,
    highlighted: Option<(PathBuf, egui::text::LayoutJob)>,
}

impl QuickView {
    fn current(&self) -> &Entry {
        &self.items[self.idx]
    }

    /// Ask for a file's preview (once) on a background thread.
    fn request(&mut self, ctx: &egui::Context, i: usize) {
        let Some(e) = self.items.get(i) else { return };
        if self.cache.contains_key(&e.path) {
            return;
        }
        self.cache.insert(e.path.clone(), None);
        let (path, mime, size, tx, c) = (e.path.clone(), e.mime.clone(), e.size, self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let p = preview::load_px(&path, &mime, size, QUICK_PX);
            let _ = tx.send((path, p));
            c.request_repaint();
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((path, mut p)) = self.rx.try_recv() {
            let tex = p.image.take().map(|img| ctx.load_texture(format!("quick:{}", path.display()), img, egui::TextureOptions::LINEAR));
            self.cache.insert(path, Some(Loaded { preview: p, tex }));
        }
        // Keep only the files around the current one.
        if self.cache.len() > CACHE {
            let keep: Vec<PathBuf> = (self.idx.saturating_sub(2)..(self.idx + 3).min(self.items.len())).map(|i| self.items[i].path.clone()).collect();
            self.cache.retain(|p, _| keep.contains(p));
        }
    }
}

impl RivetApp {
    /// Open Preview on `path`, stepping through the previewable files of the current view.
    pub(super) fn open_quick_view(&mut self, ctx: &egui::Context, path: PathBuf) {
        let t = self.tab();
        let items: Vec<Entry> = t
            .visible
            .iter()
            .map(|&i| &t.entries[i])
            .filter(|e| !e.is_dir && preview::kind_of(&e.mime) != Kind::Other)
            .cloned()
            .collect();
        let Some(idx) = items.iter().position(|e| e.path == path) else { return };
        let (tx, rx) = channel();
        let mut q = QuickView { items, idx, cache: HashMap::new(), tx, rx, player: None, video_pos: 0.0, highlighted: None };
        q.request(ctx, idx);
        self.quick = Some(q);
        self.peek.stop();
    }

    /// Whether this file can be shown in Preview.
    pub(super) fn quick_viewable(e: &Entry) -> bool {
        !e.is_dir && preview::kind_of(&e.mime) != Kind::Other
    }

    fn quick_go(&mut self, ctx: &egui::Context, to: usize) {
        let Some(q) = &mut self.quick else { return };
        if to >= q.items.len() || to == q.idx {
            return;
        }
        q.idx = to;
        q.player = None;
        q.video_pos = 0.0;
        let path = q.current().path.clone();
        // The file view follows along, so closing leaves the last one selected.
        let t = self.tab_mut();
        t.selected.clear();
        t.selected.insert(path.clone());
        t.scroll_to = Some(path);
        ctx.request_repaint();
    }

    /// Draw Preview (if open) over everything and handle its keys.
    pub(super) fn quick_view(&mut self, ctx: &egui::Context) {
        let Some(q) = &mut self.quick else { return };
        q.poll(ctx);
        let (idx, n) = (q.idx, q.items.len());
        // Load the current file first, then its neighbours so stepping feels instant.
        q.request(ctx, idx);
        if idx + 1 < n {
            q.request(ctx, idx + 1);
        }
        if idx > 0 {
            q.request(ctx, idx - 1);
        }

        // ---- keys (taken here so the file view doesn't also react)
        let (left, right, home, end, close) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft) || i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowRight) || i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::Home),
                i.consume_key(egui::Modifiers::NONE, Key::End),
                i.consume_key(egui::Modifiers::NONE, Key::Escape) || i.consume_key(egui::Modifiers::NONE, Key::Space),
            )
        });

        let screen = ctx.content_rect();
        let mut go: Option<usize> = None;
        let mut shut = close;
        let mut open_with = false;
        let mut toggle_play = false;
        let e = q.current().clone();
        let kind = preview::kind_of(&e.mime);
        let loaded = q.cache.get(&e.path).and_then(|l| l.as_ref());
        let tex = loaded.and_then(|l| l.tex.clone());
        let facts: Vec<(String, String)> = loaded.map(|l| l.preview.facts.clone()).unwrap_or_default();
        let text = loaded.and_then(|l| l.preview.text.clone());
        let duration = loaded.and_then(|l| l.preview.duration).unwrap_or(0.0);
        let loading = loaded.is_none();

        // Muted inline video: pick up decoded frames.
        if let Some(player) = &mut q.player {
            if let Some(frame) = player.frame() {
                if let Some(l) = q.cache.get_mut(&e.path).and_then(|l| l.as_mut()) {
                    if let Some(t) = &mut l.tex {
                        t.set(frame, egui::TextureOptions::LINEAR);
                    }
                }
            }
            q.video_pos = player.position().min(duration.max(0.0));
            if player.ended {
                q.player = None;
                q.video_pos = 0.0;
            } else {
                ctx.request_repaint_after(Player::next_frame_in());
            }
        }
        let playing = q.player.is_some();

        let highlighted = match (&text, &q.highlighted) {
            (Some(_), Some((p, job))) if *p == e.path => Some(job.clone()),
            _ => None,
        };
        let needs_highlight = text.is_some() && highlighted.is_none();

        egui::Area::new(egui::Id::unique("quick-view"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let v = ui.visuals().clone();
                ui.set_min_size(screen.size());
                // Dimmed backdrop; clicking it closes.
                let bg = ui.interact(screen, egui::Id::unique("quick-bg"), Sense::click());
                ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(215));
                if bg.clicked() {
                    shut = true;
                }
                let white = Color32::from_gray(235);
                let dim = Color32::from_gray(170);

                // ---- top: name, position and facts; close button
                let top = Rect::from_min_size(screen.min, Vec2::new(screen.width(), 64.0));
                ui.painter().text(Pos2::new(top.center().x, top.top() + 22.0), egui::Align2::CENTER_CENTER, &e.name, crate::fonts::ui_font(15.0), white);
                let mut sub = vec![format!("{} of {}", idx + 1, n)];
                if let Some((_, d)) = facts.iter().find(|(k, _)| k == "Dimensions" || k == "Resolution") {
                    sub.push(d.clone());
                }
                if let Some((_, d)) = facts.iter().find(|(k, _)| k == "Duration") {
                    sub.push(d.clone());
                }
                sub.push(fsops::human_size(e.size));
                ui.painter().text(Pos2::new(top.center().x, top.top() + 44.0), egui::Align2::CENTER_CENTER, sub.join("  ·  "), crate::fonts::ui_font(12.0), dim);
                let close_r = Rect::from_center_size(Pos2::new(top.right() - 32.0, top.top() + 30.0), Vec2::splat(34.0));
                let cresp = ui.interact(close_r, egui::Id::unique("quick-close"), Sense::click()).on_hover_text("Close (Esc)");
                if cresp.hovered() {
                    ui.painter().circle_filled(close_r.center(), 17.0, super::status(ui.ctx()).danger);
                }
                let c = close_r.center();
                let st = Stroke::new(2.0, white);
                ui.painter().line_segment([c + Vec2::new(-6.0, -6.0), c + Vec2::new(6.0, 6.0)], st);
                ui.painter().line_segment([c + Vec2::new(6.0, -6.0), c + Vec2::new(-6.0, 6.0)], st);
                if cresp.clicked() {
                    shut = true;
                }
                // "Open with …" at the bottom.
                let ob = Rect::from_center_size(Pos2::new(screen.center().x, screen.bottom() - 30.0), Vec2::new(132.0, 32.0));
                let oresp = ui.interact(ob, egui::Id::unique("quick-open"), Sense::click());
                ui.painter().rect_filled(ob, R_MD, if oresp.hovered() { Color32::from_white_alpha(46) } else { Color32::from_white_alpha(26) });
                ui.painter().text(ob.center(), egui::Align2::CENTER_CENTER, "Open", crate::fonts::ui_font(13.5), white);
                if oresp.on_hover_text("Open with the default app").clicked() {
                    open_with = true;
                }

                // ---- the stage
                let stage = Rect::from_min_max(Pos2::new(screen.left() + 84.0, top.bottom() + 6.0), Pos2::new(screen.right() - 84.0, screen.bottom() - 60.0));
                if let Some(t) = &tex {
                    let ppp = ui.ctx().pixels_per_point();
                    let size = t.size_vec2() / ppp;
                    let scale = (stage.width() / size.x).min(stage.height() / size.y).min(1.0);
                    let r = Rect::from_center_size(stage.center(), size * scale);
                    // The picture itself doesn't close the light box.
                    let iresp = ui.interact(r, egui::Id::unique("quick-image"), Sense::click());
                    egui::Image::from_texture(t).corner_radius(R_MD).paint_at(ui, r);
                    if kind == Kind::Video {
                        if !playing {
                            let c = r.center();
                            ui.painter().circle_filled(c, 34.0, Color32::from_black_alpha(if iresp.hovered() { 200 } else { 150 }));
                            let tri = vec![c + Vec2::new(-10.0, -16.0), c + Vec2::new(17.0, 0.0), c + Vec2::new(-10.0, 16.0)];
                            ui.painter().add(egui::Shape::convex_polygon(tri, white, Stroke::NONE));
                        }
                        // A thin progress line under the video.
                        if duration > 0.0 {
                            let bar = Rect::from_min_size(Pos2::new(r.left(), r.bottom() + 8.0), Vec2::new(r.width(), 3.0));
                            ui.painter().rect_filled(bar, 1.5, Color32::from_white_alpha(40));
                            let f = (q.video_pos / duration).clamp(0.0, 1.0) as f32;
                            ui.painter().rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * f, 3.0)), 1.5, v.selection.bg_fill);
                        }
                        if iresp.on_hover_text(if playing { "Pause" } else { "Play (muted)" }).clicked() {
                            toggle_play = true;
                        }
                    }
                } else if let Some(job) = &highlighted {
                    let w = stage.width().min(900.0);
                    let card = Rect::from_center_size(stage.center(), Vec2::new(w, stage.height()));
                    ui.painter().rect_filled(card, R_LG, v.extreme_bg_color);
                    let _ = ui.interact(card, egui::Id::unique("quick-text"), Sense::click());
                    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(card.shrink(18.0)));
                    egui::ScrollArea::both().id_salt("quick-text-scroll").show(&mut cui, |ui| {
                        ui.add(egui::Label::new(job.clone()).wrap_mode(egui::TextWrapMode::Extend));
                    });
                } else if loading || needs_highlight {
                    ui.put(Rect::from_center_size(stage.center(), Vec2::splat(32.0)), egui::Spinner::new().size(32.0).color(white));
                } else {
                    ui.painter().text(stage.center(), egui::Align2::CENTER_CENTER, "No preview for this file", crate::fonts::ui_font(14.0), dim);
                }

                // ---- side arrows
                for (dir, x) in [(-1i32, screen.left() + 42.0), (1, screen.right() - 42.0)] {
                    let target = idx as i32 + dir;
                    if target < 0 || target >= n as i32 {
                        continue;
                    }
                    let r = Rect::from_center_size(Pos2::new(x, stage.center().y), Vec2::splat(48.0));
                    let resp = ui.interact(r, egui::Id::unique(("quick-arrow", dir)), Sense::click());
                    ui.painter().circle_filled(r.center(), 24.0, if resp.hovered() { Color32::from_white_alpha(52) } else { Color32::from_white_alpha(22) });
                    let c = r.center() + Vec2::new(-3.0 * dir as f32, 0.0);
                    let d = dir as f32;
                    let st = Stroke::new(2.4, white);
                    ui.painter().line_segment([c + Vec2::new(-4.0 * d, -9.0), c + Vec2::new(5.0 * d, 0.0)], st);
                    ui.painter().line_segment([c + Vec2::new(5.0 * d, 0.0), c + Vec2::new(-4.0 * d, 9.0)], st);
                    if resp.on_hover_text(if dir < 0 { "Previous (←)" } else { "Next (→)" }).clicked() {
                        go = Some(target as usize);
                    }
                }
            });

        // Syntax-highlight text the first time it is shown.
        if needs_highlight {
            let v = ctx.global_style().visuals.clone();
            let (p, s, t) = self.theme_trio();
            let colors = crate::highlight::colors(v.text_color(), v.extreme_bg_color, p, s, t);
            let job = crate::highlight::layout(text.as_deref().unwrap_or(""), &e.name, &colors, egui::FontId::monospace(13.0));
            if let Some(q) = &mut self.quick {
                q.highlighted = Some((e.path.clone(), job));
            }
            ctx.request_repaint();
        }
        if toggle_play {
            if let Some(q) = &mut self.quick {
                if q.player.is_some() {
                    if let Some(p) = q.player.take() {
                        q.video_pos = p.position();
                    }
                } else if let Some([w, h]) = q.cache.get(&e.path).and_then(|l| l.as_ref()).and_then(|l| l.tex.as_ref()).map(|t| t.size()) {
                    q.player = Player::start(&e.path, q.video_pos, (w & !1).max(2), (h & !1).max(2), ctx);
                }
            }
        }
        if left && idx > 0 {
            go = Some(idx - 1);
        }
        if right && idx + 1 < n {
            go = Some(idx + 1);
        }
        if home {
            go = Some(0);
        }
        if end && n > 0 {
            go = Some(n - 1);
        }
        if let Some(i) = go {
            self.quick_go(ctx, i);
        }
        if open_with {
            self.pending.push(Action::Open(e.path.clone()));
            shut = true;
        }
        if shut {
            self.quick = None;
        }
    }
}
