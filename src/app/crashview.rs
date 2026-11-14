//! The "Rivet closed unexpectedly" dialog, shown once after a crash (see `crate::crash`).

use eframe::egui::{self, Id};

use super::{Dialog, RivetApp};

impl RivetApp {
    /// Returns true when the dialog should close.
    pub(super) fn crash_dialog(&mut self, ctx: &egui::Context) -> bool {
        let close_icon = self.icon(ctx, &["window-close-symbolic"], 14.0);
        let Dialog::Crash(report, copied) = &mut self.dialog else { return true };
        let mut close = false;
        let mut open_folder = false;
        let mut open_issue = false;
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(super::r_lg());
        let width = (ctx.content_rect().width() - 80.0).clamp(380.0, 620.0);
        let r = egui::Modal::new(Id::unique("crash-report")).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            let v = ui.visuals().clone();
            let title = if report.closed { "Rivet closed unexpectedly" } else { "Rivet ran into a problem" };
            close |= super::mediatools::header(ui, title, &close_icon);
            ui.add_space(8.0);
            ui.add(egui::Label::new("A report was saved on this computer. Nothing has been sent anywhere.").wrap());
            ui.add_space(2.0);
            ui.add(egui::Label::new(egui::RichText::new("Sharing it helps get this fixed. It can include file or folder names, so look it over first.").color(v.weak_text_color())).wrap());
            ui.add_space(12.0);
            // The report itself, selectable.
            egui::Frame::NONE.fill(v.faint_bg_color).corner_radius(super::r_md()).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::both().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&report.text).monospace().size(11.5)).selectable(true).extend());
                });
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button(if *copied { "Copied" } else { "Copy Report" }).clicked() {
                    ui.ctx().copy_text(report.text.clone());
                    *copied = true;
                }
                if ui.button("Open Folder").clicked() {
                    open_folder = true;
                }
                if ui.button("Report on GitHub").on_hover_text("Copies the report and opens a new issue; paste it in").clicked() {
                    ui.ctx().copy_text(report.text.clone());
                    *copied = true;
                    open_issue = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });
        if r.should_close() {
            close = true;
        }
        let Dialog::Crash(report, _) = &self.dialog else { return true };
        if open_folder {
            let _ = crate::launch::open_url(&crate::crash::dir().to_string_lossy());
        }
        if open_issue {
            let title = format!("Crash in {} at {}", env!("CARGO_PKG_VERSION"), report.place);
            let url = format!("{}/issues/new?title={}", env!("CARGO_PKG_REPOSITORY"), crate::places::percent_encode_path(std::path::Path::new(&title)));
            let _ = crate::launch::open_url(&url);
        }
        close
    }
}
