# Rivet Files

A Nemo-style file manager for Linux, written in Rust with [egui](https://github.com/emilk/egui).
No GTK, no Qt — it talks to the desktop through the freedesktop standards directly.

## Features

- Icon, list and compact views, tabs, back/forward/up, breadcrumb path bar or typed location (Ctrl+L)
- Sidebar with My Computer, Bookmarks (shared `gtk-3.0/bookmarks`), Devices with usage bars and eject, Network, plus a folder-tree mode
- Copy / cut / paste, drag and drop, rename, new folder / document (from Templates), freedesktop Trash with restore
- Filter-as-you-type and recursive search, picture thumbnails, Properties with permissions
- Open with the default app (`mimeapps.list`), Open With…, Open in Terminal
- **System icons**: uses your freedesktop icon theme (follows gtk / KDE / gsettings), SVG and PNG
- **Live desktop colours**: follows matugen, Noctalia, DankMaterialShell or GTK colour files and restyles
  itself within a second when they change — no restart
- **Custom toolbar**: pick the buttons and their order in Preferences (or right-click the toolbar)
- **Optional window decorations**: system title bar, or a built-in one with its own window buttons

## Build

```sh
cargo build --release
./target/release/rivet [FOLDER]
```

Options: `--no-decorations`, `--decorations`, `--view icons|list|compact`, `--tree`, `--hide-menubar`,
`--screenshot FILE.png` (draws ~120 frames, saves a picture, exits).

## Theme integration

Rivet reads colours from, in order of priority:

1. `~/.config/rivet/colors.css` (`@define-color name #rrggbb;`)
2. whichever is newer: your matugen outputs (learned by lining each template in
   `~/.config/matugen/config.toml` up with its output) or generated GTK colour files
   (`~/.config/gtk-{3,4}.0/gtk.css` / `colors.css`, following `@import`)
3. the defaults of the installed GTK theme

Preferences → *Add Rivet template to matugen* registers a dedicated template if you prefer one.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| Ctrl+L | Type a location |
| Ctrl+F or just type | Filter; Enter searches subfolders |
| Ctrl+T / Ctrl+W | New tab / close tab |
| Ctrl+H | Show hidden files |
| Ctrl+1 / 2 / 3 | Icon / list / compact view |
| Ctrl+C / X / V | Copy / cut / paste |
| F2 | Rename |
| Delete / Shift+Delete | Trash / delete permanently |
| Alt+Left / Right / Up | Back / forward / parent |
| Ctrl+Shift+N | New folder |
| Ctrl+D | Bookmark this folder |
| F9 | Show or hide the sidebar |
| Ctrl+M or F10 | Show or hide the menu bar |
| Alt (hold or tap) | Peek at a hidden menu bar |
