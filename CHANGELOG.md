# Changelog

## 0.2.0 — unreleased

### New
- **Peek panel** (F11, View menu, status bar): a big preview of the selection with all its details.
  Pictures, videos that play right in the panel (muted, with seek), music covers, and text files
  with syntax highlighting in your theme's colors. Drag the grip to resize the preview.
  A split "Open with …" button (the arrow picks another app), plus Copy location, Rename, Trash and Properties.
- **Network** that works: finds servers on your network, lists SMB shares, connects (with a login
  window or as guest), disconnects, and *Connect to Server…* for smb://, sftp://, ftp://, dav:// addresses.
- **Compress and Extract Here**: zip, tar.gz, tar.xz, tar.zst and 7z, in the background, with undo.
- **Undo and redo** (Ctrl+Z / Ctrl+Shift+Z) for copies, moves, renames, Trash and new items.
- **Sidebar and Peek on either side** (Preferences → Window); the status bar buttons follow them.

### Improved
- Open With window redesigned; it always lists the current default app.
- Tidier menus with consistent shortcut labels; roomier hamburger menu; wider menu bar hover.
- Title bar setting reads *System* / *App*.
- Check and screenshot windows stay out of the way on niri.
- Release builds run on older glibc; Flatpak and AUR packaging.

### Removed
- Alt no longer peeks at a hidden menu bar (Ctrl+M shows it).

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
