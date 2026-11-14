//! Crash reports, kept on this computer only.
//!
//! When Rivet panics (a bug), a plain-text report is written to
//! `~/.local/state/rivet/crashes/crash-YYYYMMDD-HHMMSS.txt`: the version, what went wrong and
//! where, the stack, and the system. Nothing is sent anywhere. The next time Rivet starts it shows
//! the newest unseen report once, with buttons to copy it, open its folder, or open a GitHub issue.

use std::path::{Path, PathBuf};

/// How many reports to keep (older ones are removed when a new one is written).
const KEEP: usize = 20;

pub fn dir() -> PathBuf {
    crate::places::state_home().join("rivet/crashes")
}

/// Write a report whenever Rivet panics, then carry on as before (the message still goes to
/// the terminal).
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let place = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown".into());
        let stack = std::backtrace::Backtrace::force_capture();
        let _ = write_report(&thread, &what, &place, &trim_stack(&stack.to_string()));
        previous(info);
    }));
}

/// The stack from where the panic was raised: the panic machinery's own frames dropped, and at
/// most 40 frames kept.
fn trim_stack(stack: &str) -> String {
    let lines: Vec<&str> = stack.lines().collect();
    // Frames are "  N: name" lines, each followed by "at file:line" lines.
    let is_frame = |l: &str| l.trim_start().split_once(':').is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()));
    let machinery = |l: &str| {
        ["panicking", "begin_panic", "panic_fmt", "rust_begin_unwind", "rust_end_short_backtrace", "crash::install", "alloc::boxed::Box<dyn", "backtrace::Backtrace"]
            .iter()
            .any(|m| l.contains(m))
    };
    // The top of the stack is the panic machinery (and this hook); start at the first frame after it.
    let mut start = 0;
    if lines.iter().take_while(|l| !is_frame(l) || machinery(l)).any(|l| is_frame(l)) {
        start = lines.iter().position(|l| is_frame(l) && !machinery(l)).unwrap_or(0);
    }
    let mut out = Vec::new();
    let mut frames = 0;
    for l in &lines[start..] {
        if is_frame(l) {
            frames += 1;
            if frames > 40 {
                out.push("   …");
                break;
            }
        }
        out.push(l);
    }
    out.join("\n")
}

fn write_report(thread: &str, what: &str, place: &str, stack: &str) -> std::io::Result<PathBuf> {
    let d = dir();
    std::fs::create_dir_all(&d)?;
    let now = chrono::Local::now();
    let mut path = d.join(format!("crash-{}.txt", now.format("%Y%m%d-%H%M%S")));
    // Two panics in one second (say, two threads): keep both.
    let mut n = 2;
    while path.exists() {
        path = d.join(format!("crash-{}-{n}.txt", now.format("%Y%m%d-%H%M%S")));
        n += 1;
    }
    let text = format!(
        "Rivet Files {version} crash report\n\
         Time:    {time}\n\
         Thread:  {thread}\n\
         Where:   {place}\n\
         What:    {what}\n\
         System:  {system}\n\
         Session: {session}\n\
         Package: {package}\n\
         \n\
         Stack:\n{stack}\n",
        version = env!("CARGO_PKG_VERSION"),
        time = now.format("%Y-%m-%d %H:%M:%S %z"),
        system = system(),
        session = session(),
        package = package(),
    );
    std::fs::write(&path, text)?;
    prune();
    Ok(path)
}

/// "Fedora Linux 44 (Workstation Edition), Linux 7.2.15, x86_64"
fn system() -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "Linux".into());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|k| k.trim().to_string()).unwrap_or_default();
    format!("{os}, Linux {kernel}, {}", std::env::consts::ARCH)
}

/// "wayland, GNOME" — which display system and desktop.
fn session() -> String {
    let kind = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown desktop".into());
    format!("{kind}, {desktop}")
}

/// How Rivet was installed: Flatpak, AppImage or a plain binary.
fn package() -> &'static str {
    if crate::places::in_flatpak() {
        "Flatpak"
    } else if std::env::var_os("APPDIR")
        // An AppImage that started Rivet passes APPDIR on; only count it when Rivet runs from inside.
        .is_some_and(|d| std::env::current_exe().is_ok_and(|e| e.starts_with(&d)))
    {
        "AppImage"
    } else {
        "binary"
    }
}

/// The saved reports, newest first.
pub fn reports() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt")))
        .collect();
    v.sort();
    v.reverse();
    v
}

fn prune() {
    for old in reports().into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(old);
    }
}

/// The newest report not shown yet, if any.
pub fn unseen() -> Option<Report> {
    let newest = reports().into_iter().next()?;
    let name = newest.file_name()?.to_string_lossy().to_string();
    let seen = std::fs::read_to_string(dir().join("seen")).unwrap_or_default();
    if seen.trim() == name {
        return None;
    }
    Report::load(&newest)
}

/// Remember that this report has been shown, so it isn't shown again.
pub fn mark_seen(report: &Report) {
    if let Some(name) = report.path.file_name() {
        let _ = std::fs::write(dir().join("seen"), name.to_string_lossy().as_bytes());
    }
}

pub struct Report {
    pub path: PathBuf,
    pub text: String,
    /// The window's own thread stopped (Rivet closed), rather than a background task.
    pub closed: bool,
    /// Where in Rivet's code it happened ("src/app/views.rs:120:9"), for an issue title.
    /// (The message itself can contain file names, so it stays out of the title.)
    pub place: String,
}

impl Report {
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(k).map(|v| v.trim().to_string())).unwrap_or_default();
        let closed = field("Thread:") == "main";
        let place = field("Where:");
        Some(Self { path: path.to_path_buf(), text, closed, place })
    }
}

#[cfg(test)]
mod tests {
    use super::trim_stack;

    #[test]
    fn stack_starts_at_rivet_code() {
        let stack = "   0: rivet::crash::install::{closure#0}\n             at src/crash.rs:30:21\n   1: std::panicking::panic_with_hook\n   2: core::panicking::panic_fmt\n   3: rivet::app::RivetApp::ui\n             at src/app.rs:10:5\n   4: std::panicking::try\n   5: main";
        let t = trim_stack(stack);
        assert!(t.starts_with("   3: rivet::app::RivetApp::ui"), "{t}");
        // Frames further down that mention panicking (Rust's own startup) are kept.
        assert!(t.contains("std::panicking::try"));
    }
}
