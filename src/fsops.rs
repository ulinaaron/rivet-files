//! File operations: copy, move, trash (freedesktop trash spec), restore, delete.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::places;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OpKind {
    Copy,
    Move,
    Trash,
    Delete,
    Restore,
    EmptyTrash,
    Compress,
    Extract,
}

impl OpKind {
    pub fn verb(&self) -> &'static str {
        match self {
            OpKind::Copy => "Copying",
            OpKind::Move => "Moving",
            OpKind::Trash => "Moving to Trash",
            OpKind::Delete => "Deleting",
            OpKind::Restore => "Restoring",
            OpKind::EmptyTrash => "Emptying Trash",
            OpKind::Compress => "Compressing",
            OpKind::Extract => "Extracting",
        }
    }
}

/// Shared progress of a background operation.
pub struct Progress {
    pub kind: OpKind,
    pub total_items: usize,
    pub done_items: AtomicU64,
    pub bytes_done: AtomicU64,
    pub current: Mutex<String>,
    pub finished: AtomicBool,
    pub cancel: AtomicBool,
    pub errors: Mutex<Vec<String>>,
    pub created: Mutex<Vec<PathBuf>>,
    /// (source, result) for every item that succeeded — what undo needs.
    pub done: Mutex<Vec<(PathBuf, PathBuf)>>,
    /// Why this operation runs: 0 = you did it, 1 = an undo, 2 = a redo.
    pub role: u8,
}

impl Progress {
    pub(crate) fn blank(kind: OpKind, total: usize, role: u8) -> Progress {
        Progress {
            kind,
            total_items: total.max(1),
            done_items: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            current: Mutex::new(String::new()),
            finished: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            errors: Mutex::new(Vec::new()),
            created: Mutex::new(Vec::new()),
            done: Mutex::new(Vec::new()),
            role,
        }
    }
}

pub fn spawn_op(kind: OpKind, sources: Vec<PathBuf>, dest: Option<PathBuf>) -> Arc<Progress> {
    spawn_op_role(kind, sources, dest, 0)
}

/// Move each item to an exact path (used to undo a move). Recorded as a Move.
pub fn spawn_move_pairs(pairs: Vec<(PathBuf, PathBuf)>, role: u8) -> Arc<Progress> {
    let p = Arc::new(Progress::blank(OpKind::Move, pairs.len(), role));
    let pp = p.clone();
    std::thread::spawn(move || {
        for (src, dst) in pairs {
            *pp.current.lock().unwrap() = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let target = if dst.symlink_metadata().is_ok() { unique_target(dst.parent().unwrap_or(Path::new("/")), &dst) } else { dst };
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match move_path(&src, &target, &pp) {
                Ok(()) => {
                    pp.created.lock().unwrap().push(target.clone());
                    pp.done.lock().unwrap().push((src, target));
                }
                Err(e) => pp.errors.lock().unwrap().push(format!("{}: {e}", src.display())),
            }
            pp.done_items.fetch_add(1, Ordering::Relaxed);
        }
        pp.finished.store(true, Ordering::Release);
    });
    p
}

pub fn spawn_op_role(kind: OpKind, sources: Vec<PathBuf>, dest: Option<PathBuf>, role: u8) -> Arc<Progress> {
    let p = Arc::new(Progress::blank(kind, sources.len(), role));
    let pp = p.clone();
    std::thread::spawn(move || {
        if kind == OpKind::EmptyTrash {
            if let Err(e) = empty_trash() {
                pp.errors.lock().unwrap().push(e.to_string());
            }
        }
        for src in sources {
            if pp.cancel.load(Ordering::Relaxed) {
                break;
            }
            *pp.current.lock().unwrap() = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let res: std::io::Result<Option<PathBuf>> = match kind {
                OpKind::Copy => {
                    let d = dest.as_ref().unwrap();
                    let target = unique_target(d, &src);
                    copy_recursive(&src, &target, &pp).map(|_| Some(target))
                }
                OpKind::Move => {
                    let d = dest.as_ref().unwrap();
                    if src.parent() == Some(d.as_path()) {
                        Ok(None)
                    } else if d.starts_with(&src) {
                        Err(std::io::Error::other("Cannot move a folder into itself"))
                    } else {
                        let target = unique_target(d, &src);
                        move_path(&src, &target, &pp).map(|_| Some(target))
                    }
                }
                OpKind::Trash => trash(&src).map(Some),
                OpKind::Delete => remove_recursive(&src).map(|_| None),
                OpKind::Restore => restore_from_trash(&src).map(Some),
                // These run through archive::spawn_compress / spawn_extract.
                OpKind::EmptyTrash | OpKind::Compress | OpKind::Extract => Ok(None),
            };
            match res {
                Ok(Some(c)) => {
                    pp.done.lock().unwrap().push((src.clone(), c.clone()));
                    // Trashed items aren't "created" in the folder you're looking at.
                    if kind != OpKind::Trash {
                        pp.created.lock().unwrap().push(c);
                    }
                }
                Ok(None) => {}
                Err(e) => pp.errors.lock().unwrap().push(format!(
                    "{}: {}",
                    src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                    e
                )),
            }
            pp.done_items.fetch_add(1, Ordering::Relaxed);
        }
        pp.finished.store(true, Ordering::Release);
    });
    p
}

