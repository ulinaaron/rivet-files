//! Folder contents, tabs and navigation history.

use std::collections::HashSet;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Instant, SystemTime};

use crate::fsops;
use crate::mime::MimeDb;
use crate::places;
use crate::settings::{Settings, SortKey};

#[derive(Clone, Debug, PartialEq)]
pub enum Location {
    Dir(PathBuf),
    Trash,
    Recent,
    Network,
    Search { root: PathBuf, query: String },
}

impl Location {
    pub fn title(&self) -> String {
        match self {
            Location::Dir(p) => {
                if *p == places::home() {
                    "Home".into()
                } else if p == Path::new("/") {
                    "File System".into()
                } else {
                    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string())
                }
            }
            Location::Trash => "Trash".into(),
            Location::Recent => "Recent".into(),
            Location::Network => "Network".into(),
            Location::Search { query, .. } => format!("Search: {query}"),
        }
    }
    pub fn dir(&self) -> Option<&Path> {
        match self {
            Location::Dir(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_link: bool,
    pub broken_link: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mime: String,
    pub hidden: bool,
    pub mode: u32,
    pub uid: u32,
    pub child_count: Option<usize>,
    pub trash_origin: Option<(PathBuf, String)>,
}

impl Entry {
    pub fn from_path(path: PathBuf, db: &MimeDb) -> Option<Entry> {
        let lmeta = std::fs::symlink_metadata(&path).ok()?;
        let is_link = lmeta.file_type().is_symlink();
        let meta = if is_link { std::fs::metadata(&path).ok() } else { Some(lmeta.clone()) };
        let broken_link = is_link && meta.is_none();
        let meta = meta.unwrap_or(lmeta);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string());
        let is_dir = meta.is_dir();
        let mode = meta.permissions().mode();
        let executable = !is_dir && mode & 0o111 != 0;
        let mime = if broken_link {
            "inode/symlink".to_string()
        } else if !is_dir && meta.len() == 0 {
            // Empty file: nothing to sniff, so don't open it — the name decides.
            db.guess_by_name(&path).unwrap_or_else(|| "application/x-zerosize".into())
        } else {
            db.guess(&path, is_dir, executable)
        };
        let child_count = if is_dir {
            std::fs::read_dir(&path).ok().map(|r| r.count())
        } else {
            None
        };
        Some(Entry {
            hidden: name.starts_with('.') || name.ends_with('~'),
            name,
            is_dir,
            is_link,
            broken_link,
            size: meta.len(),
            modified: meta.modified().ok(),
            mime,
            mode,
            uid: meta.uid(),
            child_count,
            trash_origin: None,
            path,
        })
    }

    pub fn is_executable(&self) -> bool {
        !self.is_dir && self.mode & 0o111 != 0
    }
}

pub struct Tab {
    pub loc: Location,
    pub back: Vec<Location>,
    pub forward: Vec<Location>,
    pub entries: Vec<Entry>,
    /// Indices into `entries` after hiding/filtering/sorting.
    pub visible: Vec<usize>,
    pub selected: HashSet<PathBuf>,
    pub anchor: Option<usize>,
    pub cursor: Option<usize>,
    pub filter: String,
    pub error: Option<String>,
    pub dir_mtime: Option<SystemTime>,
    pub last_check: Instant,
    pub scroll_to: Option<PathBuf>,
    pub search_rx: Option<Receiver<Vec<PathBuf>>>,
    pub searching: bool,
    pub rubber: Option<eframe::egui::Pos2>,
}

impl Tab {
    pub fn new(loc: Location) -> Self {
        Tab {
            loc,
            back: vec![],
            forward: vec![],
            entries: vec![],
            visible: vec![],
            selected: HashSet::new(),
            anchor: None,
            cursor: None,
            filter: String::new(),
            error: None,
            dir_mtime: None,
            last_check: Instant::now(),
            scroll_to: None,
            search_rx: None,
            searching: false,
            rubber: None,
        }
    }

    pub fn reload(&mut self, db: &MimeDb, settings: &Settings) {
        self.error = None;
        let mut entries = Vec::new();
        match &self.loc {
            Location::Dir(p) => {
                self.dir_mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok();
                match std::fs::read_dir(p) {
                    Ok(rd) => {
                        let paths: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
                        entries = inspect_all(paths, db);
                    }
                    Err(e) => {
                        self.error = Some(match e.kind() {
                            std::io::ErrorKind::PermissionDenied => "You do not have permission to view this folder.".into(),
                            std::io::ErrorKind::NotFound => "This folder no longer exists.".into(),
                            _ => format!("Could not open this folder: {e}"),
                        })
                    }
                }
            }
            Location::Trash => {
                let files = places::trash_dir().join("files");
                self.dir_mtime = std::fs::metadata(&files).and_then(|m| m.modified()).ok();
                if let Ok(rd) = std::fs::read_dir(&files) {
                    for e in rd.flatten() {
                        if let Some(mut en) = Entry::from_path(e.path(), db) {
                            en.trash_origin = fsops::trash_info(&en.path);
                            en.hidden = false;
                            entries.push(en);
                        }
                    }
                }
            }
            Location::Recent => {
                self.dir_mtime = None;
                for p in places::recent_files() {
                    if let Some(mut en) = Entry::from_path(p, db) {
                        en.hidden = false;
                        entries.push(en);
                    }
                }
            }
            Location::Network => {
                self.dir_mtime = None;
                for p in places::network_places() {
                    if let Some(mut en) = Entry::from_path(p, db) {
                        en.hidden = false;
                        entries.push(en);
                    }
                }
            }
            Location::Search { .. } => {
                // Results arrive from the search thread; keep current list.
                return;
            }
        }
        self.entries = entries;
        let keep: HashSet<PathBuf> = self.entries.iter().map(|e| e.path.clone()).collect();
        self.selected.retain(|p| keep.contains(p));
        self.resort(db, settings);
        self.last_check = Instant::now();
    }

    pub fn resort(&mut self, db: &MimeDb, s: &Settings) {
        let f = self.filter.to_lowercase();
        let show_hidden = s.show_hidden || matches!(self.loc, Location::Search { .. });
        let mut vis: Vec<usize> = (0..self.entries.len())
            .filter(|&i| {
                let e = &self.entries[i];
                (show_hidden || !e.hidden) && (f.is_empty() || e.name.to_lowercase().contains(&f))
            })
            .collect();
        let entries = &self.entries;
        let recent = matches!(self.loc, Location::Recent);
        vis.sort_by(|&a, &b| {
            let (x, y) = (&entries[a], &entries[b]);
            if s.folders_first && x.is_dir != y.is_dir && !recent {
                return y.is_dir.cmp(&x.is_dir);
            }
            let o = match s.sort {
                SortKey::Name => natural_cmp(&x.name, &y.name),
                SortKey::Size => {
                    if x.is_dir && y.is_dir {
                        x.child_count.cmp(&y.child_count)
                    } else {
                        x.size.cmp(&y.size)
                    }
                }
                SortKey::Type => db.describe(&x.mime).cmp(&db.describe(&y.mime)),
                SortKey::Modified => x.modified.cmp(&y.modified),
            }
            .then_with(|| natural_cmp(&x.name, &y.name));
            if s.sort_desc { o.reverse() } else { o }
        });
        self.visible = vis;
    }

    pub fn visible_index_of(&self, p: &Path) -> Option<usize> {
        self.visible.iter().position(|&i| self.entries[i].path == p)
    }

    pub fn selected_entries(&self) -> Vec<&Entry> {
        self.visible
            .iter()
            .map(|&i| &self.entries[i])
            .filter(|e| self.selected.contains(&e.path))
            .collect()
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        self.selected_entries().iter().map(|e| e.path.clone()).collect()
    }

    /// Re-read the folder if it changed on disk (cheap polling instead of a watcher).
    /// The folder this tab shows, if it lives on disk (for the watcher).
    pub fn watched_dir(&self) -> Option<PathBuf> {
        match &self.loc {
            Location::Dir(p) => Some(p.clone()),
            Location::Trash => Some(places::trash_dir().join("files")),
            _ => None,
        }
    }

    pub fn needs_refresh(&mut self) -> bool {
        self.last_check = Instant::now();
        let p = match &self.loc {
            Location::Dir(p) => p.clone(),
            Location::Trash => places::trash_dir().join("files"),
            _ => return false,
        };
        let m = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
        m != self.dir_mtime
    }
}

/// Inspect many paths, spread over the CPU cores for big folders.
pub fn inspect_all(paths: Vec<PathBuf>, db: &MimeDb) -> Vec<Entry> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(16);
    if paths.len() < 2_000 || threads < 2 {
        return paths.into_iter().filter_map(|p| Entry::from_path(p, db)).collect();
    }
    let chunk = paths.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().filter_map(|p| Entry::from_path(p.clone(), db)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

/// Natural sort: "file2" before "file10", case-insensitive.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let a = a.trim_start_matches('.').to_lowercase();
    let b = b.trim_start_matches('.').to_lowercase();
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek(), bi.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = ai.peek().filter(|c| c.is_ascii_digit()) {
                    na.push(*c);
                    ai.next();
                }
                let mut nb = String::new();
                while let Some(c) = bi.peek().filter(|c| c.is_ascii_digit()) {
                    nb.push(*c);
                    bi.next();
                }
                let na2 = na.trim_start_matches('0');
                let nb2 = nb.trim_start_matches('0');
                let o = na2.len().cmp(&nb2.len()).then(na2.cmp(nb2));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.cmp(y);
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// Recursive name search below `root`, streamed back in batches.
pub fn spawn_search(root: PathBuf, query: String, show_hidden: bool, ctx: eframe::egui::Context) -> Receiver<Vec<PathBuf>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let q = query.to_lowercase();
        let mut stack = vec![root];
        let mut batch = Vec::new();
        let mut total = 0;
        let mut last = Instant::now();
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                let ft = e.file_type().ok();
                if ft.map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(e.path());
                }
                if name.to_lowercase().contains(&q) {
                    batch.push(e.path());
                    total += 1;
                }
            }
            if !batch.is_empty() && last.elapsed().as_millis() > 150 {
                if tx.send(std::mem::take(&mut batch)).is_err() {
                    return;
                }
                ctx.request_repaint();
                last = Instant::now();
            }
            if total > 5000 {
                break;
            }
        }
        let _ = tx.send(batch);
        ctx.request_repaint();
    });
    rx
}

#[cfg(test)]
mod tests {
    #[test]
    fn natural_order() {
        let mut v = vec!["file10", "File2", "file1", ".hidden", "alpha"];
        v.sort_by(|a, b| super::natural_cmp(a, b));
        assert_eq!(v, vec!["alpha", "file1", "File2", "file10", ".hidden"]);
    }
    #[test]
    fn mime_guess() {
        let db = crate::mime::MimeDb::load();
        let g = |n: &str| db.guess(std::path::Path::new(n), false, false);
        assert_eq!(g("/nonexistent/x.png"), "image/png");
        assert_eq!(g("/nonexistent/archive.tar.gz"), "application/x-compressed-tar");
        assert!(crate::mime::glob_match("*.[ch]", "main.c"));
        assert!(db.icon_names("image/png").iter().any(|n| n == "image-x-generic"));
    }
}
