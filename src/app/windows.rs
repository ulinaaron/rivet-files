//! Windows that live inside the app: a title bar with roll-up, maximize and close,
//! drag to move (double-click to maximize), resize from the edges, and kept within
//! the app's area. Several can be open at once and they don't block the file view.

use eframe::egui::{self, Align2, Color32, Id, Pos2, Rect, Sense, Stroke, Vec2};

use super::paint_tex;

pub(super) struct WinState {
    pub id: u64,
    pub pos: Option<Pos2>,
    pub size: Vec2,
    pub maximized: bool,
    pub shaded: bool,
    /// Nudge for the first placement, so new windows cascade.
    pub cascade: Vec2,
}

impl WinState {
    pub fn new(id: u64, size: Vec2) -> Self {
        WinState { id, pos: None, size, maximized: false, shaded: false, cascade: Vec2::ZERO }
    }
}

/// Textures for the title bar: (window icon, roll-up, maximize/restore, close).
pub(super) struct Chrome {
    pub title: String,
    pub icon: Option<egui::TextureHandle>,
    pub shade: Option<egui::TextureHandle>,
    pub max: Option<egui::TextureHandle>,
    pub close: Option<egui::TextureHandle>,
}

pub(super) struct Shown<R> {
    pub open: bool,
    pub rect: Option<Rect>,
    pub inner: Option<R>,
}

const HEADER_H: f32 = 36.0;

/// Draw one in-app window. `area` is where it may live (the app minus a margin).
pub(super) fn show<R>(ctx: &egui::Context, st: &mut WinState, chrome: &Chrome, area: Rect, min: Vec2, add: impl FnOnce(&mut egui::Ui) -> R) -> Shown<R> {
    // Separate memory for the maximized state, so restoring brings back the old size.
    let id = Id::unique(("inner-window", st.id, st.maximized, st.shaded));
    let size = st.size.min(area.size());
    let mut pos = st.pos.unwrap_or_else(|| area.center() - size / 2.0 + st.cascade);
    // Keep at least the title bar inside the app.
    pos.x = pos.x.clamp(area.left(), (area.right() - size.x).max(area.left()));
    pos.y = pos.y.clamp(area.top(), (area.bottom() - HEADER_H).max(area.top()));
    st.pos = Some(pos);

    let style = ctx.global_style();
    let frame = egui::Frame::window(&style).inner_margin(egui::Margin::ZERO);
    let mut w = egui::Window::new("").id(id).title_bar(false).collapsible(false).movable(false).frame(frame).constrain_to(area);
    w = if st.maximized {
        w.fixed_rect(area).resizable(false)
    } else if st.shaded {
        w.current_pos(pos).resizable(false).fixed_size(Vec2::new(size.x, HEADER_H))
    } else {
        w.current_pos(pos).default_size(size).min_size(min).resizable(true)
    };

    let mut open = true;
    let mut toggle_max = false;
    let mut toggle_shade = false;
    let mut drag = Vec2::ZERO;
    let shaded = st.shaded;
    let maximized = st.maximized;
    let r = w.show(ctx, |ui| {
        ui.set_min_width(if maximized { area.width() } else { size.x.min(area.width()) });
        // ---- title bar
        let (hrect, hresp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER_H), Sense::click_and_drag());
        let v = ui.visuals().clone();
        let fg = v.text_color();
        if let Some(t) = &chrome.icon {
            paint_tex(ui.painter(), t, Rect::from_center_size(Pos2::new(hrect.left() + 20.0, hrect.center().y), Vec2::splat(16.0)), Color32::WHITE);
        }
        ui.painter().text(hrect.center(), Align2::CENTER_CENTER, &chrome.title, crate::fonts::ui_font(13.5), v.strong_text_color());
        // Pop-up windows only get a close button (no roll-up or maximize).
        let buttons: [(&Option<egui::TextureHandle>, u8); 1] = [(&chrome.close, 2)];
        let mut x = hrect.right() - 8.0;
        for (tex, kind) in buttons.iter().rev() {
            let r = Rect::from_center_size(Pos2::new(x - 12.0, hrect.center().y), Vec2::splat(24.0));
            x -= 28.0;
            let resp = ui.interact(r, ui.id().with(("winbtn", *kind)), Sense::click());
            let hover = resp.hovered();
            if hover {
                let c = if *kind == 2 { Color32::from_rgb(0xd9, 0x3f, 0x3f) } else { v.widgets.hovered.bg_fill };
                ui.painter().circle_filled(r.center(), 11.0, c);
            }
            let tint = if hover && *kind == 2 { Color32::WHITE } else { fg };
            match tex {
                Some(t) => paint_tex(ui.painter(), t, Rect::from_center_size(r.center(), Vec2::splat(14.0)), tint),
                None => {
                    let c = r.center();
                    let s = Stroke::new(1.5, tint);
                    match kind {
                        0 => {
                            ui.painter().line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], s);
                        }
                        1 => {
                            ui.painter().rect_stroke(Rect::from_center_size(c, Vec2::splat(9.0)), 1.0, s, egui::StrokeKind::Middle);
                        }
                        _ => {
                            ui.painter().line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], s);
                            ui.painter().line_segment([c + Vec2::new(4.5, -4.5), c + Vec2::new(-4.5, 4.5)], s);
                        }
                    }
                }
            }
            let tip = match kind {
                0 => if shaded { "Unroll" } else { "Roll up" },
                1 => if maximized { "Restore" } else { "Maximize" },
                _ => "Close",
            };
            if resp.on_hover_text(tip).clicked() {
                match kind {
                    0 => toggle_shade = true,
                    1 => toggle_max = true,
                    _ => open = false,
                }
            }
        }
        if hresp.dragged() && !maximized {
            drag = hresp.drag_delta();
        }
        if shaded {
            return None;
        }
        // ---- content
        let inner = egui::Frame::NONE
            .inner_margin(egui::Margin { left: 16, right: 16, top: 2, bottom: 16 })
            .show(ui, |ui| add(ui))
            .inner;
        Some(inner)
    });

    let rect = r.as_ref().map(|r| r.response.rect);
    if let Some(rect) = rect {
        if !st.maximized && !st.shaded {
            st.size = rect.size();
        }
        // Clicking anywhere on a window brings it to the front.
        if r.as_ref().is_some_and(|r| r.response.contains_pointer()) && ctx.input(|i| i.pointer.any_pressed()) {
            ctx.move_to_top(r.as_ref().unwrap().response.layer_id);
        }
    }
    if drag != Vec2::ZERO {
        st.pos = Some(pos + drag);
    }
    if toggle_max {
        st.maximized = !st.maximized;
        st.shaded = false;
    }
    if toggle_shade {
        st.shaded = !st.shaded;
        st.maximized = false;
    }
    Shown { open, rect, inner: r.and_then(|r| r.inner).flatten() }
}