/// "photo.jpg" -> "photo (copy).jpg", "photo (copy 2).jpg", ... if the name is taken.
pub fn unique_target(dir: &Path, src: &Path) -> PathBuf {
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let candidate = dir.join(&name);
    if !candidate.exists() && candidate.symlink_metadata().is_err() {
        return candidate;
    }
    let is_dir = src.is_dir();
    let (stem, ext) = split_name(&name, is_dir);
    for i in 1.. {
        let suffix = if i == 1 { " (copy)".to_string() } else { format!(" (copy {i})") };
        let c = dir.join(format!("{stem}{suffix}{ext}"));
        if !c.exists() && c.symlink_metadata().is_err() {
            return c;
        }
    }
    unreachable!()
}

pub fn unique_new_name(dir: &Path, base: &str) -> PathBuf {
    let c = dir.join(base);
    if !c.exists() {
        return c;
    }
    let (stem, ext) = split_name(base, false);
    for i in 2.. {
        let c = dir.join(format!("{stem} {i}{ext}"));
        if !c.exists() {
            return c;
        }
    }
    unreachable!()
}

/// Split "archive.tar.gz" into ("archive", ".tar.gz"); folders have no extension.
pub fn split_name(name: &str, is_dir: bool) -> (String, String) {
    if is_dir || name.starts_with('.') && name.matches('.').count() == 1 {
        return (name.to_string(), String::new());
    }
    let lower = name.to_lowercase();
    for double in [".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst"] {
        if lower.ends_with(double) && name.len() > double.len() {
            let i = name.len() - double.len();
            return (name[..i].to_string(), name[i..].to_string());
        }
    }
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i..].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn copy_recursive(src: &Path, dst: &Path, p: &Progress) -> std::io::Result<()> {
    if p.cancel.load(Ordering::Relaxed) {
        return Err(std::io::Error::other("Cancelled"));
    }
    let meta = std::fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dst)?;
    } else if meta.is_dir() {
        std::fs::create_dir(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst.join(e.file_name()), p)?;
        }
        let _ = std::fs::set_permissions(dst, meta.permissions());
    } else {
        let n = std::fs::copy(src, dst)?;
        p.bytes_done.fetch_add(n, Ordering::Relaxed);
    }
    Ok(())
}

fn move_path(src: &Path, dst: &Path, p: &Progress) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            copy_recursive(src, dst, p)?;
            remove_recursive(src)
        }
        Err(e) => Err(e),
    }
}

pub fn remove_recursive(p: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(p)?;
    if meta.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

/// Move to ~/.local/share/Trash following the freedesktop trash spec so the
/// desktop's own trash (and other file managers) can see and restore it.
pub fn trash(src: &Path) -> std::io::Result<PathBuf> {
    let tdir = places::trash_dir();
    let files = tdir.join("files");
    let info = tdir.join("info");
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let (stem, ext) = split_name(&name, src.is_dir());
    let mut tname = name.clone();
    let mut i = 2;
    while files.join(&tname).symlink_metadata().is_ok() || info.join(format!("{tname}.trashinfo")).exists() {
        tname = format!("{stem}.{i}{ext}");
        i += 1;
    }
    let abs = std::fs::canonicalize(src.parent().unwrap_or(Path::new("/")))
        .unwrap_or_else(|_| src.parent().unwrap_or(Path::new("/")).to_path_buf())
        .join(&name);
    let date = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
    let content = format!("[Trash Info]\nPath={}\nDeletionDate={}\n", places::percent_encode_path(&abs), date);
    let info_path = info.join(format!("{tname}.trashinfo"));
    std::fs::write(&info_path, content)?;
    let dummy = Progress::blank(OpKind::Trash, 1, 0);
    let trashed = files.join(&tname);
    if let Err(e) = move_path(src, &trashed, &dummy) {
        let _ = std::fs::remove_file(&info_path);
        return Err(e);
    }
    Ok(trashed)
}

/// Where a trashed item originally lived, and when it was deleted.
pub fn trash_info(trashed: &Path) -> Option<(PathBuf, String)> {
    let name = trashed.file_name()?.to_string_lossy().to_string();
    let info = places::trash_dir().join("info").join(format!("{name}.trashinfo"));
    let text = std::fs::read_to_string(info).ok()?;
    let mut path = None;
    let mut date = String::new();
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("Path=") {
            path = Some(PathBuf::from(places::percent_decode(p)));
        }
        if let Some(d) = line.strip_prefix("DeletionDate=") {
            date = d.replace('T', " ");
        }
    }
    path.map(|p| (p, date))
}

