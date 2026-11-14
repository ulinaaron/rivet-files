# Changelog

## Unreleased

### New
- **Terminal mode** (F7, Preferences → Appearance → Mode, View → Terminal Mode, or Quick Actions): a
  GUI that feels like a TUI. Everything is monospace, icons are Nerd Font glyphs colored by kind
  (drawn from any installed Nerd Font, your monospace font first), corners are square, shadows are
  gone and the sidebar, files and Peek sit in plain boxes with their titles in the border. Picture
  thumbnails give way to glyphs in the file list (Peek and Preview still show pictures).
- **Monospace font** (Preferences → Appearance → Typography): used for text and code previews,
  and for everything in terminal mode. Type to find a font, like the interface font.
- **Zen mode** (F12, or View → Zen Mode): the files fill the window, edge to edge. Move the pointer
  to the toolbar's edge, the sidebar's edge or Peek's edge and that panel slides in over the files;
  move away and it slides back out. Peek takes the other side if it shares one with the sidebar.
  The menus live behind the toolbar's menu button while in Zen mode.
- **Quick Open** (Ctrl+P, or View → Quick Open): a jump-anywhere bar. Type a few letters of
  any folder — recent ones, places, bookmarks, drives, folders here and under Home — and press
  Enter. Start with `/` or `~` to walk a path; Tab fills in the highlighted folder.
- **Quick Actions** (Ctrl+Shift+P, or View → Quick Actions): every menu command and view option
  in one searchable list — views, panels, sorting, layout, theme, going places, file commands.
  On / off options show their state and keep the list open, so you can flip several in a row.

## 0.4.0

### New
- **Filters**: show a folder with only certain items — by type (folders, pictures, videos, audio,
  documents, archives, code & text, other), by name (contains / excludes / includes patterns like
  `*.jpg, IMG_*`) and by date modified. A filter is Global, for This folder, or for This folder &
  below; only one applies, and a folder's own filter comes first, then one set on a folder above
  it, then Global. A strip above the files says when one is on and how many items it hides. The
  Filters panel drops down over the files (strip funnel, a Filters toolbar button, View → Filters…
  or Ctrl+Shift+F), so the sidebar stays free for getting around. Saved in `~/.config/rivet/filters`.
- **Bulk Rename**: select several items and press F2 (or right-click → Bulk Rename…). Insert /
  overwrite text, date / time, numbering, remove characters, search & replace, upper / lower /
  title / sentence case — on the name, the extension or both, with a live preview that flags
  clashes before anything changes. One Ctrl+Z undoes the whole batch.
- **Convert & Resize** (uses ffmpeg): right-click → Convert To… for pictures (JPEG, PNG, WebP, GIF,
  BMP, TIFF), videos (MP4, WebM, MKV, GIF, MP3) and audio (MP3, Ogg, FLAC, WAV, M4A), one file or
  many, with a quality setting; keep the originals or replace them (originals go to the Trash).
  Right-click → Resize… for pictures: by percentage or to fit a box, as new copies or overwriting.
  Runs in the background.
- **Modules** page in Preferences: Bulk Rename and Convert & Resize each have an on/off switch.
  Convert & Resize switches itself off while ffmpeg isn't installed, and notices when it is.

### Improved
- README: features grouped, a Tools gallery (Filters, Bulk Rename).

### Fixed
- Arrows in the Bulk Rename and Resize previews are drawn, so they show with every font.

### Developer
- All developer scripts live in `utilities/` behind one command, `utilities/rivet-dev <command>`:
  `bump`, `release [--publish]`, `push`, `appimage`, `portable`, `install`, `link`, `screenshots`,
  `bench`… (`help` lists them). `push` and `release --publish` take in GitHub's commits first.

## 0.3.0

### New
- **Toolbar and status bar positions** (Preferences → Appearance → Window): the toolbar at the top
  or bottom, the status bar off, top or bottom. On the same edge they stack.
- **Preview**: a full-window light box for pictures, videos (muted playback), music covers and
  text. Right-click → Preview or press Space; ← / → (or the side arrows) step through the folder, Return opens the file,
  Esc, Space or a click outside closes it. Its bottom split button opens with the default app
  (named, with its icon) or picks another one.
- **Preferences** has a sidebar of pages: Appearance (with Window), Views, Behavior, Menu (with the
  toolbar), System and About. Appearance is split into roomy groups: desktop colors, style and accent,
  icons, typography, window. Help → About opens the About page.
- **Theme-tinted action colors**: info, danger, warning and success are 70% their usual hue and
  30% your theme accent. Delete Permanently, close-button hovers and errors use danger; Rename uses success.
- **Folder suggestions** while typing a path: a fuzzy-matched list of sub-folders under the
  location bar; Tab completes the highlighted one, Enter completes it and goes there, ↑/↓ pick.
- **Location bar**: drag across it to select the path as text; right-click it (or any folder in it)
  for Copy Location, Open in New Tab and Edit Location.

### Improved
- **Keyboard focus**: Tab now shows a clear accent ring on whatever has focus (menus, toolbar,
  sidebar, status bar, file list). The file list is a single Tab stop — arrows, Enter and Space
  work on the files from there — instead of every file being one.
- **Keyboard**: Ctrl+, opens Preferences with the keyboard; ↑/↓ move between its pages, Home/End
  jump, Esc closes, and the file view no longer reacts to those keys while Preferences has focus.
- **Light mode contrast**: file icons and thumbnails get a soft shadow on light backgrounds, Rivet's
  own page icons are tinted with a thin edge, and secondary text, panels and controls are stronger.
- **Properties** redesigned: big icon and name field, a Basic / Permissions switch, and roomy
  General, Dates, Ownership and Access cards.
- **F8** shows or hides the status bar.
- Esc in a Preferences text field or list only leaves it.
- README: minimal vs bloated screenshots.
- **Font picker**: type in the font list to narrow it to the closest matches; Enter picks the best one.
- Clearer System page (name · status · action rows) and no filler text in Preferences.
- Roomier Rename window.
- The status bar no longer repeats the free disk space (the sidebar's drive bars show it).
- Peek's split *Open with* button: full-height divider and an apps icon for choosing another app.
- README: Rivet's goals and a short roadmap.
- Code tidy-up: no compiler warnings, unused code removed.

### Fixed
- The `rivet` terminal command (Preferences → System → Link) could point at another AppImage app
  when Rivet was started from one; Rivet now only treats itself as an AppImage when it runs from it.

### Removed
- The niri-only code that moved check / screenshot windows to another workspace.

## 0.2.0

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
- Roomier right-click menus.
- Peek layout: small action buttons first, a full-width split *Open with …* button below (files only).
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
