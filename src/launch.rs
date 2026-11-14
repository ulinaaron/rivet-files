//! Opening files: default applications (mimeapps.list), "Open With" choices from
//! installed .desktop entries, and opening a terminal in a folder.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::places;

#[derive(Clone, Debug)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub mimes: Vec<String>,
    pub terminal: bool,
}

fn app_dirs() -> Vec<PathBuf> {
    let mut v = vec![places::data_home().join("applications")];
    let dd = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in dd.split(':') {
        let p = PathBuf::from(d).join("applications");
        if !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

fn parse_desktop(path: &Path, id: String) -> Option<DesktopApp> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let (mut name, mut exec, mut icon, mut mimes) = (None, None, String::new(), Vec::new());
    let mut hidden = false;
    let mut terminal = false;
    let mut is_app = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "Name" => name = Some(v.trim().to_string()),
            "Exec" => exec = Some(v.trim().to_string()),
            "Icon" => icon = v.trim().to_string(),
            "MimeType" => mimes = v.split(';').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
            "NoDisplay" | "Hidden" => hidden |= v.trim() == "true",
            "Terminal" => terminal = v.trim() == "true",
            "Type" => is_app = v.trim() == "Application",
            _ => {}
        }
    }
    if hidden || !is_app {
        return None;
    }
    Some(DesktopApp { id, name: name?, exec: exec?, icon, mimes, terminal })
}

pub fn all_apps() -> Vec<DesktopApp> {
    let mut v: Vec<DesktopApp> = Vec::new();
    for d in app_dirs() {
        let mut stack = vec![(d.clone(), String::new())];
        while let Some((dir, prefix)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                let fname = e.file_name().to_string_lossy().to_string();
                if p.is_dir() {
                    stack.push((p, format!("{prefix}{fname}-")));
                } else if fname.ends_with(".desktop") {
                    let id = format!("{prefix}{fname}");
                    if v.iter().any(|a| a.id == id) {
                        continue;
                    }
                    if let Some(a) = parse_desktop(&p, id) {
                        v.push(a);
                    }
                }
            }
        }
    }
    v.sort_by_key(|a| a.name.to_lowercase());
    v
}

pub(crate) fn mimeapps_default(mime: &str) -> Option<String> {
    let mut files = vec![places::config_home().join("mimeapps.list")];
    if let Ok(d) = std::env::var("XDG_CURRENT_DESKTOP") {
        for de in d.split(':') {
            files.insert(0, places::config_home().join(format!("{}-mimeapps.list", de.to_lowercase())));
        }
    }
    for dd in app_dirs() {
        files.push(dd.join("mimeapps.list"));
        files.push(dd.join("defaults.list"));
    }
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let mut in_default = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_default = line == "[Default Applications]";
                continue;
            }
            if in_default
                && let Some((k, v)) = line.split_once('=')
                    && k.trim() == mime
                        && let Some(first) = v.split(';').find(|s| !s.is_empty()) {
                            return Some(first.trim().to_string());
                        }
        }
    }
    None
}

/// Make an app the default for a file type (writes ~/.config/mimeapps.list via xdg-mime).
pub fn set_default_app(app_id: &str, mime: &str) -> Result<(), String> {
    let ok = Command::new("xdg-mime")
        .args(["default", app_id, mime])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok { Ok(()) } else { Err("Could not change the default application.".into()) }
}

pub fn default_app<'a>(apps: &'a [DesktopApp], mime: &str) -> Option<&'a DesktopApp> {
    if let Some(id) = mimeapps_default(mime)
        && let Some(a) = apps.iter().find(|a| a.id == id) {
            return Some(a);
        }
    apps.iter().find(|a| a.mimes.iter().any(|m| m == mime))
}