pub fn restore_from_trash(trashed: &Path) -> std::io::Result<PathBuf> {
    let (orig, _) = trash_info(trashed).ok_or_else(|| std::io::Error::other("No restore information"))?;
    let parent = orig.parent().unwrap_or(Path::new("/")).to_path_buf();
    std::fs::create_dir_all(&parent)?;
    let target = if orig.exists() { unique_target(&parent, &orig) } else { orig };
    let dummy = Progress::blank(OpKind::Restore, 1, 0);
    move_path(trashed, &target, &dummy)?;
    let name = trashed.file_name().unwrap().to_string_lossy().to_string();
    let _ = std::fs::remove_file(places::trash_dir().join("info").join(format!("{name}.trashinfo")));
    Ok(target)
}

pub fn empty_trash() -> std::io::Result<()> {
    let t = places::trash_dir();
    for sub in ["files", "info"] {
        if let Ok(rd) = std::fs::read_dir(t.join(sub)) {
            for e in rd.flatten() {
                let _ = remove_recursive(&e.path());
            }
        }
    }
    Ok(())
}

/// Total size and number of files below a folder (stops after a time limit).
pub fn dir_size(p: &Path, deadline: std::time::Instant) -> (u64, u64, bool) {
    let mut size = 0;
    let mut count = 0;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        if std::time::Instant::now() > deadline {
            return (size, count, false);
        }
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                if let Ok(m) = e.metadata() {
                    count += 1;
                    if m.is_dir() && !e.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
                        stack.push(e.path());
                    } else {
                        size += m.len();
                    }
                }
            }
        }
    }
    (size, count, true)
}

pub fn human_size(n: u64) -> String {
    const U: [&str; 6] = ["bytes", "kB", "MB", "GB", "TB", "PB"];
    if n < 1000 {
        return if n == 1 { "1 byte".into() } else { format!("{n} bytes") };
    }
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    format!("{:.1} {}", v, U[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(p: &Arc<Progress>) {
        while !p.finished.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn names_and_copies() {
        assert_eq!(split_name("photo.jpg", false), ("photo".into(), ".jpg".into()));
        assert_eq!(split_name("a.tar.gz", false), ("a".into(), ".tar.gz".into()));
        assert_eq!(split_name(".bashrc", false), (".bashrc".into(), "".into()));
        assert_eq!(split_name("dir.d", true), ("dir.d".into(), "".into()));

        let root = std::env::temp_dir().join(format!("rivet-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(a.join("sub")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("note.txt"), "hello").unwrap();
        std::fs::write(a.join("sub/deep.txt"), "deep").unwrap();

        // copy a file into its own folder -> "(copy)"
        let p = spawn_op(OpKind::Copy, vec![a.join("note.txt")], Some(a.clone()));
        wait(&p);
        assert!(a.join("note (copy).txt").exists());
        // copy a folder recursively
        let p = spawn_op(OpKind::Copy, vec![a.join("sub")], Some(b.clone()));
        wait(&p);
        assert_eq!(std::fs::read_to_string(b.join("sub/deep.txt")).unwrap(), "deep");
        // move a file
        let p = spawn_op(OpKind::Move, vec![a.join("note.txt")], Some(b.clone()));
        wait(&p);
        assert!(!a.join("note.txt").exists() && b.join("note.txt").exists());
        // moving a folder into itself is refused
        let p = spawn_op(OpKind::Move, vec![b.clone()], Some(b.join("sub")));
        wait(&p);
        assert_eq!(p.errors.lock().unwrap().len(), 1);
        // delete
        let p = spawn_op(OpKind::Delete, vec![b.join("sub")], None);
        wait(&p);
        assert!(!b.join("sub").exists());
        assert_eq!(unique_new_name(&a, "Untitled Folder"), a.join("Untitled Folder"));
        std::fs::create_dir(a.join("Untitled Folder")).unwrap();
        assert_eq!(unique_new_name(&a, "Untitled Folder"), a.join("Untitled Folder 2"));
        assert_eq!(human_size(1500), "1.5 kB");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
