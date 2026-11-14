<img src="data/rivet-files.svg" width="96" alt="Rivet Files icon">

# Rivet Files

[![CI](https://github.com/ulinaaron/rivet-files/actions/workflows/ci.yml/badge.svg)](https://github.com/ulinaaron/rivet-files/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/ulinaaron/rivet-files?label=release)](https://github.com/ulinaaron/rivet-files/releases/latest)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#-license)
[![Platform: Linux](https://img.shields.io/badge/platform-Linux-informational)](#-install)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20egui-orange)](https://github.com/emilk/egui)
![No GTK or Qt](https://img.shields.io/badge/GTK%20%2F%20Qt-none-success)

A Linux file manager in the same vein as Dolphin, Nemo, Thunar and Caja — written in Rust. Does not use GTK or Qt.

Rivet is powerful all the while being minimal. Make it your own — bloat the interface with every button and panel imaginable. Or strip it back and keep just a few controls visible.

![Rivet Files showing a home folder](docs/screenshots/icons-wide.png)

<table>
  <tr>
    <td><img src="docs/screenshots/minimal-wide.png" alt="Minimal: no menu bar, status bar or Peek; a short toolbar"></td>
    <td><img src="docs/screenshots/bloated-wide.png" alt="Bloated: menu bar, full toolbar, tabs, status bar and Peek"></td>
  </tr>
  <tr>
    <td align="center">Minimal — menu button, back / forward, location bar, two views</td>
    <td align="center">Bloated — every bar, button and panel</td>
  </tr>
</table>

## ✨ Features

**Getting around**
- 🪜 **Stepped path bar** — each folder in the path is its own button; click one to jump straight to it
- 🔮 **Fuzzy suggestions** — type a path and matching folders appear as you go, even from a few scattered letters; Tab completes
- 🗂️ **Three views** — icons, list and compact, with tabs
- 🧭 **Sidebar** — places, bookmarks, drives with usage bars and network shares, or a folder tree; left or right
- 🔦 **Quick Open** (Ctrl+P) — fuzzy-find any folder and open it
- ⚙️ **Quick Actions** (Ctrl+Shift+P) — every command and setting in one searchable list
- 🧪 **Filters** — limit a folder to certain types, names or dates; globally, per folder, or for a folder and its subfolders
- 🔍 **Search** — type to filter the folder; Enter searches subfolders too
- ⌨️ **Vim keys** (optional) — `hjkl`, `gg` `G`, counts, visual mode, `yy` `dd` `p`
- 🌐 **Network** — browse servers, open SMB shares, or connect to an address

**Working with files**
- 📋 **The basics** — copy, cut, paste, drag and drop, rename, Trash with restore
- ↩️ **Undo and redo** — copies, moves, renames, Trash, new items, compress and extract
- ✏️ **Bulk Rename** — insert text, dates or numbers, remove, replace, change case; every new name previewed first
- 🔄 **Convert and resize** — pictures, video and audio to other formats; resize pictures (needs ffmpeg)
- 🗜️ **Compress and extract** — zip, tar.gz, tar.xz, tar.zst, 7z
- 🚀 **Open With** — a searchable list of apps; set a new default

**Previews**
- 🖼️ **Thumbnails** for pictures, alongside your system icon theme
- 👀 **Peek panel** (F11) — large previews, video playback, syntax-highlighted code and file details; left or right
- 🔎 **Preview** (Space) — full-window view; ← → step through the folder's pictures, videos and text

**Customizing**
- 🎨 **Desktop colors** — follows Matugen, Noctalia, Dux Shell and GTK themes as they change
- 💜 **Built-in icons** recolored from your palette (primary, secondary or tertiary)
- 🧰 **Layout** — toolbar top or bottom, status bar top, bottom or off, your own toolbar buttons, fonts and text size, optional menu bar
- 🧘 **Zen mode** (F12) — the files fill the window; the toolbar, sidebar and Peek slide in at the edges
- 🖥️ **Terminal mode** (F7) — monospace text, Nerd Font icons and boxed panels
- 🧩 **Modules** — turn off Bulk Rename or Convert & Resize
- 🪟 **Title bar** — your desktop's, or Rivet's own

**Under the hood**
- ⚡ **Fast** — opens a 100,000-file folder in under 0.4 s; no CPU use when idle
- 🦀 **Small** — one 14 MB binary or a 6 MB AppImage; ffmpeg is optional (video previews and conversion)
- 🩹 **Crash reports** — saved on your computer and shown on the next start; nothing is sent unless you choose to share it

## 🎨 Follows your colors

Change your wallpaper or scheme and Rivet restyles itself within a second.

<table>
  <tr>
    <td><img src="docs/screenshots/theme-ember-wide.png" alt="Ember: warm dark scheme"></td>
    <td><img src="docs/screenshots/theme-moss-wide.png" alt="Moss: green dark scheme"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/theme-violet-wide.png" alt="Violet: purple dark scheme"></td>
    <td><img src="docs/screenshots/theme-rose-wide.png" alt="Rose: light scheme"></td>
  </tr>
</table>

<table>
  <tr>
    <td><img src="docs/screenshots/list-wide.png" alt="List view of a Documents folder"></td>
    <td><img src="docs/screenshots/rivet-icons-wide.png" alt="Rivet's own icon set"></td>
    <td><img src="docs/screenshots/preferences-wide.png" alt="Preferences window"></td>
  </tr>
  <tr>
    <td align="center">List view</td>
    <td align="center">Built-in icons</td>
    <td align="center">Preferences</td>
  </tr>
</table>

### 👀 Peek

The Peek panel (F11) previews the selected item: pictures, video playback, album art, and text with
syntax highlighting in your theme's colors, followed by the file's details. Drag the grip under the
preview to change its height.

<table>
  <tr>
    <td><img src="docs/screenshots/peek-code-wide.png" alt="Peek panel showing highlighted Rust code"></td>
    <td><img src="docs/screenshots/peek-picture-wide.png" alt="Peek panel showing a picture and its details"></td>
  </tr>
  <tr>
    <td align="center">Code, colored by your theme</td>
    <td align="center">Pictures and details</td>
  </tr>
</table>

### 🧰 Tools

<table>
  <tr>
    <td><img src="docs/screenshots/filters-wide.png" alt="The Filters panel: this folder shows only folders and pictures"></td>
    <td><img src="docs/screenshots/bulk-rename-wide.png" alt="Bulk Rename with a live preview of the new names"></td>
  </tr>
  <tr>
    <td align="center">Filters — this folder shows only folders and pictures</td>
    <td align="center">Bulk Rename — every new name previewed first</td>
  </tr>
</table>

### 🧘 Zen mode, Quick Open and Quick Actions

Zen mode (F12) hides the bars and lets the files fill the window. Move the pointer to an edge to
slide in the toolbar, sidebar or Peek; move away to hide it. Quick Open (Ctrl+P) finds any folder
from a few letters. Quick Actions (Ctrl+Shift+P) runs any command or setting by name, such as
"tree", "theme dark" or "zen".

<table>
  <tr>
    <td><img src="docs/screenshots/zen-wide.png" alt="Zen mode with the toolbar and sidebar slid in"></td>
    <td><img src="docs/screenshots/quick-open-wide.png" alt="Quick Open jumping to a folder"></td>
  </tr>
</table>

### 🖥️ Terminal mode

Terminal mode (F7, or Mode in Preferences → Appearance) styles Rivet like a terminal program:
monospace text, Nerd Font icons colored by file type, and panels drawn as boxes with their titles in
the border. The layout and features don't change. It uses your Monospace Font (Preferences →
Appearance → Typography) and any installed Nerd Font.

<img src="docs/screenshots/terminal-wide.png" alt="Rivet in terminal mode: monospace text, Nerd Font icons and boxed panels">

## 📦 Install

Grab the latest from the [Releases page](https://github.com/ulinaaron/rivet-files/releases):

- **AppImage** — one file, nothing else to install: `chmod +x Rivet_Files-*.AppImage` and run it.
  It includes the keyboard libraries below and runs on Ubuntu 22.04, Debian 12, and newer.
  Preferences → System → *Add Rivet to the app launcher* puts it in your menus.
- **Archive** — unpack `rivet-files-*-linux.tar.gz` and run `./install.sh`
  (add `--default` to make Rivet your default file manager). Same glibc baseline as the AppImage.
  Install the libraries below if the desktop does not already have them.
- **From source** — `utilities/rivet-dev install` (builds a release and adds Rivet to your app launcher)
- **Flatpak** — a `.flatpak` bundle is attached to GitHub releases once CI has built it.
  It is not on Flathub until that submission is done; the manifest and the steps are in [packaging/README.md](packaging/README.md).
  Inside the sandbox, home, other disks, Trash and `~/.config` theme files are the real ones. Host `/usr` and `/etc` are under **Host system files** (`/run/host`).
- **Arch Linux** — PKGBUILDs for `rivet-files-bin` (the release tarball) and `rivet-files` (built from the git tag) are in `packaging/aur`. They are not in the AUR until published; see [packaging/README.md](packaging/README.md).

### Libraries the window needs

Rivet talks to X11 or Wayland directly. It does not use GTK or Qt. A normal desktop already has the display libraries (`libX11`, `libwayland-client`, `libwayland-cursor`, OpenGL). What a minimal install often lacks is the keyboard library loaded at startup: `libxkbcommon` on Wayland, and `libxkbcommon-x11` plus `libxcb-xkb` on X11. If one is missing, Rivet prints the install command instead of crashing. The AppImage already carries those three.

| Distro | Install |
| --- | --- |
| Debian, Ubuntu | `sudo apt install libxkbcommon0 libxkbcommon-x11-0 libxcb-xkb1` |
| Fedora, RHEL | `sudo dnf install libxkbcommon libxkbcommon-x11` |
| Arch Linux | `sudo pacman -S libxkbcommon` |
| openSUSE | `sudo zypper install libxkbcommon0 libxkbcommon-x11-0` |

## ⌨️ Handy shortcuts

| Keys | Action |
| --- | --- |
| `Ctrl+L` · `Tab` | Type a location · complete the suggested folder |
| `Ctrl+F` or just type | Filter this folder |
| `Ctrl+T` / `Ctrl+W` | New / close tab |
| `Ctrl+H` | Show hidden files |
| `Ctrl+1` `2` `3` | Icon / list / compact view |
| `F2` · `Delete` | Rename (Bulk Rename with several selected) · move to Trash |
| `Ctrl+M` | Show or hide the menu bar |
| `Ctrl+P` · `Ctrl+Shift+P` | Quick Open: go to any folder · Quick Actions: any command or option |
| `F7` · `F8` · `F9` · `F11` · `F12` | Terminal mode · status bar · sidebar · Peek panel · Zen mode |
| `Ctrl+Shift+F` | Filters |
| `Space` · `←` `→` · `Return` | Preview the selection · previous / next file · open it |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / redo |
| `Ctrl+,` | Preferences |
| `F1` | All shortcuts |

Prefer Vim keys? Turn them on in Preferences → Behavior: `h` `j` `k` `l` move (in the list view
`h` `l` go to the parent folder and open), `-` parent folder, `o` open, `gg` `G` first / last,
counts (`5j`, `10G`), `V` visual selection, `yy` `dd` `p` copy / cut / paste, `cw` rename,
`D` Trash, `.` hidden files, `H` `L` back / forward, `/` search, `:` Quick Actions.

## ⚡ Performance

**100,000 files** — one folder of empty files on btrfs, a fresh launch each time, median of five runs
(`utilities/rivet-dev bench`). *First window* is when the compositor lists the window,
*settled* is when the process has used no CPU for 500 ms, *PSS* is memory, *CPU* is total
processor time up to that point — the same method as [Flea's benchmark](https://github.com/thisisgm/flea#performance).

| | First window | Settled | PSS | CPU |
| --- | ---: | ---: | ---: | ---: |
| **Rivet** 0.1.1, list | 385 ms | 0.42 s | 76.8 MiB | 1.05 s |
| **Rivet** 0.1.1, icons | 385 ms | 0.43 s | 76.9 MiB | 1.05 s |

For context, Flea publishes these figures from **their own machine** with the same method
(we have not run these apps here, so treat it as a rough comparison):

| | First window | Settled | PSS | CPU |
| --- | ---: | ---: | ---: | ---: |
| Flea 0.4.0 | 319 ms | 0.75 s | 63.0 MiB | 0.53 s |
| Dolphin 26.08 | 791 ms | 5.25 s | 235.1 MiB | 8.18 s |
| Nautilus 51.1 | 931 ms | 20.33 s | 337.2 MiB | 21.67 s |
| Nemo 6.6 | 928 ms | 22.61 s | 479.3 MiB | 26.03 s |
| PCManFM 1.4 | 479 ms | 35.29 s | 124.4 MiB | 24.25 s |
| Thunar 4.20 | 601 ms | 19.71 s | 394.9 MiB | 19.82 s |

Rivet reads big folders on all CPU cores, so its CPU total is higher than its wall-clock time.
When idle it uses no CPU at all: a small background watcher wakes the window only when a
folder, your colors or your drives actually change.

<details>
<summary><b>More numbers</b></summary>

Release build on Fedora, Wayland, NVMe SSD:

| | |
| --- | --- |
| Drawing a frame (100,000 items) | ~2 ms |
| Reading 2,092 / 100,000 items | ~19 ms / ~156 ms |
| Sorting 100,000 items by name / type | ~17 ms / ~27 ms |
| Indexing icon themes (once) | ~26 ms |
| Binary size (stripped) | ~14 MB (AppImage ~6 MB) |

Reproduce with `rivet --bench [FOLDER…]`, `RIVET_FRAME_STATS=1 rivet --screenshot out.png`
and `utilities/rivet-dev bench`.

</details>

## 🗺️ Roadmap

- Freedesktop file picker integration
- Tagging and pinning files

## 🛠️ For developers

<details>
<summary><b>Build, run and options</b></summary>

```sh
cargo build --release
./target/release/rivet [FOLDER]
```

`rivet [FOLDER]` opens that folder (`~` and `file://` URLs work). `rivet --help` lists the same options:

| Option | |
| --- | --- |
| `-h`, `--help` | Show this help and exit |
| `-V`, `--version` | Print the version and exit |
| `--home` | Open the home folder |
| `--view icons\|list\|compact` | Starting view |
| `--tree` | Show the sidebar as a folder tree |
| `--hide-menubar` | Start with the menu bar hidden |
| `--decorations` | Use the desktop's title bar |
| `--no-decorations`, `--csd` | Use Rivet's own title bar |
| `--screenshot FILE.png` | Save a picture of the window and exit |
| `--measure SECS` | Open a window and exit after SECS seconds (default 15) |
| `--bench [FOLDER...]` | Time folder reads without a window, then exit |
| `--export-icons [DIR]` | Write the logo icons and exit (default `data/icons`) |
| `--install-desktop-entry` | Add Rivet to the app launcher and exit |

A global `rivet` command that always runs your latest build: `utilities/rivet-dev link` (system-wide, asks for
your password) or `utilities/rivet-dev link --user` (`~/.local/bin`). Preferences → System has the same buttons.

</details>

<details>
<summary><b>Where the colors come from</b></summary>

In order of priority:

1. `~/.config/rivet/colors.css` (`@define-color name #rrggbb;`)
2. whichever is newer: your matugen outputs (learned by lining each template in
   `~/.config/matugen/config.toml` up with its output) or generated GTK color files
   (`~/.config/gtk-{3,4}.0/gtk.css` / `colors.css`, following `@import`)
3. the defaults of the installed GTK theme

Preferences → *Add Rivet template to matugen* registers a dedicated template if you prefer one.

</details>

<details>
<summary><b>Screenshots, logo and releases</b></summary>

- **All developer tasks** run through one command: `utilities/rivet-dev help` lists them (the scripts behind it live in `utilities/lib/`).
- **Screenshots** come from a throwaway sample home folder: `utilities/rivet-dev screenshots`
- **Logo** master is `data/rivet-files.svg`; regenerate the PNG set with
  `rivet --export-icons data/icons` and copy the 512 px one to `docs/logo.png`
- **AppImage**: `utilities/rivet-dev appimage` (needs `squashfs-tools`). It links for glibc 2.31 and bundles the keyboard libraries, so the one file runs on Ubuntu 22.04 and Debian 12.
- **Release**: keep notes under `## Unreleased` in `CHANGELOG.md`. `utilities/rivet-dev bump` (patch, or `minor` / `major` / `X.Y.Z`)
  sets the version, turns that section into the release's (or writes one from the commits) and commits; then `utilities/rivet-dev release --publish`. Plain `release` only builds and tags on your computer; `push` only sends your commits to GitHub, with no release. The release binary is linked with Zig against glibc 2.31 (not the build machine's libc). CI rejects an ELF that needs a GLIBC symbol newer than 2.35.

</details>

## 📄 License

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
