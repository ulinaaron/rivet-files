//! Command-line help, and the check for libraries winit loads with dlopen.
//!
//! xkbcommon-dl panics (`Library libxkbcommon-x11.so could not be loaded.`) when
//! the X11 keyboard library is missing. That panic is inside a dependency, so
//! Rivet looks for the same sonames first and exits with install commands.

use std::fmt::Write as _;

/// Which display server winit will pick. Empty variables count as unset, and
/// Wayland wins when both are set — the same rules as winit 0.30.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    Wayland,
    X11,
    Headless,
}

pub fn session(wayland: bool, x11: bool) -> Session {
    if wayland {
        Session::Wayland
    } else if x11 {
        Session::X11
    } else {
        Session::Headless
    }
}

pub fn current_session() -> Session {
    let set = |key: &str| std::env::var(key).ok().is_some_and(|v| !v.is_empty());
    session(set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET"), set("DISPLAY"))
}

/// Sonames to try, in order. The `.so.0` name is what distributions ship.
pub fn required_libraries(session: Session) -> Vec<&'static [&'static str]> {
    match session {
        Session::Headless => Vec::new(),
        // Wayland uses libxkbcommon itself. It does not load the X11 addon.
        Session::Wayland => vec![&["libxkbcommon.so.0", "libxkbcommon.so"]],
        // X11 loads both. libxkbcommon-x11 also needs libxcb-xkb, which dlopen
        // reports if that dependency is the piece that's missing.
        Session::X11 => vec![
            &["libxkbcommon.so.0", "libxkbcommon.so"],
            &["libxkbcommon-x11.so.0", "libxkbcommon-x11.so"],
        ],
    }
}

/// `Ok` when one of `names` can be dlopened. The handle is kept for the rest
/// of the process so a later load (winit) sees the same mapping.
#[cfg(target_os = "linux")]
pub fn load_any(names: &[&str]) -> Result<(), String> {
    use std::ffi::{CStr, CString};
    unsafe { libc::dlerror() };
    let mut errors = Vec::new();
    for name in names {
        let c = CString::new(*name).map_err(|_| format!("{name}: embedded NUL"))?;
        let handle = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
        if !handle.is_null() {
            return Ok(());
        }
        let err = unsafe { libc::dlerror() };
        let detail = if err.is_null() {
            "could not be loaded".to_string()
        } else {
            unsafe { CStr::from_ptr(err) }.to_string_lossy().into_owned()
        };
        // dlerror already names the file. Keep the first attempt; the later
        // names are fallbacks (.so without a version).
        if errors.is_empty() {
            errors.push(detail);
        }
    }
    Err(errors.join("\n"))
}

#[cfg(not(target_os = "linux"))]
pub fn load_any(names: &[&str]) -> Result<(), String> {
    let _ = names;
    Ok(())
}

/// Text for a missing library. `missing` is `(soname, dlerror text)`.
pub fn missing_libraries_message(session: Session, missing: &[(&str, &str)]) -> String {
    let mut out = String::from(
        "Rivet can't open a window because a library it needs for keyboard input is missing.\n\
         Install it, then run Rivet again.\n\n",
    );
    for (_name, detail) in missing {
        let _ = writeln!(out, "{}", detail.trim());
    }
    out.push('\n');
    match session {
        Session::X11 => out.push_str(
            "On X11, Rivet loads libxkbcommon-x11. On Debian and Ubuntu that package also installs libxcb-xkb1.\n\n",
        ),
        Session::Wayland => out.push_str("On Wayland, Rivet loads libxkbcommon.\n\n"),
        Session::Headless => {}
    }
    let x11 = missing.iter().any(|(name, _)| name.contains("xkbcommon-x11"));
    // A backslash-newline in a Rust string drops the next line's leading
    // spaces, so these lines are written out in full.
    if x11 {
        out.push_str("  Debian or Ubuntu:  sudo apt install libxkbcommon-x11-0\n");
        out.push_str("  Fedora or RHEL:    sudo dnf install libxkbcommon-x11\n");
        out.push_str("  Arch Linux:        sudo pacman -S libxkbcommon\n");
        out.push_str("  openSUSE:          sudo zypper install libxkbcommon-x11-0\n");
    } else {
        out.push_str("  Debian or Ubuntu:  sudo apt install libxkbcommon0\n");
        out.push_str("  Fedora or RHEL:    sudo dnf install libxkbcommon\n");
        out.push_str("  Arch Linux:        sudo pacman -S libxkbcommon\n");
        out.push_str("  openSUSE:          sudo zypper install libxkbcommon0\n");
    }
    if x11 && missing.iter().any(|(name, _)| name.starts_with("libxkbcommon.so")) {
        out.push_str(
            "\nlibxkbcommon itself was missing too. The package above depends on it \
             (Debian and Ubuntu: libxkbcommon0, Fedora: libxkbcommon).\n",
        );
    }
    out
}

