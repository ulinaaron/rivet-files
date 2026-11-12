//! Sidebar places: home and XDG user folders, bookmarks, mounted drives, recent files.

use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

pub fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
}

#[derive(Clone, Debug)]
pub struct UserDirs {
    pub desktop: Option<PathBuf>,
    pub documents: Option<PathBuf>,
    pub download: Option<PathBuf>,
    pub music: Option<PathBuf>,
    pub pictures: Option<PathBuf>,
    pub videos: Option<PathBuf>,
    pub templates: Option<PathBuf>,
    pub public: Option<PathBuf>,
}

pub fn user_dirs() -> UserDirs {
    let h = home();
    let text = std::fs::read_to_string(config_home().join("user-dirs.dirs")).unwrap_or_default();
    let get = |key: &str, default: &str| -> Option<PathBuf> {
        let mut val: Option<PathBuf> = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix(&format!("XDG_{key}_DIR=")) {
                let v = rest.trim().trim_matches('"').replace("$HOME", &h.to_string_lossy());
                let p = PathBuf::from(v);
                if p != h {
                    val = Some(p);
                }
            }
        }
        let p = val.unwrap_or_else(|| h.join(default));
        if p.is_dir() { Some(p) } else { None }
    };
    UserDirs {
        desktop: get("DESKTOP", "Desktop"),
        documents: get("DOCUMENTS", "Documents"),
        download: get("DOWNLOAD", "Downloads"),
        music: get("MUSIC", "Music"),
        pictures: get("PICTURES", "Pictures"),
        videos: get("VIDEOS", "Videos"),
        templates: get("TEMPLATES", "Templates"),
        public: get("PUBLICSHARE", "Public"),
    }
}

