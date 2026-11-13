//! The Peek panel: an optional side panel showing a large preview and the details
//! of the selected item (or the current folder). Pictures show big, videos play
//! inline (muted), music shows its cover, text files show their first lines.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::SystemTime;

use super::{user_name, Action, RivetApp, R_MD, R_SM};
use crate::fsops;
use crate::model::Entry;
use crate::preview::{self, Kind, Player, Preview};

enum Loaded {
    File(Preview),
    /// (bytes, items, complete)
    Folder(u64, u64, bool, usize, usize),
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
    /// Direct children of a selected folder: (folders, files).
    counts: Option<(usize, usize)>,
    highlighted: Option<(u64, egui::text::LayoutJob)>,
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

/// 1234567 → "1,234,567"
fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "Today, 14:32", "Yesterday, 09:10" or "Wed 12 Nov 2026, 23:45".
fn fmt_full(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "—".into() };
    let dt: chrono::DateTime<chrono::Local> = t.into();
    let today = chrono::Local::now().date_naive();
    let d = dt.date_naive();
    if d == today {
        dt.format("Today, %H:%M").to_string()
    } else if today.pred_opt() == Some(d) {
        dt.format("Yesterday, %H:%M").to_string()
    } else {
        dt.format("%a %-d %b %Y, %H:%M").to_string()
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
                let (mut dirs, mut files) = (0, 0);
                for d in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                    if d.file_type().is_ok_and(|t| t.is_dir()) { dirs += 1 } else { files += 1 }
                }
                let (bytes, items, done) = fsops::dir_size(&path, std::time::Instant::now() + std::time::Duration::from_millis(1500));
                Loaded::Folder(bytes, items, done, dirs, files)
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
            Loaded::Folder(b, n, done, dirs, files) => {
                self.peek.folder = Some((b, n, done));
                self.peek.counts = Some((dirs, files));
            }
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

