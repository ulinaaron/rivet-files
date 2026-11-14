//! Zen mode (the files fill the window; the toolbar, sidebar and Peek slide in when the
//! pointer reaches their edge) and Quick Open (Ctrl+P: a fuzzy jump-anywhere bar).

use eframe::egui::{self, Align2, Color32, Id, Key, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::chrome::{folder_matches, fuzzy_score};
use super::{paint_tex, Action, RivetApp, r_lg, r_md, r_sm};
use crate::model::Location;
use crate::places;

/// How close to an edge (in points) the pointer must come to slide a panel in.
const EDGE: f32 = ZEN_GUTTER as f32 + 2.0;
/// The empty border round the files in Zen mode: the files never sit under the reveal zones.
pub(super) const ZEN_GUTTER: i8 = 18;
/// Gap between a slid-in panel and the window edge.
const GAP: f32 = 8.0;
/// How far the pointer may stray outside a slid-in panel before it slides away.
const SLACK: f32 = 28.0;
/// How long the "you're in Zen mode" hint stays up.
const HINT_SECS: f32 = 4.5;

#[derive(Default)]
pub(super) struct ZenState {
    /// Which panels are slid in: sidebar, toolbar, Peek.
    open: [bool; 3],
    /// Where each was drawn last frame (to know when the pointer has left it).
    rects: [Option<Rect>; 3],
    /// When Zen mode was switched on (for the hint).
    pub(super) since: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// One place Quick Open can take you.
#[derive(Clone)]
struct Spot {
    loc: Location,
    name: String,
    /// Where it is, shortened with "~".
    sub: String,
    icon: Vec<&'static str>,
    /// Places and recently visited folders rank above folders found by the scan.
    bonus: i32,
}

pub(super) struct Spotlight {
    query: String,
    sel: usize,
    items: Vec<Spot>,
    shown: Vec<Spot>,
    last_query: Option<String>,
    cache: Option<(PathBuf, Vec<String>)>,
    focus: bool,
}

fn tilde(p: &Path) -> String {
    let home = places::home();
    if p == home {
        return "~".into();
    }
    match p.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

fn expand(q: &str) -> PathBuf {
    match q.strip_prefix('~') {
        Some(rest) => places::home().join(rest.trim_start_matches('/')),
        None => PathBuf::from(q),
    }
}

impl RivetApp {
    // ------------------------------------------------------------------ Zen

    pub(super) fn toggle_zen(&mut self) {
        self.s.zen = !self.s.zen;
        self.zen = ZenState { since: self.s.zen.then(Instant::now), ..Default::default() };
        self.dirty_settings = true;
    }

    /// The edges each panel lives on while in Zen mode. Peek moves to the other side when
    /// it shares one with the sidebar.
    fn zen_sides(&self) -> [Side; 3] {
        let sidebar = if self.s.sidebar_right { Side::Right } else { Side::Left };
        let toolbar = if self.s.toolbar_bottom { Side::Bottom } else { Side::Top };
        let mut peek = if self.s.peek_left { Side::Left } else { Side::Right };
        if self.s.show_sidebar && peek == sidebar {
            peek = if peek == Side::Left { Side::Right } else { Side::Left };
        }
        [sidebar, toolbar, peek]
    }

    /// Slide-in panels for Zen mode. `area` is the window below the title bar.
    pub(super) fn zen_overlays(&mut self, ctx: &egui::Context, area: Rect) {
        let sides = self.zen_sides();
        let enabled = [self.s.show_sidebar, true, self.s.show_peek];
        let ptr = ctx.input(|i| i.pointer.latest_pos().filter(|_| i.pointer.has_pointer()));
        let busy = egui::Popup::is_any_open(ctx);
        // While typing a path or searching, the toolbar stays out.
        let toolbar_busy = self.path_editing || self.search_open;
        let modal = !matches!(self.dialog, super::Dialog::None) || self.quick.is_some() || self.spotlight.is_some();
        for i in 0..3 {
            if !enabled[i] {
                self.zen.open[i] = false;
                continue;
            }
            let near = ptr.is_some_and(|p| {
                area.contains(p)
                    && match sides[i] {
                        Side::Left => p.x <= area.left() + EDGE,
                        Side::Right => p.x >= area.right() - EDGE,
                        Side::Top => p.y <= area.top() + EDGE,
                        Side::Bottom => p.y >= area.bottom() - EDGE,
                    }
            });
            let inside = match (ptr, self.zen.rects[i]) {
                (Some(p), Some(r)) => r.expand(SLACK).contains(p),
                _ => false,
            };
            let keep = (i == 1 && toolbar_busy) || (self.zen.open[i] && (inside || busy || ctx.dragged_id().is_some()));
            self.zen.open[i] = (near && !modal) || keep;
        }
        // Test hook: RIVET_ZEN_SHOW=sidebar,toolbar,peek keeps those slid in.
        if self.screenshot.is_some()
            && let Ok(v) = std::env::var("RIVET_ZEN_SHOW") {
                for (i, name) in ["sidebar", "toolbar", "peek"].iter().enumerate() {
                    if v.split(',').any(|s| s.trim() == *name) {
                        self.zen.open[i] = true;
                    }
                }
            }

        let fill = ctx.global_style().visuals.panel_fill;
        let dark = ctx.global_style().visuals.dark_mode;
        let shadow = egui::Shadow { offset: [0, 4], blur: 24, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 50 }) };
        let card = if self.s.terminal {
            egui::Frame::NONE.fill(fill).stroke(super::tui_line(&ctx.global_style().visuals))
        } else {
            egui::Frame::NONE.fill(fill).corner_radius(r_lg()).shadow(shadow)
        };
        let inner = area.shrink(GAP);
        // The side panels make room for the toolbar while it's out.
        let tb_h = self.zen.rects[1].map_or(50.0, |r| r.height());
        let tb_t = ctx.animate_bool_responsive(Id::unique(("zen-slide", 1usize)), self.zen.open[1]);
        let mut side_area = inner;
        if sides[1] == Side::Top {
            side_area.set_top(inner.top() + tb_t * (tb_h + GAP));
        } else {
            side_area.set_bottom(inner.bottom() - tb_t * (tb_h + GAP));
        }

        // Sidebar and Peek first, so the toolbar lies over their corners.
        for i in [0usize, 2, 1] {
            let t = ctx.animate_bool_responsive(Id::unique(("zen-slide", i)), self.zen.open[i]);
            if t <= 0.0 {
                self.zen.rects[i] = None;
                if i == 2 && !self.zen.open[2] && enabled[2] {
                    self.peek.stop();
                }
                continue;
            }
            let side = sides[i];
            let (size, home) = match i {
                1 => {
                    let w = inner.width();
                    let pos = if side == Side::Top { inner.left_top() } else { Pos2::new(inner.left(), inner.bottom() - tb_h) };
                    (Vec2::new(w, 0.0), pos)
                }
                _ => {
                    let w = if i == 0 { self.s.sidebar_width } else { self.s.peek_width }.min(inner.width() * 0.6);
                    let x = if side == Side::Left { inner.left() } else { inner.right() - w };
                    (Vec2::new(w, side_area.height()), Pos2::new(x, side_area.top()))
                }
            };
            // Slide in from the edge.
            let away = 1.0 - t;
            let off = match side {
                Side::Left => Vec2::new(-(size.x + GAP * 2.0) * away, 0.0),
                Side::Right => Vec2::new((size.x + GAP * 2.0) * away, 0.0),
                Side::Top => Vec2::new(0.0, -70.0 * away),
                Side::Bottom => Vec2::new(0.0, 70.0 * away),
            };
            let id = Id::unique(("zen-panel", i));
            let resp = egui::Area::new(id)
                .order(egui::Order::Middle)
                .fixed_pos(home + off)
                .constrain(false)
                .show(ctx, |ui| {
                    ui.multiply_opacity(t.clamp(0.0, 1.0));
                    match i {
                        0 => {
                            card.inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 10 }).show(ui, |ui| {
                                ui.set_width(size.x - 16.0);
                                ui.set_height(size.y - 16.0);
                                egui::ScrollArea::vertical().id_salt("zen-sidebar").auto_shrink([false, false]).show(ui, |ui| {
                                    if self.s.sidebar_tree {
                                        self.tree_contents(ui, ctx);
                                    } else {
                                        self.sidebar_contents(ui, ctx);
                                    }
                                });
                            });
                        }
                        2 => {
                            card.inner_margin(egui::Margin { left: 10, right: 12, top: 8, bottom: 10 }).show(ui, |ui| {
                                ui.set_width(size.x - 22.0);
                                ui.set_height(size.y - 18.0);
                                egui::ScrollArea::vertical().id_salt("zen-peek").auto_shrink([false, false]).show(ui, |ui| self.peek_contents(ui, ctx));
                            });
                        }
                        _ => {
                            card.inner_margin(egui::Margin::symmetric(super::chrome::TOOLBAR_SIDE_PAD, 9)).show(ui, |ui| {
                                let w = size.x - 2.0 * super::chrome::TOOLBAR_SIDE_PAD as f32;
                                ui.set_width(w);
                                ui.set_max_width(w);
                                self.toolbar_contents(ui, ctx);
                            });
                        }
                    }
                });
            self.zen.rects[i] = Some(resp.response.rect);
            if i == 1 {
                ctx.move_to_top(resp.response.layer_id);
            }
            if t < 1.0 {
                ctx.request_repaint();
            }
        }
        self.zen_hint(ctx, area);
    }

    /// "Zen mode" hint at the bottom of the window for a few seconds after switching it on.
    fn zen_hint(&mut self, ctx: &egui::Context, area: Rect) {
        let Some(since) = self.zen.since else { return };
        let age = since.elapsed().as_secs_f32();
        if age > HINT_SECS {
            self.zen.since = None;
            return;
        }
        let alpha = ((HINT_SECS - age) / 0.6).min(age / 0.25).clamp(0.0, 1.0);
        ctx.request_repaint();
        let side = |s: Side| match s {
            Side::Left => "left",
            Side::Right => "right",
            Side::Top => "top",
            Side::Bottom => "bottom",
        };
        let sides = self.zen_sides();
        let text = format!(
            "Zen mode · move to the {} edge for the toolbar{} · Ctrl+P to open any folder · F12 to leave",
            side(sides[1]),
            if self.s.show_sidebar { format!(", the {} edge for the sidebar", side(sides[0])) } else { String::new() },
        );
        egui::Area::new(Id::unique("zen-hint"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -(ctx.content_rect().bottom() - area.bottom()) - 24.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.multiply_opacity(alpha);
                let v = ui.visuals().clone();
                egui::Frame::popup(ui.style()).corner_radius(r_md()).inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.label(egui::RichText::new(text).color(v.strong_text_color()));
                });
            });
    }

    // ------------------------------------------------------------- Quick Open

    /// Remember folders as they're visited, newest first (Quick Open ranks them first).
    pub(super) fn remember_dir(&mut self, loc: &Location) {
        if let Location::Dir(p) = loc {
            self.recent_dirs.retain(|d| d != p);
            self.recent_dirs.insert(0, p.clone());
            self.recent_dirs.truncate(40);
        }
    }

    /// The search field at the top of Quick Open and Quick Actions.
    pub(super) fn palette_field(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, query: &mut String, focus: &mut bool, hint: &str) {
        let weak = ui.visuals().weak_text_color();
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            if let Some(t) = self.icon(ctx, &["edit-find-symbolic", "system-search-symbolic"], 18.0) {
                let (r, _) = ui.allocate_exact_size(Vec2::new(22.0, 36.0), Sense::hover());
                paint_tex(ui.painter(), &t, Rect::from_center_size(r.center(), Vec2::splat(18.0)), weak);
            }
            let edit = egui::TextEdit::singleline(query)
                .frame(egui::Frame::NONE)
                .font(egui::FontId::proportional(18.0))
                .align(Align2::LEFT_CENTER)
                .min_size(Vec2::new(0.0, 36.0))
                .desired_width(f32::INFINITY)
                .hint_text(hint);
            let out = edit.show(ui);
            if *focus {
                out.response.request_focus();
                // Put the cursor at the end (after Tab fills in a path).
                let mut state = out.state.clone();
                let end = egui::text::CCursor::new(query.chars().count());
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                state.store(ui.ctx(), out.response.id);
                *focus = false;
            }
        });
    }

    pub(super) fn toggle_spotlight(&mut self) {
        self.actions = None;
        if self.spotlight.is_some() {
            self.spotlight = None;
            return;
        }
        self.spotlight = Some(Spotlight {
            query: String::new(),
            sel: 0,
            items: self.spotlight_items(),
            shown: Vec::new(),
            last_query: None,
            cache: None,
            focus: true,
        });
    }

    /// Everything Quick Open offers before you type a path: recent folders, places,
    /// bookmarks, drives, and folders near here and under Home.
    fn spotlight_items(&self) -> Vec<Spot> {
        let mut out: Vec<Spot> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let name_of = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string());
        let mut push = |out: &mut Vec<Spot>, loc: Location, name: String, icon: Vec<&'static str>, bonus: i32| {
            let key = match &loc {
                Location::Dir(p) => p.display().to_string(),
                other => format!("{other:?}"),
            };
            if !seen.insert(key) {
                return;
            }
            let sub = match &loc {
                Location::Dir(p) => tilde(p),
                Location::Trash => "Deleted items".into(),
                Location::Recent => "Recently used files".into(),
                Location::Network => "Mounted network shares".into(),
                Location::Search { .. } => String::new(),
            };
            out.push(Spot { loc, name, sub, icon, bonus });
        };
        let folder_icon = |p: &Path| -> Vec<&'static str> {
            match places::special_folder_icon(p, &self.user_dirs) {
                Some(s) => vec![s, "folder"],
                None => vec!["folder"],
            }
        };
        for (i, d) in self.recent_dirs.iter().enumerate() {
            if d.is_dir() {
                push(&mut out, Location::Dir(d.clone()), if *d == places::home() { "Home".into() } else { name_of(d) }, folder_icon(d), 400 - i as i32 * 8);
            }
        }
        let home = places::home();
        push(&mut out, Location::Dir(home.clone()), "Home".into(), vec!["user-home", "folder"], 200);
        let ud = &self.user_dirs;
        for p in [&ud.desktop, &ud.documents, &ud.download, &ud.music, &ud.pictures, &ud.videos].into_iter().flatten() {
            push(&mut out, Location::Dir(p.clone()), name_of(p), folder_icon(p), 180);
        }
        for b in &self.bookmarks {
            push(&mut out, Location::Dir(b.path.clone()), b.label.clone(), folder_icon(&b.path), 180);
        }
        for m in &self.mounts {
            let icon = if m.network { vec!["folder-remote", "network-server"] } else { vec!["drive-harddisk"] };
            push(&mut out, Location::Dir(m.path.clone()), m.label.clone(), icon, 150);
        }
        push(&mut out, Location::Recent, "Recent".into(), vec!["document-open-recent", "folder-recent"], 140);
        push(&mut out, Location::Dir("/".into()), "File System".into(), vec!["drive-harddisk-system", "drive-harddisk"], 140);
        push(&mut out, Location::Trash, "Trash".into(), vec!["user-trash"], 120);
        push(&mut out, Location::Network, "Network".into(), vec!["network-workgroup", "network-server"], 100);
        // Folders inside the current one, then a shallow walk of Home.
        let subdirs = |d: &Path| -> Vec<PathBuf> {
            let mut v: Vec<PathBuf> = std::fs::read_dir(d)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path())
                .collect();
            v.sort();
            v
        };
        if let Some(cur) = self.current_dir() {
            for p in subdirs(&cur) {
                let icon = folder_icon(&p);
                push(&mut out, Location::Dir(p.clone()), name_of(&p), icon, 60);
            }
        }
        let mut level = vec![home];
        let mut reads = 0;
        for depth in 0..3 {
            let mut next = Vec::new();
            for d in level {
                reads += 1;
                if reads > 400 || out.len() > 5000 {
                    break;
                }
                for p in subdirs(&d) {
                    let icon = folder_icon(&p);
                    push(&mut out, Location::Dir(p.clone()), name_of(&p), icon, -10 * depth);
                    next.push(p);
                }
            }
            level = next;
        }
        out
    }

    /// Re-rank the list for what's typed. A query starting with "/" or "~" lists the
    /// folders at that path instead.
    fn spotlight_rank(sp: &mut Spotlight) {
        let q = sp.query.trim().to_string();
        if sp.last_query.as_deref() == Some(q.as_str()) {
            return;
        }
        sp.last_query = Some(q.clone());
        sp.sel = 0;
        if q.starts_with('/') || q.starts_with('~') {
            let (prefix, word) = match q.rfind('/') {
                Some(i) => (q[..=i].to_string(), q[i + 1..].to_string()),
                None => (format!("{q}/"), String::new()),
            };
            let mut shown = Vec::new();
            let typed = expand(&q);
            if !word.is_empty() && typed.is_dir() || q == "~" || q.ends_with('/') && expand(&prefix).is_dir() {
                let p = if q.ends_with('/') { expand(&prefix) } else { typed };
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| if q == "~" { "Home".into() } else { p.display().to_string() });
                shown.push(Spot { sub: tilde(&p), loc: Location::Dir(p), name, icon: vec!["folder-open", "folder"], bonus: 0 });
            }
            let base = expand(&prefix);
            for n in folder_matches(&prefix, &word, &mut sp.cache, 30) {
                let p = base.join(&n);
                if shown.iter().any(|s| s.loc == Location::Dir(p.clone())) {
                    continue;
                }
                shown.push(Spot { sub: tilde(&p), loc: Location::Dir(p), name: n, icon: vec!["folder"], bonus: 0 });
            }
            sp.shown = shown;
            return;
        }
        if q.is_empty() {
            // Recent folders and places, in their own order.
            sp.shown = sp.items.iter().filter(|s| s.bonus >= 100).take(40).cloned().collect();
            return;
        }
        let mut scored: Vec<(i32, &Spot)> = sp
            .items
            .iter()
            .filter_map(|s| {
                let by_name = fuzzy_score(&s.name, &q);
                // "doc/proj" style queries match against the whole path.
                let by_path = if q.contains('/') { fuzzy_score(&s.sub, &q).map(|v| v - 400) } else { None };
                by_name.max(by_path).map(|v| (v + s.bonus / 2, s))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.sub.len().cmp(&b.1.sub.len())));
        sp.shown = scored.into_iter().take(40).map(|(_, s)| s.clone()).collect();
    }

    pub(super) fn spotlight(&mut self, ctx: &egui::Context) {
        if self.spotlight.is_none() {
            // Test hook: RIVET_SPOTLIGHT=1 (or a query) opens it on the first frame.
            if self.screenshot.is_some() && self.frames == 1
                && let Ok(q) = std::env::var("RIVET_SPOTLIGHT") {
                    self.toggle_spotlight();
                    if let Some(sp) = &mut self.spotlight {
                        sp.query = if q == "1" { String::new() } else { q };
                    }
                }
            if self.spotlight.is_none() {
                return;
            }
        }
        // The keys Quick Open uses, taken before the text field sees them.
        let (up, down, enter, tab, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Tab),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
            )
        });
        let mut sp = self.spotlight.take().unwrap();
        Self::spotlight_rank(&mut sp);
        let n = sp.shown.len();
        if n > 0 {
            if down {
                sp.sel = (sp.sel + 1) % n;
            }
            if up {
                sp.sel = (sp.sel + n - 1) % n;
            }
        }
        let mut go: Option<Location> = None;
        let mut close = esc;
        if enter {
            match sp.shown.get(sp.sel) {
                Some(s) => go = Some(s.loc.clone()),
                None => {
                    let p = expand(sp.query.trim());
                    if p.is_dir() {
                        go = Some(Location::Dir(p));
                    }
                }
            }
        }
        // Tab fills in the highlighted folder, to keep typing below it.
        if tab
            && let Some(Location::Dir(p)) = sp.shown.get(sp.sel).map(|s| s.loc.clone()) {
                let mut t = tilde(&p);
                if !t.ends_with('/') {
                    t.push('/');
                }
                sp.query = t;
                sp.focus = true;
                Self::spotlight_rank(&mut sp);
            }

        let screen = ctx.content_rect();
        let w = 600f32.min(screen.width() - 40.0);
        let top = (screen.height() * 0.16).max(30.0);
        let id = Id::unique("spotlight");
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(8)).corner_radius(r_lg());
        let area = egui::Modal::default_area(id).anchor(Align2::CENTER_TOP, Vec2::new(0.0, top));
        let backdrop = Color32::from_black_alpha(if ctx.global_style().visuals.dark_mode { 90 } else { 50 });
        let r = egui::Modal::new(id).area(area).frame(frame).backdrop_color(backdrop).show(ctx, |ui| {
            ui.set_width(w);
            let v = ui.visuals().clone();
            self.palette_field(ui, ctx, &mut sp.query, &mut sp.focus, "Go to a folder…  (or type a path: / or ~)");
            if !sp.shown.is_empty() {
                ui.add_space(4.0);
                let (sep, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().hline(sep.x_range(), sep.center().y, Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color));
                ui.add_space(4.0);
            }
            let row_h = 44.0;
            egui::ScrollArea::vertical().max_height(row_h * 8.0 + 4.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let shown = sp.shown.clone();
                for (i, s) in shown.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                        sp.sel = i;
                    }
                    let active = i == sp.sel;
                    if active {
                        let (fill, stroke) = super::selected_tile(&v, v.window_fill);
                        ui.painter().rect_filled(rect, r_sm(), fill);
                        ui.painter().rect_stroke(rect, r_sm(), stroke, egui::StrokeKind::Inside);
                        if up || down {
                            ui.scroll_to_rect(rect, None);
                        }
                    }
                    if let Some(t) = self.icon(ctx, &s.icon, 24.0) {
                        paint_tex(ui.painter(), &t, Rect::from_center_size(Pos2::new(rect.left() + 22.0, rect.center().y), Vec2::splat(24.0)), Color32::WHITE);
                    }
                    let x = rect.left() + 44.0;
                    let maxw = rect.right() - x - 10.0;
                    let mut job = egui::text::LayoutJob::simple_singleline(s.name.clone(), crate::fonts::ui_font(14.0), v.strong_text_color());
                    job.wrap = egui::text::TextWrapping::truncate_at_width(maxw);
                    let g = ui.painter().layout_job(job);
                    ui.painter().galley(Pos2::new(x, rect.top() + 5.0), g, v.strong_text_color());
                    let mut job = egui::text::LayoutJob::simple_singleline(s.sub.clone(), crate::fonts::ui_font(12.0), v.weak_text_color());
                    job.wrap = egui::text::TextWrapping::truncate_at_width(maxw);
                    let g = ui.painter().layout_job(job);
                    ui.painter().galley(Pos2::new(x, rect.bottom() - 6.0 - g.size().y), g, v.weak_text_color());
                    if resp.clicked() {
                        go = Some(s.loc.clone());
                    }
                }
            });
            if sp.shown.is_empty() && !sp.query.trim().is_empty() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("No matching folders").color(v.weak_text_color()));
                });
                ui.add_space(4.0);
            }
            // A quiet key legend.
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Up/Down choose   ·   Enter open   ·   Tab fill in   ·   Esc close").size(11.5).color(v.weak_text_color()));
            });
        });
        if r.backdrop_response.clicked() {
            close = true;
        }
        if let Some(loc) = go {
            self.pending.push(Action::Navigate(loc));
            close = true;
        }
        if !close {
            self.spotlight = Some(sp);
        }
    }
}