/// Icon name for a well-known folder (used both in the sidebar and in the file view).
pub fn special_folder_icon(path: &Path, dirs: &UserDirs) -> Option<&'static str> {
    if path == home() {
        return Some("user-home");
    }
    if path == Path::new("/") {
        return Some("drive-harddisk");
    }
    let pairs: [(&Option<PathBuf>, &'static str); 8] = [
        (&dirs.desktop, "user-desktop"),
        (&dirs.documents, "folder-documents"),
        (&dirs.download, "folder-download"),
        (&dirs.music, "folder-music"),
        (&dirs.pictures, "folder-pictures"),
        (&dirs.videos, "folder-videos"),
        (&dirs.templates, "folder-templates"),
        (&dirs.public, "folder-publicshare"),
    ];
    for (p, icon) in pairs {
        if p.as_deref() == Some(path) {
            return Some(icon);
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct Bookmark {
    pub path: PathBuf,
    pub label: String,
}

fn bookmarks_file() -> PathBuf {
    config_home().join("gtk-3.0/bookmarks")
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(v) = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn percent_encode_path(p: &Path) -> String {
    let mut out = String::new();
    for b in p.to_string_lossy().as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || "/-_.~".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Bookmarks shared with other Linux file managers (the standard bookmarks file).
pub fn load_bookmarks() -> Vec<Bookmark> {
    let text = std::fs::read_to_string(bookmarks_file()).unwrap_or_default();
    let mut v = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let (uri, label) = match line.split_once(' ') {
            Some((u, l)) => (u, Some(l.to_string())),
            None => (line, None),
        };
        if let Some(p) = uri.strip_prefix("file://") {
            let path = PathBuf::from(percent_decode(p));
            let label = label.unwrap_or_else(|| {
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "/".into())
            });
            v.push(Bookmark { path, label });
        }
    }
    v
}

pub fn save_bookmarks(list: &[Bookmark]) -> std::io::Result<()> {
    let mut s = String::new();
    for b in list {
        let default = b.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        s.push_str(&format!("file://{}", percent_encode_path(&b.path)));
        if b.label != default {
            s.push(' ');
            s.push_str(&b.label);
        }
        s.push('\n');
    }
    let f = bookmarks_file();
    if let Some(parent) = f.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(f, s)
}

#[derive(Clone, Debug)]
pub struct Mount {
    pub path: PathBuf,
    pub label: String,
    pub device: String,
    pub removable: bool,
    pub network: bool,
}

fn unescape_mount(s: &str) -> String {
    // /proc/mounts escapes space as \040 etc.
    let mut out: Vec<u8> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 4]).ok().and_then(|x| u8::from_str_radix(x, 8).ok()) {
                out.push(v);
                i += 4;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// User-visible mounted drives (USB sticks, other partitions, network shares).
pub fn mounts() -> Vec<Mount> {
    let text = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
    let mut v: Vec<Mount> = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let device = parts[0];
        let mp = unescape_mount(parts[1]);
        let fstype = parts[2];
        let network = fstype.starts_with("nfs") || fstype == "cifs" || fstype == "smb3" || fstype.starts_with("fuse.sshfs") || fstype == "fuse.rclone";
        let user_visible = network
            || mp.starts_with("/media/")
            || mp.starts_with("/run/media/")
            || mp.starts_with("/mnt/")
            || (device.starts_with("/dev/") && (mp == "/home" || mp.starts_with("/data")));
        // Skip the partition holding the home folder; it is already reachable as Home.
        if !user_visible || mp.starts_with("/run/media/") && mp.matches('/').count() < 4 || home().starts_with(&mp) {
            continue;
        }
        if ["tmpfs", "proc", "sysfs", "devtmpfs", "overlay", "squashfs", "autofs"].contains(&fstype) {
            continue;
        }
        let path = PathBuf::from(&mp);
        if v.iter().any(|m| m.path == path) {
            continue;
        }
        let label = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(mp.clone());
        let removable = mp.starts_with("/media/") || mp.starts_with("/run/media/");
        v.push(Mount { path, label, device: device.to_string(), removable, network });
    }
    v
}

/// Network locations: mounted network shares plus anything the desktop's
/// virtual filesystem service has mounted under /run/user/<uid>/gvfs.
pub fn network_places() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = mounts().into_iter().filter(|m| m.network).map(|m| m.path).collect();
    let uid = unsafe { libc::getuid() };
    if let Ok(rd) = std::fs::read_dir(format!("/run/user/{uid}/gvfs")) {
        for e in rd.flatten() {
            v.push(e.path());
        }
    }
    v
}

/// (total bytes, free bytes) of the filesystem holding `path`.
pub fn disk_space(path: &Path) -> Option<(u64, u64)> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::statvfs(c.as_ptr(), &mut st) };
    if r != 0 {
        return None;
    }
    let bs = st.f_frsize as u64;
    Some((st.f_blocks as u64 * bs, st.f_bavail as u64 * bs))
}

/// Recently used files from the standard desktop history file.
pub fn recent_files() -> Vec<PathBuf> {
    let text = std::fs::read_to_string(data_home().join("recently-used.xbel")).unwrap_or_default();
    let mut items: Vec<(String, PathBuf)> = Vec::new();
    for chunk in text.split("<bookmark ").skip(1) {
        let Some(h) = chunk.find("href=\"") else { continue };
        let rest = &chunk[h + 6..];
        let Some(end) = rest.find('"') else { continue };
        let href = &rest[..end];
        let Some(p) = href.strip_prefix("file://") else { continue };
        let path = PathBuf::from(percent_decode(&p.replace("&amp;", "&")));
        let modified = chunk
            .find("modified=\"")
            .map(|i| chunk[i + 10..].split('"').next().unwrap_or("").to_string())
            .unwrap_or_default();
        if path.exists() && !items.iter().any(|(_, x)| *x == path) {
            items.push((modified, path));
        }
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items.into_iter().map(|i| i.1).take(200).collect()
}

pub fn trash_dir() -> PathBuf {
    data_home().join("Trash")
}

pub fn trash_is_empty() -> bool {
    std::fs::read_dir(trash_dir().join("files")).map(|mut r| r.next().is_none()).unwrap_or(true)
}