pub fn apps_for<'a>(apps: &'a [DesktopApp], mime: &str) -> Vec<&'a DesktopApp> {
    let media = format!("{}/*", mime.split('/').next().unwrap_or(""));
    let mut v: Vec<&DesktopApp> = apps
        .iter()
        .filter(|a| a.mimes.iter().any(|m| m == mime || *m == media))
        .collect();
    if mime.starts_with("text/") || mime == "application/x-shellscript" {
        for a in apps.iter().filter(|a| a.mimes.iter().any(|m| m == "text/plain")) {
            if !v.iter().any(|x| x.id == a.id) {
                v.push(a);
            }
        }
    }
    v
}

fn split_exec(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = exec.chars().peekable();
    let mut has = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_q = !in_q;
                has = true;
            }
            '\\' if in_q => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ' ' if !in_q => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            _ => cur.push(c),
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn launch_app(app: &DesktopApp, files: &[PathBuf]) -> Result<(), String> {
    let mut args = exec_args(app, files);
    if args.is_empty() {
        return Err("Nothing to run".into());
    }
    if app.terminal {
        let mut t = terminal_command();
        t.push("-e".into());
        t.extend(args);
        args = t;
    }
    spawn_detached(&args, files.first().and_then(|f| f.parent()))
}

/// Expand a desktop entry's Exec= line into a command line for these files.
pub fn exec_args(app: &DesktopApp, files: &[PathBuf]) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut used = false;
    for tok in split_exec(&app.exec) {
        match tok.as_str() {
            "%f" | "%u" => {
                if let Some(f) = files.first() {
                    args.push(f.to_string_lossy().to_string());
                }
                used = true;
            }
            "%F" | "%U" => {
                args.extend(files.iter().map(|f| f.to_string_lossy().to_string()));
                used = true;
            }
            "%i" => {
                if !app.icon.is_empty() {
                    args.push("--icon".into());
                    args.push(app.icon.clone());
                }
            }
            "%c" => args.push(app.name.clone()),
            "%k" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
            t => args.push(t.replace("%%", "%")),
        }
    }
    if !used {
        args.extend(files.iter().map(|f| f.to_string_lossy().to_string()));
    }
    args
}

fn spawn_detached(args: &[String], cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(d) = cwd
        && d.is_dir() {
            cmd.current_dir(d);
        }
    cmd.spawn().map(|_| ()).map_err(|e| format!("Could not start {}: {e}", args[0]))
}

/// Open a web address or folder with the desktop's default handler.
pub fn open_url(url: &str) -> Result<(), String> {
    spawn_detached(&["xdg-open".into(), url.to_string()], None)
}

/// Open with the desktop's default application.
pub fn open_default(apps: &[DesktopApp], mime: &str, path: &Path) -> Result<(), String> {
    // Host .desktop files point at binaries outside the sandbox. The runtime's
    // xdg-open asks the desktop portal to open the file with the real default app.
    if places::in_flatpak() {
        return spawn_detached(&["xdg-open".into(), path.to_string_lossy().to_string()], None);
    }
    if let Some(a) = default_app(apps, mime) {
        return launch_app(a, &[path.to_path_buf()]);
    }
    spawn_detached(&["xdg-open".into(), path.to_string_lossy().to_string()], None)
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

fn terminal_command() -> Vec<String> {
    if let Ok(t) = std::env::var("TERMINAL")
        && !t.is_empty() && which(&t) {
            return vec![t];
        }
    for t in [
        "x-terminal-emulator", "kitty", "alacritty", "foot", "wezterm", "ghostty", "gnome-terminal",
        "kgx", "ptyxis", "konsole", "xfce4-terminal", "tilix", "terminator", "xterm",
    ] {
        if which(t) {
            return vec![t.to_string()];
        }
    }
    vec!["xterm".into()]
}

pub fn open_terminal(dir: &Path) -> Result<(), String> {
    let t = terminal_command();
    spawn_detached(&t, Some(dir))
}

pub fn run_executable(path: &Path) -> Result<(), String> {
    spawn_detached(&[path.to_string_lossy().to_string()], path.parent())
}