/// Before `eframe` starts: winit's X11 backend panics if these libraries are absent.
pub fn ensure_gui_libraries() -> Result<(), String> {
    let session = current_session();
    let mut missing = Vec::new();
    for names in required_libraries(session) {
        if let Err(detail) = load_any(names) {
            missing.push((names[0], detail));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        let borrowed: Vec<(&str, &str)> = missing.iter().map(|(n, d)| (*n, d.as_str())).collect();
        Err(missing_libraries_message(session, &borrowed))
    }
}

pub fn usage() -> String {
    format!(
        "\
Rivet Files {version} — a classic Linux file manager

Usage: rivet [OPTIONS] [FOLDER]

With no FOLDER, Rivet opens the last place you were. A path, ~, or a file:// URL opens that folder.

Options:
  -h, --help                  Show this help and exit
  -V, --version               Print the version and exit
      --home                  Open the home folder
      --view MODE             Start in icons, list, or compact view
      --tree                  Show the sidebar as a folder tree
      --hide-menubar          Start with the menu bar hidden
      --decorations           Use the desktop's title bar
      --no-decorations        Use Rivet's own title bar
      --csd                   Same as --no-decorations
      --screenshot FILE.png   Save a picture of the window and exit
                              (--screenshot=FILE.png also works)
      --measure SECS          Open a window and exit after SECS seconds (default 15)
      --bench [FOLDER...]     Time folder reads without a window, then exit
      --export-icons [DIR]    Write the logo icons and exit (default: data/icons)
      --install-desktop-entry Add Rivet to the app launcher and exit
",
        version = env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_matches_winit() {
        assert_eq!(session(false, false), Session::Headless);
        assert_eq!(session(false, true), Session::X11);
        assert_eq!(session(true, false), Session::Wayland);
        // Wayland is preferred when both variables are set.
        assert_eq!(session(true, true), Session::Wayland);
    }

    #[test]
    fn x11_needs_the_x11_library_and_wayland_does_not() {
        let x11 = required_libraries(Session::X11);
        assert!(x11.iter().any(|names| names.contains(&"libxkbcommon-x11.so.0")));
        assert!(x11.iter().any(|names| names.contains(&"libxkbcommon.so.0")));
        let wayland = required_libraries(Session::Wayland);
        assert!(wayland.iter().all(|names| !names.iter().any(|n| n.contains("x11"))));
        assert!(required_libraries(Session::Headless).is_empty());
    }

    #[test]
    fn missing_x11_library_names_every_distro() {
        let text = missing_libraries_message(
            Session::X11,
            &[("libxkbcommon-x11.so.0", "libxkbcommon-x11.so.0: cannot open shared object file")],
        );
        assert!(text.contains("can't open a window"));
        assert!(text.contains("libxcb-xkb1"));
        assert!(text.contains("sudo apt install libxkbcommon-x11-0"));
        assert!(text.contains("sudo dnf install libxkbcommon-x11"));
        assert!(text.contains("sudo pacman -S libxkbcommon"));
        assert!(text.contains("sudo zypper install libxkbcommon-x11-0"));
        assert!(!text.contains("panicked"));
    }

    #[test]
    fn missing_wayland_library_asks_for_libxkbcommon() {
        let text = missing_libraries_message(Session::Wayland, &[("libxkbcommon.so.0", "not found")]);
        assert!(text.contains("sudo apt install libxkbcommon0"));
        assert!(!text.contains("libxkbcommon-x11-0"));
    }

    #[test]
    fn dlopen_finds_libc_and_reports_a_missing_name() {
        assert!(load_any(&["libc.so.6"]).is_ok());
        let err = load_any(&["libthis-is-not-a-rivet-library.so.0"]).unwrap_err();
        assert!(err.contains("libthis-is-not-a-rivet-library.so.0"));
    }

    #[test]
    fn help_lists_every_option() {
        let help = usage();
        for flag in [
            "-h",
            "--help",
            "-V",
            "--version",
            "--home",
            "--view",
            "--tree",
            "--hide-menubar",
            "--decorations",
            "--no-decorations",
            "--csd",
            "--screenshot",
            "--measure",
            "--bench",
            "--export-icons",
            "--install-desktop-entry",
        ] {
            assert!(help.contains(flag), "help omits {flag}");
        }
        assert!(help.contains("default 15"));
        assert!(help.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn readme_stays_in_sync_with_help() {
        let readme = include_str!("../README.md");
        assert!(readme.contains("A Linux file manager"), "tagline has a double space or changed");
        assert!(!readme.contains("A  Linux"));
        for flag in [
            "--help",
            "--version",
            "--home",
            "--view",
            "--tree",
            "--hide-menubar",
            "--decorations",
            "--no-decorations",
            "--csd",
            "--screenshot",
            "--measure",
            "--bench",
            "--export-icons",
            "--install-desktop-entry",
        ] {
            assert!(readme.contains(flag), "README omits {flag}");
        }
        assert!(readme.contains("libxkbcommon-x11-0"));
        assert!(readme.contains("libxcb-xkb1"));
        assert!(readme.contains("libxkbcommon0"));
        for word in ["Behaviour", "organise", "Customise", "Maximise", "colour", "cancelled", "maximise"] {
            assert!(!readme.contains(word), "README contains {word}");
        }
        for (label, text) in [
            ("CHANGELOG", include_str!("../CHANGELOG.md")),
            ("desktop entry", include_str!("../data/rivet-files.desktop")),
        ] {
            for word in ["Behaviour", "organise", "Customise", "Maximise", "colour", "cancelled", "maximise", "recoloured", "Customisable"] {
                assert!(!text.contains(word), "{label} contains {word}");
            }
        }
    }
}
