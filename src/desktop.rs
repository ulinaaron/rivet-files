//! The launcher entry (freedesktop .desktop file) and default-file-manager setting.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::places;

pub const DESKTOP_ID: &str = "rivet-files.desktop";

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
Comment=Browse and organise your files
Exec={e} %U
Icon=system-file-manager
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

/// The file our entry points at, if one is installed.
pub fn installed_exec() -> Option<String> {
    let t = std::fs::read_to_string(entry_path()).ok()?;
    t.lines().find_map(|l| l.strip_prefix("Exec=").map(|s| s.to_string()))
}

pub fn is_installed() -> bool {
    entry_path().is_file()
}

/// Write the launcher entry pointing at the program that is running now.
pub fn install() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let p = entry_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, entry_text(&exe)).map_err(|e| e.to_string())?;
    refresh_database();
    Ok(p)
}

pub fn uninstall() -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    #[test]
    fn exec_line_quotes_spaces() {
        let t = super::entry_text(std::path::Path::new("/home/me/My Apps/rivet"));
        assert!(t.contains("Exec=\"/home/me/My Apps/rivet\" %U"));
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