        // ---- the preview card (its height can be dragged with the grip under it)
        let tex = self.peek.tex.clone();
        let text = self.peek.preview.as_ref().and_then(|p| p.text.clone());
        let max_h = self.s.peek_preview_h;
        if let Some(t) = &tex {
            let [tw, th] = t.size();
            let aspect = tw as f32 / th.max(1) as f32;
            let h = (width / aspect).min(max_h).max(60.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, h), if kind == Kind::Video { Sense::click() } else { Sense::hover() });
            ui.painter().rect_filled(rect, R_MD, card_fill);
            let img_w = (h * aspect).min(width);
            let img_rect = Rect::from_center_size(rect.center(), Vec2::new(img_w, img_w / aspect));
            egui::Image::from_texture(t).corner_radius(R_MD).paint_at(ui, img_rect);
            if kind == Kind::Video {
                self.peek_video_controls(ui, ctx, &e, rect, resp);
            }
            self.peek_grip(ui, width);
        } else if let Some(text) = &text {
            let job = self.peek_highlighted(ui, &e, text);
            egui::Frame::NONE.fill(card_fill).corner_radius(R_MD).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(width - 20.0);
                egui::ScrollArea::both().id_salt("peek-text").max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                    ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
                });
            });
            self.peek_grip(ui, width);
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
            ui.painter().rect_filled(rect, R_MD, card_fill);
            if let Some(t) = self.entry_icon(ctx, &e, 96.0, false) {
                super::paint_tex(ui.painter(), &t, Rect::from_center_size(rect.center(), Vec2::splat(96.0)), Color32::WHITE);
            }
            if self.peek.rx.is_some() && kind != Kind::Other {
                ui.put(Rect::from_min_size(rect.right_bottom() - Vec2::splat(26.0), Vec2::splat(18.0)), egui::Spinner::new().size(14.0));
            }
            ui.add_space(4.0);
        }
        if let Some(note) = self.peek.preview.as_ref().and_then(|p| p.note.clone()) {
            ui.label(egui::RichText::new(note).small().color(weak));
        }

        // ---- name and a one-line summary
        ui.add_space(4.0);
        ui.add(egui::Label::new(egui::RichText::new(&e.name).size(16.0).strong()).wrap());
        let desc = if e.is_dir { "Folder".to_string() } else { self.mime.describe(&e.mime) };
        let summary = if e.is_dir {
            match (here, self.peek.folder) {
                (true, _) => format!("{desc} · {} items", self.tab().visible.len()),
                (false, Some((bytes, ..))) => format!("{desc} · {}", fsops::human_size(bytes)),
                _ => desc.clone(),
            }
        } else {
            format!("{desc} · {}", fsops::human_size(e.size))
        };
        ui.label(egui::RichText::new(summary).color(weak));
        ui.add_space(6.0);

        // ---- actions: the main "Open with …" button, then small icon buttons
        self.peek_actions(ui, ctx, &e, here, width);

        // ---- details, grouped
        let meta = std::fs::metadata(&e.path).ok();
        let mut general: Vec<(String, String)> = Vec::new();
        general.push(("Type".into(), if e.is_dir { "Folder".into() } else { format!("{desc}\n{}", e.mime) }));
        if e.is_dir {
            if here {
                let n = self.tab().visible.len();
                general.push(("Contents".into(), format!("{n} item{}", if n == 1 { "" } else { "s" })));
            } else {
                general.push(("Contents".into(), match (self.peek.folder, &self.peek.counts) {
                    (Some((bytes, items, done)), Some((dirs, files))) => format!(
                        "{dirs} folder{}, {files} file{}\n{items} items inside, {}{}",
                        if *dirs == 1 { "" } else { "s" },
                        if *files == 1 { "" } else { "s" },
                        fsops::human_size(bytes),
                        if done { "" } else { " or more" }
                    ),
                    _ => match e.child_count {
                        Some(c) => format!("{c} item{}, counting…", if c == 1 { "" } else { "s" }),
                        None => "Counting…".into(),
                    },
                }));
            }
        } else {
            general.push(("Size".into(), if e.size >= 1000 { format!("{}\n{} bytes", fsops::human_size(e.size), group_digits(e.size)) } else { fsops::human_size(e.size) }));
            if let Some(app) = crate::launch::default_app(&self.apps, &e.mime) {
                general.push(("Opens with".into(), app.name.clone()));
            }
        }
        if e.is_link {
            general.push(("Link to".into(), std::fs::read_link(&e.path).map(|p| p.display().to_string()).unwrap_or_default()));
        }
        let loc = if here { e.path.clone() } else { e.path.parent().map(|p| p.to_path_buf()).unwrap_or_default() };
        let home = crate::places::home();
        let loc_text = match loc.strip_prefix(&home) {
            Ok(r) if r.as_os_str().is_empty() => "Home".to_string(),
            Ok(r) => format!("~/{}", r.display()),
            Err(_) => loc.display().to_string(),
        };
        general.push(("Location".into(), loc_text));
        if let Some((origin, when)) = &e.trash_origin {
            general.push(("Deleted from".into(), origin.parent().map(|p| p.display().to_string()).unwrap_or_default()));
            general.push(("Deleted".into(), when.replace('T', " ")));
        }
        self.peek_section(ui, "General", general, card_fill, width);

        if let Some(p) = &self.peek.preview {
            if !p.facts.is_empty() {
                let title = match kind {
                    Kind::Image => "Picture",
                    Kind::Video => "Video",
                    Kind::Audio => "Audio",
                    Kind::Text => "Text",
                    Kind::Other => "Details",
                };
                let facts = p.facts.clone();
                self.peek_section(ui, title, facts, card_fill, width);
            }
        }

        let mut dates = vec![("Modified".to_string(), fmt_full(e.modified))];
        if let Some(m) = &meta {
            if let Ok(c) = m.created() {
                dates.push(("Created".into(), fmt_full(Some(c))));
            }
            if let Ok(a) = m.accessed() {
                dates.push(("Last opened".into(), fmt_full(Some(a))));
            }
        }
        self.peek_section(ui, "Dates", dates, card_fill, width);

        let mut access = vec![("Owner".to_string(), user_name(e.uid))];
        if let Some(m) = &meta {
            use std::os::unix::fs::MetadataExt;
            access.push(("Group".into(), super::group_name(m.gid())));
        }
        let can = |bits: u32| -> String {
            match (bits & 4 != 0, bits & 2 != 0) {
                (true, true) => "Read and write",
                (true, false) => "Read only",
                (false, true) => "Write only",
                (false, false) => "No access",
            }
            .to_string()
        };
        access.push(("Owner can".into(), can(e.mode >> 6 & 7)));
        access.push(("Others can".into(), can(e.mode & 7)));
        let exec = if e.is_dir { "" } else if e.mode & 0o111 != 0 { ", runs as a program" } else { "" };
        access.push(("Mode".into(), format!("{} ({:04o}){exec}", mode_string(e.mode), e.mode & 0o7777)));
        self.peek_section(ui, "Access", access, card_fill, width);

        if here {
            if let Some((total, free)) = crate::places::disk_space(&e.path) {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("DISK").size(10.5).strong().color(weak));
                egui::Frame::NONE.fill(card_fill).corner_radius(R_MD).inner_margin(egui::Margin::symmetric(10, 9)).show(ui, |ui| {
                    ui.set_width(width - 20.0);
                    let used = total.saturating_sub(free);
                    let frac = if total > 0 { used as f32 / total as f32 } else { 0.0 };
                    let (bar, _) = ui.allocate_exact_size(Vec2::new(width - 20.0, 6.0), Sense::hover());
                    ui.painter().rect_filled(bar, 3.0, v.widgets.inactive.bg_fill);
                    ui.painter().rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height())), 3.0, v.selection.bg_fill);
                    ui.add_space(2.0);
                    ui.label(format!("{} free of {}", fsops::human_size(free), fsops::human_size(total)));
                });
            }
        }
        ui.add_space(8.0);
    }

    /// The text preview, colored from the theme (cached until the file or colors change).
    fn peek_highlighted(&mut self, ui: &egui::Ui, e: &Entry, text: &str) -> egui::text::LayoutJob {
        use std::hash::{Hash, Hasher};
        let v = ui.visuals();
        let (p, s, t) = self.theme_trio();
        let colors = crate::highlight::colors(v.text_color(), crate::palette::mix(v.panel_fill, v.extreme_bg_color, 0.55), p, s, t);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        e.path.hash(&mut h);
        text.len().hash(&mut h);
        colors.hash(&mut h);
        let key = h.finish();
        if let Some((k, job)) = &self.peek.highlighted {
            if *k == key {
                return job.clone();
            }
        }
        let job = crate::highlight::layout(text, &e.name, &colors, egui::FontId::monospace(11.0));
        self.peek.highlighted = Some((key, job.clone()));
        job
    }

    /// A small grip under the preview: drag it to make the preview taller or shorter
    /// (double-click resets it).
    fn peek_grip(&mut self, ui: &mut egui::Ui, width: f32) {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(width, 12.0), Sense::click_and_drag());
        let resp = resp.on_hover_cursor(egui::CursorIcon::ResizeVertical).on_hover_text("Drag to resize the preview · double-click to reset");
        let v = ui.visuals();
        let col = if resp.hovered() || resp.dragged() { v.selection.bg_fill } else { v.weak_text_color().gamma_multiply(0.45) };
        ui.painter().rect_filled(Rect::from_center_size(r.center(), Vec2::new(36.0, 4.0)), 2.0, col);
        if resp.dragged() {
            let dy = resp.drag_delta().y;
            if dy != 0.0 {
                self.s.peek_preview_h = (self.s.peek_preview_h + dy).clamp(100.0, 1200.0);
                self.dirty_settings = true;
            }
        }
        if resp.double_clicked() {
            self.s.peek_preview_h = crate::settings::PEEK_PREVIEW_H;
            self.dirty_settings = true;
        }
    }

    /// "Open with <app>" as the main button, then Copy location, Rename, Properties and Trash.
    fn peek_actions(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, e: &Entry, here: bool, width: f32) {
        let v = ui.visuals().clone();
        let in_trash = e.trash_origin.is_some();
        let app = if e.is_dir { None } else { crate::launch::default_app(&self.apps, &e.mime).map(|a| (a.name.clone(), a.icon.clone())) };
        let label = match (&app, e.is_dir, here) {
            (_, true, true) => "Open in Terminal".to_string(),
            (_, true, false) => "Open folder".to_string(),
            (Some((name, _)), _, _) => format!("Open with {name}"),
            (None, _, _) => "Open".to_string(),
        };
        let app_icon = match &app {
            Some((_, icon)) if icon.starts_with('/') => self.icons.texture_from_path(ctx, std::path::Path::new(icon), (18.0 * ctx.pixels_per_point()) as u32),
            Some((_, icon)) => self.icon(ctx, &[icon.as_str(), "application-x-executable"], 18.0),
            _ if e.is_dir && here => self.icon(ctx, &["utilities-terminal", "utilities-terminal-symbolic"], 18.0),
            _ => None,
        };
        let gap = 4.0;
        // The small buttons first, on the left …
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let flat = std::mem::replace(&mut self.flat_buttons, true);
            if !in_trash {
                if self.tool_button(ui, ctx, &["edit-copy-symbolic", "edit-copy"], "⧉", "Copy location", true, false) {
                    ctx.copy_text(e.path.display().to_string());
                    self.flash("Location copied");
                }
                if !here {
                    if self.tool_button(ui, ctx, &["document-edit-symbolic", "edit-rename-symbolic", "text-editor-symbolic"], "✎", "Rename (F2)", true, false) {
                        self.pending.push(Action::Rename(e.path.clone()));
                    }
                    if self.tool_button(ui, ctx, &["user-trash-symbolic", "user-trash"], "🗑", "Move to Trash", true, false) {
                        self.pending.push(Action::Trash);
                    }
                }
            }
            if self.tool_button(ui, ctx, &["dialog-information-symbolic", "help-about-symbolic", "document-properties-symbolic"], "i", "Properties", true, false) {
                self.pending.push(Action::Properties(Some(e.path.clone())));
            }
            self.flat_buttons = flat;
        });
        // … then the main button (files and trashed items), full width below them.
        if e.is_dir && !in_trash {
            return;
        }
        let main_w = width;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if !in_trash {
                // A split button: the left part opens with the default app, the chevron on
                // the right brings up the Open With window to pick another one.
                let split = !e.is_dir;
                let (r, _) = ui.allocate_exact_size(Vec2::new(main_w, 30.0), Sense::hover());
                let chevron_w = if split { 30.0 } else { 0.0 };
                let main_r = Rect::from_min_max(r.min, Pos2::new(r.right() - chevron_w, r.bottom()));
                let chev_r = Rect::from_min_max(Pos2::new(main_r.right(), r.top()), r.max);
                let resp = ui.interact(main_r, ui.scope_id().with("peek-open"), Sense::click());
                // The "glassy" selected-tile look (accent tint + soft accent border), like the
                // active toolbar buttons; app icons stay readable on it.
                let under = ui.visuals().panel_fill;
                let (base, border) = super::selected_tile(&v, under);
                let hover = crate::palette::mix(under, v.selection.bg_fill, 0.42);
                ui.painter().rect_filled(r, R_MD, base);
                let rad = R_MD;
                if resp.hovered() {
                    let cr = if split { egui::CornerRadius { nw: rad, sw: rad, ne: 0, se: 0 } } else { egui::CornerRadius::same(rad) };
                    ui.painter().rect_filled(main_r, cr, hover);
                }
                let fg = v.text_color();
                if split {
                    let cresp = ui.interact(chev_r, ui.scope_id().with("peek-open-with"), Sense::click());
                    if cresp.hovered() {
                        ui.painter().rect_filled(chev_r, egui::CornerRadius { nw: 0, sw: 0, ne: rad, se: rad }, hover);
                    }
                    // divider
                    ui.painter().line_segment(
                        [Pos2::new(chev_r.left(), r.top() + 7.0), Pos2::new(chev_r.left(), r.bottom() - 7.0)],
                        border,
                    );
                    let c = chev_r.center() + Vec2::new(0.0, 1.0);
                    let st = Stroke::new(1.8, fg);
                    ui.painter().line_segment([c + Vec2::new(-4.5, -2.5), c + Vec2::new(0.0, 2.0)], st);
                    ui.painter().line_segment([c + Vec2::new(0.0, 2.0), c + Vec2::new(4.5, -2.5)], st);
                    if cresp.on_hover_text("Open with another application…").clicked() {
                        self.pending.push(Action::OpenWithDialog(vec![e.path.clone()]));
                    }
                }
                let r = main_r;
                let mut x = r.left() + 10.0;
                if let Some(t) = &app_icon {
                    super::paint_tex(ui.painter(), t, Rect::from_center_size(Pos2::new(x + 9.0, r.center().y), Vec2::splat(18.0)), Color32::WHITE);
                    x += 26.0;
                }
                // "Open with Image Viewer" when it fits, otherwise just "Open" (the app shows on hover).
                let font = egui::FontId::proportional(13.5);
                let fits = ui.painter().layout_no_wrap(label.clone(), font.clone(), fg).size().x <= r.right() - x - 10.0;
                let shown = if fits || e.is_dir { label.clone() } else { "Open".to_string() };
                let mut job = egui::text::LayoutJob::simple_singleline(shown, font, fg);
                job.wrap = egui::text::TextWrapping::truncate_at_width(r.right() - x - 8.0);
                let g = ui.painter().layout_job(job);
                ui.painter().galley(Pos2::new(x, r.center().y - g.size().y / 2.0), g, fg);
                ui.painter().rect_stroke(Rect::from_min_max(r.min, Pos2::new(r.right() + chevron_w, r.bottom())), R_MD, border, egui::StrokeKind::Inside);
                if resp.on_hover_text(&label).clicked() {
                    if e.is_dir && here {
                        self.pending.push(Action::Terminal(e.path.clone()));
                    } else {
                        self.pending.push(Action::Open(e.path.clone()));
                    }
                }
            } else {
                let restore = egui::Button::new(egui::RichText::new("Restore").color(v.selection.stroke.color)).fill(v.selection.bg_fill).min_size(Vec2::new(main_w, 30.0));
                if ui.add(restore).clicked() {
                    self.pending.push(Action::Restore);
                }
            }
        });
    }

    /// A titled card of label / value rows.
    fn peek_section(&mut self, ui: &mut egui::Ui, title: &str, rows: Vec<(String, String)>, card_fill: Color32, width: f32) {
        let weak = ui.visuals().weak_text_color();
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title.to_uppercase()).size(10.5).strong().color(weak));
        egui::Frame::NONE.fill(card_fill).corner_radius(R_MD).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
            ui.set_width(width - 20.0);
            let key_w = 84.0;
            egui::Grid::new(("peek", title)).num_columns(2).spacing([10.0, 6.0]).min_col_width(key_w).max_col_width((width - 20.0 - key_w - 10.0).max(60.0)).show(ui, |ui| {
                for (k, val) in rows {
                    ui.label(egui::RichText::new(k).color(weak));
                    // A second line (e.g. exact bytes, mime type) is shown smaller and dimmer.
                    match val.split_once('\n') {
                        Some((a, b)) => {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(egui::Label::new(a).wrap());
                                ui.add(egui::Label::new(egui::RichText::new(b).small().color(weak)).wrap());
                            });
                        }
                        None => {
                            ui.add(egui::Label::new(val).wrap());
                        }
                    }
                    ui.end_row();
                }
            });
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
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("{} items selected", sel.len())).size(16.0).strong());
        let folders = sel.iter().filter(|e| e.is_dir).count();
        let files: Vec<&Entry> = sel.iter().filter(|e| !e.is_dir).collect();
        let bytes: u64 = files.iter().map(|e| e.size).sum();
        let mut summary = Vec::new();
        if folders > 0 {
            summary.push(format!("{folders} folder{}", if folders == 1 { "" } else { "s" }));
        }
        if !files.is_empty() {
            summary.push(format!("{} file{}", files.len(), if files.len() == 1 { "" } else { "s" }));
        }
        ui.label(egui::RichText::new(summary.join(", ")).color(v.weak_text_color()));
        let mut rows: Vec<(String, String)> = Vec::new();
        if !files.is_empty() {
            rows.push(("Size".into(), format!("{}{}", fsops::human_size(bytes), if folders > 0 { "\nfiles only" } else { "" })));
        }
        // The most common kinds, e.g. "3 PNG image, 1 Folder".
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for e in sel {
            let d = if e.is_dir { "Folder".to_string() } else { self.mime.describe(&e.mime) };
            match kinds.iter_mut().find(|(k, _)| *k == d) {
                Some((_, n)) => *n += 1,
                None => kinds.push((d, 1)),
            }
        }
        kinds.sort_by(|a, b| b.1.cmp(&a.1));
        let kind_text: Vec<String> = kinds.iter().take(4).map(|(k, n)| format!("{n} × {k}")).collect();
        rows.push(("Kinds".into(), kind_text.join(", ")));
        let newest = sel.iter().filter_map(|e| e.modified).max();
        let oldest = sel.iter().filter_map(|e| e.modified).min();
        rows.push(("Newest".into(), fmt_full(newest)));
        rows.push(("Oldest".into(), fmt_full(oldest)));
        let width = ui.available_width();
        self.peek_section(ui, "Selection", rows, card_fill, width);
    }
}
