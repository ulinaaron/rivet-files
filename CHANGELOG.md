# Changelog

## 0.1.0 — first release

The first public version of Rivet Files: a Nemo-style file manager for Linux, written in Rust
with egui. No GTK or Qt.

### Browsing
- Icon, list and compact views; tabs; back / forward / up; breadcrumb location bar
  (click it to type a path) with Ctrl+L
- Sidebar with My Computer, Bookmarks (shared with other file managers), Devices with
  usage bars and eject, Network, and a folder-tree mode
- Filter-as-you-type and recursive search, picture thumbnails, hidden files toggle,
  sorting by name / size / type / date

### Files
- Copy, cut, paste, drag and drop, rename, new folder / document (from Templates)
- Trash following the desktop standard, with restore and empty
- Open with the default app, Open With…, Open in Terminal, Properties with permissions

### Looks
- Uses your system icon theme, or Rivet's own SVG icons recoloured from your palette
  (primary / secondary / tertiary)
- Follows matugen, Noctalia and GTK colours live — no restart
- Surface elevations instead of borders, roomy toolbar and sidebar
- UI font and text size options
- Optional window decorations (system title bar or a built-in one)
- Customisable toolbar; hideable menu bar (Ctrl+M, Alt to peek, hamburger button)
- Preferences and Properties as movable, resizable windows inside the app

### System
- Ships as an AppImage (single file) and as a tar.gz archive with an installer
- Own app icon
- Desktop entry, "make default file manager", and a global `rivet` command
- `rivet --bench` for timings; ~0.2 s to first frame, ~1 ms per frame
