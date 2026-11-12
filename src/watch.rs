//! A tiny background watcher: every 0.7 s it checks the open folders, the colour
//! files and the list of mounted drives, and wakes the window only when one of them
//! changed. The window itself never polls, so an idle Rivet uses no CPU.

use eframe::egui;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub struct Watch {
    paths: Arc<Mutex<Vec<PathBuf>>>,
    dirty: Arc<AtomicBool>,
}

type Stamp = Option<(SystemTime, u64, Option<PathBuf>)>;

fn stamp(p: &Path) -> Stamp {
    let link = std::fs::read_link(p).ok();
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len(), link))
}

fn mounts_hash() -> u64 {
    let mut h = DefaultHasher::new();
    std::fs::read_to_string("/proc/self/mounts").unwrap_or_default().hash(&mut h);
    h.finish()
}

impl Watch {
    pub fn spawn(ctx: egui::Context) -> Self {
        let paths: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
        let dirty = Arc::new(AtomicBool::new(false));
        let (p, d) = (paths.clone(), dirty.clone());
        std::thread::Builder::new()
            .name("rivet-watch".into())
            .spawn(move || {
                let mut prev: Option<(Vec<PathBuf>, Vec<Stamp>, u64)> = None;
                loop {
                    std::thread::sleep(Duration::from_millis(700));
                    let list = p.lock().unwrap().clone();
                    let stamps: Vec<Stamp> = list.iter().map(|x| stamp(x)).collect();
                    let mounts = mounts_hash();
                    let now = (list, stamps, mounts);
                    if let Some(old) = &prev {
                        // Only a change in something already being watched counts; a new
                        // list of paths (you opened another folder) just starts watching it.
                        let changed = if old.0 == now.0 { old.1 != now.1 || old.2 != now.2 } else { old.2 != now.2 };
                        if changed {
                            d.store(true, Ordering::Release);
                            ctx.request_repaint();
                        }
                    }
                    prev = Some(now);
                }
            })
            .expect("start watcher thread");
        Watch { paths, dirty }
    }

    /// The files and folders whose changes matter right now.
    pub fn set_paths(&self, mut list: Vec<PathBuf>) {
        list.sort();
        list.dedup();
        let mut g = self.paths.lock().unwrap();
        if *g != list {
            *g = list;
        }
    }

    /// True once after something changed.
    pub fn take(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }
}
