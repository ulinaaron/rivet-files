//! The launcher entry (freedesktop .desktop file) and default-file-manager setting.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::places;

pub const DESKTOP_ID: &str = "rivet-files.desktop";

/// Rivet's app icon (also used for the window and the AppImage).
pub const ICON_SVG: &str = include_str!("../data/rivet-files.svg");

fn icon_path() -> PathBuf {
    places::data_home().join("icons/hicolor/scalable/apps/rivet-files.svg")
}

pub fn entry_path() -> PathBuf {
    places::data_home().join("applications").join(DESKTOP_ID)
}

/// Quote a path for an Exec= line (desktop entry spec rules).
fn exec_quote(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut out = String::from("\"");
    for c in s.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    // Backslashes are themselves escaped once more inside desktop file values.
    out.replace('\\', "\\\\")
}

pub fn entry_text(exe: &Path) -> String {
    let e = exec_quote(exe);
    format!(
        "[Desktop Entry]
Type=Application
Name=Rivet Files
GenericName=File Manager
Comment=Browse and organize your files
Exec={e} %U
Icon=rivet-files
Terminal=false
StartupNotify=true
StartupWMClass=rivet-files
Categories=System;FileTools;FileManager;Core;
MimeType=inode/directory;
Keywords=folder;files;explorer;browser;manager;
Actions=new-window;home;

[Desktop Action new-window]
Name=New Window
Exec={e}

[Desktop Action home]
Name=Home Folder
Exec={e} --home
"
    )
}

pub fn is_installed() -> bool {
    entry_path().is_file()
}

/// Write the launcher entry pointing at the program that is running now.
pub fn install() -> Result<PathBuf, String> {
    let exe = this_build().ok_or("Could not find this program's own location.")?;
    let p = entry_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, entry_text(&exe)).map_err(|e| e.to_string())?;
    let ip = icon_path();
    if let Some(d) = ip.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(ip, ICON_SVG);
    refresh_database();
    Ok(p)
}

