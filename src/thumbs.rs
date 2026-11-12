//! Picture thumbnails made on a background thread.

use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

const THUMB_PX: u32 = 256;
const MAX_FILE: u64 = 40 * 1024 * 1024;

pub struct Thumbnailer {
    tx: Sender<PathBuf>,
    rx: Receiver<(PathBuf, Option<egui::ColorImage>)>,
    pending: HashSet<PathBuf>,
    done: HashMap<PathBuf, Option<egui::TextureHandle>>,
}

pub fn can_thumbnail(mime: &str) -> bool {
    matches!(
        mime,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp" | "image/svg+xml"
    )
}

impl Thumbnailer {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, wrx) = channel::<PathBuf>();
        let (wtx, rx) = channel();
        // Two workers share the queue.
        let wrx = std::sync::Arc::new(std::sync::Mutex::new(wrx));
        for _ in 0..2 {
            let wrx = wrx.clone();
            let wtx = wtx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let job = { wrx.lock().unwrap().recv() };
                let Ok(path) = job else { break };
                let img = make_thumb(&path);
                if wtx.send((path, img)).is_err() {
                    break;
                }
                ctx.request_repaint();
            });
        }
        Self { tx, rx, pending: HashSet::new(), done: HashMap::new() }
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((p, img)) = self.rx.try_recv() {
            self.pending.remove(&p);
            let tex = img.map(|i| ctx.load_texture(format!("thumb:{}", p.display()), i, egui::TextureOptions::LINEAR));
            self.done.insert(p, tex);
        }
    }

    pub fn get(&mut self, path: &PathBuf, size: u64) -> Option<egui::TextureHandle> {
        if let Some(t) = self.done.get(path) {
            return t.clone();
        }
        if size <= MAX_FILE && !self.pending.contains(path) {
            self.pending.insert(path.clone());
            let _ = self.tx.send(path.clone());
        }
        None
    }

    pub fn forget(&mut self, path: &PathBuf) {
        self.done.remove(path);
    }

    pub fn clear(&mut self) {
        self.done.clear();
    }
}

fn make_thumb(path: &PathBuf) -> Option<egui::ColorImage> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "svg" {
        return crate::icons::load_image_file(path, THUMB_PX);
    }
    let img = image::open(path).ok()?;
    let img = if img.width() > THUMB_PX || img.height() > THUMB_PX {
        img.thumbnail(THUMB_PX, THUMB_PX)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()))
}
