<img src="data/rivet-files.svg" width="96" alt="Rivet Files icon">

# Rivet Files

A Nemo-style file manager for Linux, written in Rust with [egui](https://github.com/emilk/egui).
No GTK, no Qt — it talks to the desktop through the freedesktop standards directly.

![Rivet Files showing a home folder with the Papirus icon theme](docs/screenshots/icons-wide.png)

<table>
  <tr>
    <td><img src="docs/screenshots/list-wide.png" alt="List view of a Documents folder"></td>
    <td><img src="docs/screenshots/rivet-icons-wide.png" alt="Rivet's own icon set, recoloured from the desktop palette"></td>
    <td><img src="docs/screenshots/preferences-wide.png" alt="Preferences as a window inside the app"></td>
  </tr>
  <tr>
    <td align="center">List view</td>
    <td align="center">Built-in icons, coloured from your palette</td>
    <td align="center">Preferences, as an in-app window</td>
  </tr>
</table>

### Follows your colours

The same window under four matugen-style schemes, with Rivet's built-in icons recoloured to match.
Change your wallpaper / scheme and Rivet restyles itself within a second — no restart.

<table>
  <tr>
    <td><img src="docs/screenshots/theme-ember-wide.png" alt="Ember: warm dark scheme"></td>
    <td><img src="docs/screenshots/theme-moss-wide.png" alt="Moss: green dark scheme"></td>
  </tr>
  <tr>
    <td align="center">Ember</td>
    <td align="center">Moss</td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/theme-violet-wide.png" alt="Violet: purple dark scheme"></td>
    <td><img src="docs/screenshots/theme-rose-wide.png" alt="Rose: light scheme"></td>
  </tr>
  <tr>
    <td align="center">Violet</td>
    <td align="center">Rose (light)</td>
  </tr>
</table>

<sub>Screenshots come from a throwaway sample home folder — run <code>docs/make-screenshots.sh</code> to regenerate them.</sub>

## Features

- Icon, list and compact views, tabs, back/forward/up, breadcrumb path bar or typed location (Ctrl+L)
- Sidebar with My Computer, Bookmarks (shared `gtk-3.0/bookmarks`), Devices with usage bars and eject, Network, plus a folder-tree mode
- Copy / cut / paste, drag and drop, rename, new folder / document (from Templates), freedesktop Trash with restore
- Filter-as-you-type and recursive search, picture thumbnails, Properties with permissions
- Open with the default app (`mimeapps.list`), Open With…, Open in Terminal
- **System icons**: uses your freedesktop icon theme (follows gtk / KDE / gsettings), SVG and PNG
- **Live desktop colours**: follows matugen, Noctalia, DankMaterialShell or GTK colour files and restyles
  itself within a second when they change — no restart
- **Optional built-in icon set** (Preferences → Appearance → Icon theme → *Rivet*): all-SVG folder,
  file, place and toolbar icons recoloured live from your matugen / desktop palette, used app-wide
- **Custom toolbar**: pick the buttons and their order in Preferences (or right-click the toolbar)
- **Optional window decorations**: system title bar, or a built-in one with its own window buttons

## Build

```sh
cargo build --release
./target/release/rivet [FOLDER]
```

### Download

From the [Releases page](https://github.com/ulinaaron/rivet-files/releases):

- **AppImage** — `Rivet_Files-…-x86_64.AppImage`: one file, `chmod +x` it and run.
  Preferences → System → *Add Rivet to the app launcher* adds it to your menus.
- **Archive** — `rivet-files-…-linux.tar.gz`: unpack it and run `./install.sh`.

Build the AppImage yourself with `./appimage.sh` (needs `squashfs-tools`).

### Install (adds it to your app launcher)

```sh
./install.sh            # binary to ~/.local/bin, desktop entry to ~/.local/share/applications
./install.sh --default  # …and make it the default file manager
```

Or from inside the app: Preferences → System → *Add Rivet to the app launcher*
(and *Make Rivet the default file manager*). `rivet --install-desktop-entry` does the same from a terminal.
The entry file lives in `data/rivet-files.desktop`.

### A global `rivet` command that follows your builds

```sh
./link.sh          # /usr/bin/rivet -> target/debug/rivet  (asks for your password)
./link.sh --user   # ~/.local/bin/rivet instead (no password)
./link.sh --remove [--user]
```

It is a symlink, so every rebuild is picked up. Preferences → System has the same two buttons
(the system one asks for your password through the desktop's own dialog). Real packaged files
are never overwritten.

Options: `--no-decorations`, `--decorations`, `--view icons|list|compact`, `--tree`, `--hide-menubar`, `--home`,
`--screenshot FILE.png` (draws ~120 frames, saves a picture, exits).

## Theme integration

Rivet reads colours from, in order of priority:

1. `~/.config/rivet/colors.css` (`@define-color name #rrggbb;`)
2. whichever is newer: your matugen outputs (learned by lining each template in
   `~/.config/matugen/config.toml` up with its output) or generated GTK colour files
   (`~/.config/gtk-{3,4}.0/gtk.css` / `colors.css`, following `@import`)
3. the defaults of the installed GTK theme

Preferences → *Add Rivet template to matugen* registers a dedicated template if you prefer one.

## Performance

Measured on a release build (Fedora, Wayland, Paper icon theme, NVMe SSD):

| | |
| --- | --- |
| Start to first drawn frame | ~0.2 s |
| Peak memory (window open, 2,000-item folder) | ~80 MB, most of it the GPU driver |
| Drawing a frame afterwards | ~0.8–1 ms (95% under 1.6 ms) |
| Reading + inspecting a 2,092-item folder | ~19 ms (≈9 µs per item) |
| Reading an 8,365-item folder | ~52 ms |
| Sorting 8,365 items by name / by type | ~5 ms / ~9 ms |
| Indexing the icon themes (once per run) | ~26 ms |
| Finding icons for 2,092 items | under 1 ms |
| Binary size (stripped) | ~12 MB |

Reproduce with `rivet --bench [FOLDER…]` (folder timings, no window) and
`RIVET_FRAME_STATS=1 rivet --screenshot out.png` (startup and frame times).

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

## Logo

The logo is `data/rivet-files.svg` (the master). PNGs at every standard size live in
`data/icons/hicolor/…/apps/rivet-files.png`, plus `docs/logo.png` (512 px).
After editing the SVG, regenerate them with:

```sh
cargo build && ./target/debug/rivet --export-icons data/icons
cp data/icons/hicolor/512x512/apps/rivet-files.png docs/logo.png
```

## Making a release

Bump `version` in `Cargo.toml`, add a section to `CHANGELOG.md`, commit, then run
`./release.sh` (builds `dist/rivet-files-VERSION-ARCH-linux.tar.gz` and tags it) or
`./release.sh --publish` (also creates the GitHub release).

## License

Rivet Files is available under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option. Unless you say otherwise, any contribution you submit is offered under the same terms.
All of Rivet's library dependencies use compatible permissive licences (MIT, Apache-2.0, BSD, Zlib and similar).
