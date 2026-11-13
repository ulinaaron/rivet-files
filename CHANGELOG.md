# Changelog

## 0.1.2

- bump.sh: one command to set the next version
- Delete dialog: roomier padding
- Clearer delete confirmation; the Delete key asks
- Highlighted sidebar rows recolor the same icon
- Appearance: grouped spacing, US spelling, drop auto-update hint
- Toolbar settings: trash-can button instead of 'Remove'
- Title bar setting: only in Preferences, clearer wording
- Toolbar settings: up / down chevron buttons instead of text
- Sidebar icon size: Off, Small, Medium, Large
- Highlighted rows show the icon as a solid recolored silhouette
- Active view / sidebar-mode buttons use the selected-file look; status bar padding
- Grid view: one tile style for hover and selection; names stay visible
- Faster big folders, zero idle CPU, 100k-file benchmark, mouse back/forward
- README: cleaner layout with concise emoji feature list
- Pop-up windows: close button only (no roll-up / maximize)
- Describe Rivet as a classic Linux file manager like Nemo, Thunar and Caja
- CI workflow and README badges
- README: logo above the title, not floated right
- Logo assets in the codebase
- Separate Keyboard Shortcuts and About; new purple folder + rivet icon
- Polish: one radius scale, even location pills, flat status bar, readable icons on highlights

## 0.1.1

- **AppImage**: Rivet now also ships as a single-file AppImage — download, `chmod +x`, run.
  "Add Rivet to the app launcher" from inside it points your menus at the AppImage.
- New app icon (window, launcher entry and AppImage).
- The archive's installer also installs the icon.

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
- Uses your system icon theme, or Rivet's own SVG icons recolored from your palette
  (primary / secondary / tertiary)
- Follows matugen, Noctalia and GTK colors live — no restart
- Surface elevations instead of borders, roomy toolbar and sidebar
- UI font and text size options
- Optional window decorations (system title bar or a built-in one)
- Customizable toolbar; hideable menu bar (Ctrl+M, Alt to peek, hamburger button)
- Preferences and Properties as movable, resizable windows inside the app

### System
- Desktop entry, "make default file manager", and a global `rivet` command
- `rivet --bench` for timings; ~0.2 s to first frame, ~1 ms per frame
