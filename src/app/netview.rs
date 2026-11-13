//! The Network page: servers found on the local network, the shares on a server,
//! connected shares, and "Connect to Server…". All slow work (finding servers,
//! listing shares, connecting) runs on background threads.

use eframe::egui::{self, Color32, Id, Key, Pos2, Rect, Sense, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use super::{paint_tex, Action, RivetApp, R_LG, R_MD, R_SM};
use crate::model::Location;
use crate::network::{self, Login, MountError, NetPlace};

/// What a finished background job hands back.
pub(super) enum NetResult {
    Found(Vec<NetPlace>),
    Shares(NetPlace, Result<Vec<NetPlace>, String>),
    Mounted(#[allow(dead_code)] NetPlace, Result<PathBuf, String>),
    /// The server wants a login before it shows this place.
    NeedsLogin(NetPlace, bool),
}

#[derive(Default)]
pub(super) struct NetState {
    pub servers: Vec<NetPlace>,
    pub scanned: bool,
    pub scanning: bool,
    /// The server whose shares are shown, and its shares.
    pub open_server: Option<(NetPlace, Vec<NetPlace>)>,
    pub busy: Option<String>,
    pub rx: Option<Receiver<NetResult>>,
    pub error: Option<String>,
}

/// The "connect" / "log in" window.
pub(super) struct ConnectAsk {
    pub address: String,
    pub login: Login,
    /// Reached by clicking a server/share that asked for a login (address fixed).
    pub for_place: Option<(NetPlace, bool)>,
    pub error: Option<String>,
    pub focus: bool,
}

impl RivetApp {
    fn net_job(&mut self, ctx: &egui::Context, busy: String, job: impl FnOnce() -> NetResult + Send + 'static) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
        self.net.busy = Some(busy);
        self.net.rx = Some(rx);
        self.net.error = None;
    }

    pub(super) fn net_scan(&mut self, ctx: &egui::Context) {
        if self.net.scanning {
            return;
        }
        self.net.scanning = true;
        let (tx, rx) = channel();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(NetResult::Found(network::discover()));
            c.request_repaint();
        });
        // Scans run beside other jobs; keep them on their own receiver.
        self.net_scan_rx = Some(rx);
    }

    /// Open a server or share: list an SMB server's shares, otherwise connect and go there.
    pub(super) fn net_open(&mut self, ctx: &egui::Context, place: NetPlace, login: Option<Login>) {
        if place.lists_shares() {
            let p = place.clone();
            let login2 = login.clone();
            self.net_job(ctx, format!("Looking at {}…", place.name), move || {
                // Browsing an SMB server needs it "mounted" (logged in) first.
                match network::mount(&p.uri, login2.as_ref()) {
                    Err(MountError::NeedsLogin) => return NetResult::NeedsLogin(p, true),
                    Err(MountError::Failed(_)) | Ok(()) => {}
                }
                NetResult::Shares(p.clone(), network::list_shares(&p))
            });
        } else {
            let p = place.clone();
            self.net_job(ctx, format!("Connecting to {}…", place.name), move || match network::mount(&p.uri, login.as_ref()) {
                Err(MountError::NeedsLogin) => NetResult::NeedsLogin(p, false),
                Err(MountError::Failed(e)) => NetResult::Mounted(p, Err(e)),
                Ok(()) => {
                    // gvfs needs a moment to expose the folder.
                    let mut folder = None;
                    for _ in 0..30 {
                        folder = network::folder_for(&p.uri);
                        if folder.as_ref().is_some_and(|f| f.exists()) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    NetResult::Mounted(p.clone(), folder.ok_or_else(|| "Connected, but the folder isn't available yet.".to_string()))
                }
            });
        }
    }

    /// Collect finished background work.
    pub(super) fn net_poll(&mut self) {
        if let Some(rx) = &self.net_scan_rx {
            if let Ok(NetResult::Found(list)) = rx.try_recv() {
                self.net.servers = list;
                self.net.scanned = true;
                self.net.scanning = false;
                self.net_scan_rx = None;
            }
        }
        let Some(rx) = &self.net.rx else { return };
        let Ok(res) = rx.try_recv() else { return };
        self.net.rx = None;
        self.net.busy = None;
        match res {
            NetResult::Found(_) => {}
            NetResult::Shares(server, Ok(shares)) => self.net.open_server = Some((server, shares)),
            NetResult::Shares(_, Err(e)) => self.net.error = Some(e),
            NetResult::Mounted(_place, Ok(folder)) => {
                self.mounts = crate::places::mounts();
                self.pending.push(Action::Navigate(Location::Dir(folder)));
            }
            NetResult::Mounted(_, Err(e)) => self.net.error = Some(e),
            NetResult::NeedsLogin(place, _shares) => {
                self.connect_ask = Some(ConnectAsk {
                    address: place.uri.clone(),
                    login: Login { user: std::env::var("USER").unwrap_or_default(), ..Default::default() },
                    for_place: Some((place, _shares)),
                    error: None,
                    focus: true,
                });
            }
        }
    }

    pub(super) fn open_connect_dialog(&mut self) {
        self.connect_ask = Some(ConnectAsk {
            address: String::new(),
            login: Login { user: std::env::var("USER").unwrap_or_default(), ..Default::default() },
            for_place: None,
            error: None,
            focus: true,
        });
    }

    /// The Network page shown in place of the file view.
    pub(super) fn network_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.net.scanned && !self.net.scanning {
            self.net_scan(ctx);
        }
        let v = ui.visuals().clone();
        let weak = v.weak_text_color();
        let mut open: Option<NetPlace> = None;
        let mut go: Option<PathBuf> = None;
        let mut eject: Option<PathBuf> = None;
        let connected = network::connected();
        let server_icon = self.icon(ctx, &["network-server", "folder-remote"], 32.0);
        let share_icon = self.icon(ctx, &["folder-remote", "network-server"], 32.0);
        let eject_icon = self.icon(ctx, &["media-eject-symbolic", "media-eject"], 16.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                let title = match &self.net.open_server {
                    Some((s, _)) => format!("Shares on {}", s.name),
                    None => "Network".to_string(),
                };
                ui.label(egui::RichText::new(title).heading());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    if ui.button("Connect to Server…").clicked() {
                        self.open_connect_dialog();
                    }
                    if self.net.open_server.is_some() {
                        if ui.button("All servers").clicked() {
                            self.net.open_server = None;
                        }
                    } else if ui.add_enabled(!self.net.scanning, egui::Button::new("Refresh")).clicked() {
                        self.net.scanned = false;
                    }
                    if self.net.busy.is_some() || self.net.scanning {
                        ui.spinner();
                    }
                });
            });
            if let Some(b) = &self.net.busy {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(b).color(weak));
                });
            }
            if let Some(e) = &self.net.error {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(e).color(super::status(ui.ctx()).danger));
                });
            }
            ui.add_space(8.0);

            // A tile grid like the icon view.
            let tile = |ui: &mut egui::Ui, tex: &Option<egui::TextureHandle>, title: &str, sub: &str, id: Id| -> egui::Response {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(250.0, 58.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, R_SM, v.widgets.hovered.weak_bg_fill);
                }
                let ir = Rect::from_center_size(Pos2::new(rect.left() + 28.0, rect.center().y), Vec2::splat(32.0));
                if let Some(t) = tex {
                    paint_tex(ui.painter(), t, ir, Color32::WHITE);
                }
                let mut job = egui::text::LayoutJob::simple_singleline(title.to_string(), crate::fonts::ui_font(13.5), v.strong_text_color());
                job.wrap = egui::text::TextWrapping::truncate_at_width(rect.right() - ir.right() - 18.0);
                let g = ui.painter().layout_job(job);
                ui.painter().galley(Pos2::new(ir.right() + 12.0, rect.center().y - 15.0), g, v.strong_text_color());
                let mut job = egui::text::LayoutJob::simple_singleline(sub.to_string(), crate::fonts::ui_font(11.5), weak);
                job.wrap = egui::text::TextWrapping::truncate_at_width(rect.right() - ir.right() - 18.0);
                let g = ui.painter().layout_job(job);
                ui.painter().galley(Pos2::new(ir.right() + 12.0, rect.center().y + 3.0), g, weak);
                let _ = id;
                resp
            };
            let section = |ui: &mut egui::Ui, text: &str| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new(text).small().strong().color(weak));
                });
                ui.add_space(2.0);
            };

            if let Some((_, shares)) = self.net.open_server.clone() {
                section(ui, "Shares");
                ui.horizontal_wrapped(|ui| {
                    if shares.is_empty() {
                        ui.add_space(14.0);
                    ui.label(egui::RichText::new("This server has no shared folders you can see.").color(weak));
                    }
                    for s in &shares {
                        if tile(ui, &share_icon, &s.name, "Shared folder", Id::unique(&s.uri)).on_hover_text(&s.uri).clicked() {
                            open = Some(s.clone());
                        }
                    }
                });
            } else {
                section(ui, "On this network");
                ui.horizontal_wrapped(|ui| {
                    if self.net.servers.is_empty() {
                        let msg = if self.net.scanning { "Looking for servers…" } else { "No servers found. Use “Connect to Server…” to type an address." };
                        ui.add_space(14.0);
                    ui.label(egui::RichText::new(msg).color(weak));
                    }
                    for s in self.net.servers.clone() {
                        let kind = match s.scheme() {
                            "smb" => "File sharing (SMB)",
                            "sftp" => "Remote login (SFTP)",
                            "ftp" => "FTP",
                            "dav" | "davs" => "WebDAV",
                            "nfs" => "NFS",
                            other => other,
                        }
                        .to_string();
                        if tile(ui, &server_icon, &s.name, &kind, Id::unique(&s.uri)).on_hover_text(&s.uri).clicked() {
                            open = Some(s.clone());
                        }
                    }
                });
            }

            section(ui, "Connected");
            ui.horizontal_wrapped(|ui| {
                let mut any = false;
                for (name, folder) in &connected {
                    any = true;
                    let resp = tile(ui, &share_icon, name, "Connected", Id::unique(folder));
                    // An eject button on the tile.
                    let er = Rect::from_center_size(Pos2::new(resp.rect.right() - 18.0, resp.rect.center().y), Vec2::splat(24.0));
                    let eresp = ui.interact(er, Id::unique(("net-eject", folder)), Sense::click()).on_hover_text("Disconnect");
                    if eresp.hovered() {
                        ui.painter().circle_filled(er.center(), 12.0, v.widgets.hovered.bg_fill);
                    }
                    if let Some(t) = &eject_icon {
                        paint_tex(ui.painter(), t, Rect::from_center_size(er.center(), Vec2::splat(14.0)), v.text_color());
                    }
                    if eresp.clicked() {
                        eject = Some(folder.clone());
                    } else if resp.clicked() {
                        go = Some(folder.clone());
                    }
                }
                for m in self.mounts.iter().filter(|m| m.network) {
                    any = true;
                    if tile(ui, &share_icon, &m.label, &m.path.display().to_string(), Id::unique(&m.path)).clicked() {
                        go = Some(m.path.clone());
                    }
                }
                if !any {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Nothing connected yet.").color(weak));
                }
            });
            ui.add_space(12.0);
        });

        if let Some(p) = open {
            self.net_open(ctx, p, None);
        }
        if let Some(f) = go {
            self.pending.push(Action::Navigate(Location::Dir(f)));
        }
        if let Some(f) = eject {
            match network::unmount_folder(&f) {
                Ok(()) => self.flash("Disconnected"),
                Err(e) => self.message("Could not disconnect", &e),
            }
        }
    }

    /// "Connect to Server…" and the login window for servers that ask for one.
    pub(super) fn connect_dialog(&mut self, ctx: &egui::Context) {
        let Some(ask) = &mut self.connect_ask else { return };
        let mut submit = false;
        let mut cancel = false;
        let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(egui::Margin::same(22)).corner_radius(R_LG);
        let r = egui::Modal::new(Id::unique("connect")).frame(frame).show(ctx, |ui| {
            ui.set_width(420.0);
            let v = ui.visuals().clone();
            let title = match &ask.for_place {
                Some((p, _)) => format!("Log in to {}", p.name),
                None => "Connect to Server".to_string(),
            };
            ui.label(egui::RichText::new(title).heading());
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if ask.for_place.is_some() {
                    "This server needs a user name and password."
                } else {
                    "For example smb://nas.local/share, sftp://server, ftp://host or dav://host/path"
                })
                .color(v.weak_text_color()),
            );
            ui.add_space(12.0);
            let field_bg = crate::palette::mix(v.window_fill, v.text_color(), 0.06);
            let field = |ui: &mut egui::Ui, label: &str, text: &mut String, password: bool, enabled: bool, focus: bool| {
                ui.label(egui::RichText::new(label).small().color(v.weak_text_color()));
                egui::Frame::NONE.fill(field_bg).corner_radius(R_MD).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                    let resp = ui.add_enabled(enabled, egui::TextEdit::singleline(text).password(password).frame(egui::Frame::NONE).desired_width(f32::INFINITY));
                    if focus {
                        resp.request_focus();
                    }
                });
                ui.add_space(6.0);
            };
            let focus_addr = ask.focus && ask.for_place.is_none();
            let focus_pw = ask.focus && ask.for_place.is_some();
            ask.focus = false;
            if ask.for_place.is_none() {
                field(ui, "Server address", &mut ask.address, false, true, focus_addr);
            }
            ui.checkbox(&mut ask.login.anonymous, "Connect as guest (no password)");
            ui.add_space(6.0);
            let named = !ask.login.anonymous;
            field(ui, "User name", &mut ask.login.user, false, named, false);
            if ask.address.starts_with("smb") || ask.for_place.as_ref().is_some_and(|(p, _)| p.scheme() == "smb") {
                field(ui, "Domain (optional)", &mut ask.login.domain, false, named, false);
            }
            field(ui, "Password", &mut ask.login.password, true, named, focus_pw);
            if let Some(e) = &ask.error {
                ui.label(egui::RichText::new(e).color(super::status(ui.ctx()).danger));
                ui.add_space(4.0);
            }
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().button_padding = Vec2::new(14.0, 5.0);
                let ok = egui::Button::new(egui::RichText::new("Connect").color(v.selection.stroke.color)).fill(v.selection.bg_fill);
                let can = ask.for_place.is_some() || ask.address.contains("://");
                if ui.add_enabled(can, ok).clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            submit = true;
        }
        if cancel || (r.should_close() && !submit) {
            self.connect_ask = None;
            return;
        }
        if submit {
            let ask = self.connect_ask.take().unwrap();
            let place = match ask.for_place {
                Some((p, _)) => p,
                None => {
                    let uri = ask.address.trim().to_string();
                    if !uri.contains("://") {
                        self.connect_ask = Some(ConnectAsk { error: Some("Start the address with smb://, sftp://, ftp:// or dav://".into()), ..ask });
                        return;
                    }
                    let host = network::host_of(&uri);
                    NetPlace { name: if host.is_empty() { uri.clone() } else { host }, uri: if uri.ends_with('/') || network::share_of(&uri).is_some() { uri } else { format!("{uri}/") } }
                }
            };
            self.navigate(Location::Network);
            self.net_open(ctx, place, Some(ask.login));
        }
    }
}
