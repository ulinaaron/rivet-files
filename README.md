<img src="data/rivet-files.svg" width="96" alt="Rivet Files icon">

# Rivet Files

[![CI](https://github.com/ulinaaron/rivet-files/actions/workflows/ci.yml/badge.svg)](https://github.com/ulinaaron/rivet-files/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/ulinaaron/rivet-files?label=release)](https://github.com/ulinaaron/rivet-files/releases/latest)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#-license)
[![Platform: Linux](https://img.shields.io/badge/platform-Linux-informational)](#-install)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20egui-orange)](https://github.com/emilk/egui)
![No GTK or Qt](https://img.shields.io/badge/GTK%20%2F%20Qt-none-success)

A classic Linux file manager in the same family as Nemo, Thunar and Caja — written in Rust, with no GTK or Qt.

![Rivet Files showing a home folder](docs/screenshots/icons-wide.png)

## ✨ Features

- 🗂️ **Icon, list and compact views** with tabs and a clickable path bar
- 🧭 **Sidebar** — places, bookmarks, drives with usage bars, network, or a folder tree
- 📋 **Everything you expect** — copy, cut, paste, drag and drop, rename, Trash with restore
- 🔍 **Instant filter** as you type, plus search through subfolders
- 🖼️ **Picture thumbnails** and your system icon theme
- 🎨 **Follows your colours** — matugen, Noctalia and GTK themes, live, no restart
- 💜 **Built-in icon set** recoloured from your palette (primary, secondary or tertiary)
- 🧰 **Make it yours** — custom toolbar, fonts, text size, hideable menu bar
- 🪟 **Optional window decorations** — system title bar or a built-in one
- ⚡ **Fast** — ~0.2 s to open, ~1 ms per frame, thousands of files without a stutter
- 🦀 **Light** — one 12 MB program, no toolkit to install

## 🎨 Follows your colours

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

## 📦 Install

Grab the latest from the [Releases page](https://github.com/ulinaaron/rivet-files/releases):

- **AppImage** — one file: `chmod +x Rivet_Files-*.AppImage` and run it.
  Preferences → System → *Add Rivet to the app launcher* puts it in your menus.
- **Archive** — unpack `rivet-files-*-linux.tar.gz` and run `./install.sh`
  (add `--default` to make Rivet your default file manager).
- **From source** — `cargo build --release && ./install.sh`

## ⌨️ Handy shortcuts

| Keys | Action |
| --- | --- |
| `Ctrl+L` | Type a location |
| `Ctrl+F` or just type | Filter this folder |
| `Ctrl+T` / `Ctrl+W` | New / close tab |
| `Ctrl+H` | Show hidden files |
| `Ctrl+1` `2` `3` | Icon / list / compact view |
| `F2` · `Delete` | Rename · move to Trash |
| `Ctrl+M` · `Alt` | Toggle · peek at the menu bar |
| `F1` | All shortcuts |

## 🛠️ For developers

<details>
<summary><b>Build, run and options</b></summary>

```sh
cargo build --release
./target/release/rivet [FOLDER]
```

Options: `--no-decorations`, `--decorations`, `--view icons|list|compact`, `--tree`,
`--hide-menubar`, `--home`, `--version`, `--install-desktop-entry`, `--bench [FOLDER…]`,
`--export-icons DIR`, `--screenshot FILE.png`.

A global `rivet` command that always runs your latest build: `./link.sh` (system-wide, asks for
your password) or `./link.sh --user` (`~/.local/bin`). Preferences → System has the same buttons.

</details>

<details>
<summary><b>Where the colours come from</b></summary>

In order of priority:

1. `~/.config/rivet/colors.css` (`@define-color name #rrggbb;`)
2. whichever is newer: your matugen outputs (learned by lining each template in
   `~/.config/matugen/config.toml` up with its output) or generated GTK colour files
   (`~/.config/gtk-{3,4}.0/gtk.css` / `colors.css`, following `@import`)
3. the defaults of the installed GTK theme

Preferences → *Add Rivet template to matugen* registers a dedicated template if you prefer one.

</details>

<details>
<summary><b>Performance numbers</b></summary>

Release build on Fedora, Wayland, NVMe SSD:

| | |
| --- | --- |
| Start to first drawn frame | ~0.2 s |
| Peak memory (2,000-item folder) | ~80 MB, mostly the GPU driver |
| Drawing a frame | ~0.8–1 ms |
| Reading a 2,092 / 8,365-item folder | ~19 ms / ~52 ms |
| Sorting 8,365 items by name / type | ~5 ms / ~9 ms |
| Indexing icon themes (once) | ~26 ms |
| Binary size (stripped) | ~12 MB |

Reproduce with `rivet --bench [FOLDER…]` and `RIVET_FRAME_STATS=1 rivet --screenshot out.png`.

</details>

<details>
<summary><b>Screenshots, logo and releases</b></summary>

- **Screenshots** come from a throwaway sample home folder: `docs/make-screenshots.sh`
- **Logo** master is `data/rivet-files.svg`; regenerate the PNG set with
  `rivet --export-icons data/icons` and copy the 512 px one to `docs/logo.png`
- **AppImage**: `./appimage.sh` (needs `squashfs-tools`)
- **Release**: bump `version` in `Cargo.toml`, add a `CHANGELOG.md` section, commit,
  then `./release.sh --publish`

</details>

## 📄 License

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