pub fn uninstall() -> Result<(), String> {
    let _ = std::fs::remove_file(icon_path());
    match std::fs::remove_file(entry_path()) {
        Ok(()) => {
            refresh_database();
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn refresh_database() {
    let dir = places::data_home().join("applications");
    let _ = Command::new("update-desktop-database").arg(dir).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

pub fn is_default_file_manager() -> bool {
    crate::launch::mimeapps_default("inode/directory").as_deref() == Some(DESKTOP_ID)
}

/// Make folders open in Rivet everywhere (desktop, other apps' "Show in folder").
pub fn set_default_file_manager() -> Result<(), String> {
    if !is_installed() {
        install()?;
    }
    let ok = Command::new("xdg-mime")
        .args(["default", DESKTOP_ID, "inode/directory"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok || write_mimeapps_default() {
        Ok(())
    } else {
        Err("Could not update the default applications list.".into())
    }
}

/// Fallback when xdg-mime is missing: edit ~/.config/mimeapps.list directly.
fn write_mimeapps_default() -> bool {
    let p = places::config_home().join("mimeapps.list");
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let mut out = Vec::new();
    let mut in_default = false;
    let mut done = false;
    for line in text.lines() {
        if line.trim().starts_with('[') {
            if in_default && !done {
                out.push(format!("inode/directory={DESKTOP_ID}"));
                done = true;
            }
            in_default = line.trim() == "[Default Applications]";
            out.push(line.to_string());
            continue;
        }
        if in_default && line.trim_start().starts_with("inode/directory=") {
            out.push(format!("inode/directory={DESKTOP_ID}"));
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        if !in_default {
            out.push("[Default Applications]".into());
        }
        out.push(format!("inode/directory={DESKTOP_ID}"));
    }
    std::fs::write(&p, out.join("\n") + "\n").is_ok()
}

// ---------------------------------------------------------------- `rivet` command link

/// Where a global `rivet` command can live.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LinkSpot {
    /// ~/.local/bin — just for you, no password.
    User,
    /// /usr/bin — for everyone, needs an administrator password.
    System,
}

impl LinkSpot {
    pub fn path(self) -> PathBuf {
        match self {
            LinkSpot::User => std::env::var_os("XDG_BIN_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| places::home().join(".local/bin"))
                .join("rivet"),
            LinkSpot::System => PathBuf::from("/usr/bin/rivet"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum LinkState {
    Missing,
    /// A link to the program that is running now.
    ThisBuild,
    /// A link to some other file.
    Elsewhere(PathBuf),
    /// A real file (e.g. from a package) — never overwritten.
    NotALink,
}

fn this_build() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok().map(|p| std::fs::canonicalize(&p).unwrap_or(p))?;
    // Inside an AppImage the running file lives in a temporary mount; point at the AppImage itself.
    // APPIMAGE / APPDIR are inherited by every program an AppImage starts, so only trust them
    // when this program really runs from that mount (not when another AppImage app launched us).
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from).map(|d| std::fs::canonicalize(&d).unwrap_or(d));
    if let (Some(a), Some(dir)) = (std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()), appdir) {
        if exe.starts_with(&dir) {
            return Some(a);
        }
    }
    Some(exe)
}

pub fn link_state(spot: LinkSpot) -> LinkState {
    let p = spot.path();
    let Ok(meta) = std::fs::symlink_metadata(&p) else { return LinkState::Missing };
    if !meta.file_type().is_symlink() {
        return LinkState::NotALink;
    }
    let target = std::fs::canonicalize(&p).unwrap_or_else(|_| std::fs::read_link(&p).unwrap_or_default());
    if Some(&target) == this_build().as_ref() { LinkState::ThisBuild } else { LinkState::Elsewhere(target) }
}

/// The shell command that makes the link (shown so it can be run by hand).
pub fn link_command(spot: LinkSpot) -> String {
    let exe = this_build().unwrap_or_default();
    let q = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
    let cmd = format!("ln -sfn {} {}", q(&exe), q(&spot.path()));
    if spot == LinkSpot::System { format!("sudo {cmd}") } else { cmd }
}

/// Point the `rivet` command at this build, so `rivet` works from anywhere and
/// every rebuild is picked up automatically.
pub fn link_command_here(spot: LinkSpot) -> Result<String, String> {
    if link_state(spot) == LinkState::NotALink {
        return Err(format!("{} is a real program file (probably from a package), so it was left alone.", spot.path().display()));
    }
    let exe = this_build().ok_or("Could not find this program's own location.")?;
    let dest = spot.path();
    match spot {
        LinkSpot::User => {
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let _ = std::fs::remove_file(&dest);
            std::os::unix::fs::symlink(&exe, &dest).map_err(|e| e.to_string())?;
            Ok(format!("The rivet command now opens this build ({}).", dest.display()))
        }
        LinkSpot::System => {
            // pkexec asks for the administrator password with the desktop's own dialog.
            let ok = Command::new("pkexec")
                .args(["ln", "-sfn"])
                .arg(&exe)
                .arg(&dest)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                Ok(format!("The rivet command now opens this build for everyone ({}).", dest.display()))
            } else {
                Err(format!(
                    "The password prompt was canceled or isn't available. You can run this in a terminal instead (it's on your clipboard):\n\n{}",
                    link_command(spot)
                ))
            }
        }
    }
}

pub fn unlink_command(spot: LinkSpot) -> Result<(), String> {
    match link_state(spot) {
        LinkState::Missing => Ok(()),
        LinkState::NotALink => Err("That is a real program file, so it was left alone.".into()),
        _ => match spot {
            LinkSpot::User => std::fs::remove_file(spot.path()).map_err(|e| e.to_string()),
            LinkSpot::System => {
                let ok = Command::new("pkexec").arg("rm").arg(spot.path()).status().map(|s| s.success()).unwrap_or(false);
                if ok { Ok(()) } else { Err("The password prompt was canceled or isn't available.".into()) }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_icon_renders() {
        let img = crate::icons::render_svg(super::ICON_SVG.as_bytes(), 256).expect("icon renders");
        assert!(img.pixels.iter().filter(|p| p.a() > 0).count() > 10_000);
        // RIVET_ICON_PREVIEW=path.png writes it out for a look.
        if let Some(p) = std::env::var_os("RIVET_ICON_PREVIEW") {
            let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
            image::save_buffer(p, &rgba, 256, 256, image::ExtendedColorType::Rgba8).unwrap();
        }
    }

    #[test]
    fn user_command_link_points_at_this_build() {
        let dir = std::env::temp_dir().join(format!("rivet-bin-{}", std::process::id()));
        // SAFETY: only this test reads XDG_BIN_HOME.
        unsafe { std::env::set_var("XDG_BIN_HOME", &dir) };
        use super::*;
        // Started by some other AppImage app: its APPIMAGE / APPDIR are inherited,
        // but this build must still point at itself, not at that AppImage.
        let other = std::env::temp_dir().join(format!("other-app-{}.AppImage", std::process::id()));
        std::fs::write(&other, "x").unwrap();
        unsafe {
            std::env::set_var("APPIMAGE", &other);
            std::env::set_var("APPDIR", std::env::temp_dir().join("some-other-mount"));
        }
        let me = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        assert_eq!(this_build(), Some(me));
        unsafe {
            std::env::remove_var("APPIMAGE");
            std::env::remove_var("APPDIR");
        }
        std::fs::remove_file(&other).unwrap();
        assert_eq!(link_state(LinkSpot::User), LinkState::Missing);
        link_command_here(LinkSpot::User).unwrap();
        assert_eq!(link_state(LinkSpot::User), LinkState::ThisBuild);
        // A real file is never replaced.
        unlink_command(LinkSpot::User).unwrap();
        std::fs::write(LinkSpot::User.path(), "x").unwrap();
        assert_eq!(link_state(LinkSpot::User), LinkState::NotALink);
        assert!(link_command_here(LinkSpot::User).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
        unsafe { std::env::remove_var("XDG_BIN_HOME") };
    }

    #[test]
    fn exec_line_quotes_spaces() {
        let t = super::entry_text(std::path::Path::new("/home/me/My Apps/rivet"));
        assert!(t.contains("Exec=\"/home/me/My Apps/rivet\" %U"));
        assert!(t.contains("Comment=Browse and organize your files"));
        assert!(!t.contains("organise"));
        let app = crate::launch::DesktopApp {
            id: "x".into(),
            name: "x".into(),
            exec: "\"/home/me/My Apps/rivet\" %U".into(),
            icon: String::new(),
            mimes: vec![],
            terminal: false,
        };
        assert_eq!(crate::launch::exec_args(&app, &[std::path::PathBuf::from("/tmp")]), vec!["/home/me/My Apps/rivet".to_string(), "/tmp".to_string()]);
    }
}
