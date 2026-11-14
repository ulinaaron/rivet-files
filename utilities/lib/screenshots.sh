#!/bin/sh
# Regenerate the README screenshots from a throwaway sample home folder, so they
# never show anyone's real files, bookmarks, drives or desktop colours.
set -e
cd "$(dirname "$0")/../.."
cargo build
OUT="docs/screenshots"
mkdir -p "$OUT"

T="$(mktemp -d /tmp/rivet-docs-XXXXXX)"
H="$T/you"
for d in Desktop Documents Downloads Music Pictures Public Templates Videos Projects; do mkdir -p "$H/$d"; done
for f in notes.txt todo.md install.sh backup.tar.gz; do : > "$H/$f"; done
for f in report.pdf budget.ods slides.odp letter.odt meeting-notes.txt; do : > "$H/Documents/$f"; done
for f in song.mp3 podcast.ogg; do : > "$H/Music/$f"; done
for f in holiday.mp4; do : > "$H/Videos/$f"; done
for f in main.rs index.html style.css data.json; do : > "$H/Projects/$f"; done
# A real source file so the Peek panel has something to highlight.
cat > "$H/Projects/word_count.rs" <<'EOF'
use std::collections::HashMap;

/// Count how often each word appears.
fn count_words(text: &str) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1; // tally
    }
    counts
}

fn main() {
    let text = "the quick brown fox jumps over the lazy dog";
    let counts = count_words(text);
    println!("{} unique words, 'the' appears {} times", counts.len(), counts["the"]);
}
EOF

# A few colourful sample pictures so thumbnails show up.
python3 -I - "$H/Pictures" "$H" <<'EOF'
import sys
from PIL import Image
out, home = sys.argv[1], sys.argv[2]
def grad(name, a, b, w=640, h=400):
    im = Image.new("RGB", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            t = (x / w + y / h) / 2
            px[x, y] = tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))
    im.save(name)
grad(f"{out}/sunset.png", (255, 140, 90), (120, 40, 160))
grad(f"{out}/ocean.png", (20, 120, 200), (10, 30, 60))
grad(f"{out}/forest.png", (40, 160, 90), (10, 50, 30))
grad(f"{home}/wallpaper.png", (90, 150, 255), (30, 20, 80))
EOF

export HOME="$H"
export XDG_CONFIG_HOME="$T/config"
export XDG_DATA_HOME="$H/.local/share"
export XDG_CACHE_HOME="$T/cache"
mkdir -p "$XDG_CONFIG_HOME/rivet"
# Neutral default apps for the Peek "Open" button (whatever is installed is used if these aren't).
cat > "$XDG_CONFIG_HOME/mimeapps.list" <<'EOF'
[Default Applications]
image/png=org.gnome.Loupe.desktop
EOF
# A fixed, matugen-style blue scheme for the pictures.
cat > "$XDG_CONFIG_HOME/rivet/colors.css" <<'EOF'
@define-color accent_bg_color #8ecdff;
@define-color accent_fg_color #00344f;
@define-color window_bg_color #191c20;
@define-color window_fg_color #e1e2e8;
@define-color headerbar_bg_color #1d2024;
@define-color view_bg_color #111418;
@define-color popover_bg_color #272a2f;
@define-color secondary #b7c9d9;
@define-color tertiary #d0bfe7;
EOF

BIN=./target/debug/rivet
RIVET_ICON_THEME=Papirus "$BIN" "$H" --screenshot "$OUT/icons-wide.png"
RIVET_ICON_THEME=Papirus "$BIN" "$H/Documents" --view list --screenshot "$OUT/list-wide.png"
RIVET_ICONS=builtin RIVET_ICON_COLOR=tertiary "$BIN" "$H" --screenshot "$OUT/rivet-icons-wide.png"
RIVET_ICON_THEME=Papirus RIVET_OPEN_PREFS=1 "$BIN" "$H" --screenshot "$OUT/preferences-wide.png"
# The Peek panel: highlighted code, and a picture (items are sorted by name).
RIVET_ICON_THEME=Papirus RIVET_PEEK=1 RIVET_SELECT=4 "$BIN" "$H/Projects" --screenshot "$OUT/peek-code-wide.png"
RIVET_ICON_THEME=Papirus RIVET_PEEK=1 RIVET_SELECT=2 "$BIN" "$H/Pictures" --screenshot "$OUT/peek-picture-wide.png"
# As minimal or as bloated as you like.
# Minimal: no menu bar, no status bar, no Peek; Places in the sidebar; a short toolbar.
RIVET_ICON_THEME=Papirus RIVET_NO_STATUSBAR=1 RIVET_TOOLBAR="menu,back,forward,path,view-icons,view-list" \
    "$BIN" "$H" --hide-menubar --screenshot "$OUT/minimal-wide.png"
# Bloated: menu bar, a full toolbar, tabs, status bar and Peek on a picture.
# (The extra tab opens last and is the active one, so the picture is selected there.)
RIVET_ICON_THEME=Papirus RIVET_PEEK=1 RIVET_SELECT=2 RIVET_EXTRA_TAB="$H/Pictures" \
    RIVET_TOOLBAR="back,forward,up,reload,home,new-folder,new-tab,path,search,view-icons,view-list,view-compact,show-hidden,terminal,trash,peek" \
    "$BIN" "$H/Documents" --screenshot "$OUT/bloated-wide.png"
# Tools: the Filters panel (a "this folder" filter showing folders and pictures) and Bulk Rename.
cat > "$T/filters" <<'EOF'
[folder $START]
enabled=true
types=folders,images
name_mode=contains
name=
date=any
EOF
RIVET_ICON_THEME=Papirus RIVET_FILTERS="$T/filters" RIVET_FILTERS_OPEN=1 RIVET_TOOLBAR="back,forward,up,reload,home,computer,edit-location,path,search,filters,view-icons,view-list,view-compact,menu" \
    "$BIN" "$H" --screenshot "$OUT/filters-wide.png"
RIVET_ICON_THEME=Papirus RIVET_SELECT=0,1,2 RIVET_DIALOG=bulkrename RIVET_CLICKS="type:holiday_@20" \
    "$BIN" "$H/Pictures" --screenshot "$OUT/bulk-rename-wide.png"
# Zen mode with the toolbar and sidebar slid in, and Quick Open (Ctrl+P).
RIVET_ICON_THEME=Papirus RIVET_ZEN=1 RIVET_ZEN_SHOW=toolbar,sidebar \
    "$BIN" "$H/Documents" --screenshot "$OUT/zen-wide.png"
RIVET_ICON_THEME=Papirus RIVET_ZEN=1 RIVET_SPOTLIGHT=p \
    "$BIN" "$H" --screenshot "$OUT/quick-open-wide.png"

# The same window recoloured by different matugen-style schemes (Rivet's own icons).
# palette NAME ACCENT ACCENT_FG WINDOW_BG WINDOW_FG HEADERBAR VIEW POPOVER SECONDARY TERTIARY [VIEW_OPTS]
palette() {
    name=$1; shift
    cat > "$XDG_CONFIG_HOME/rivet/colors.css" <<EOF
@define-color accent_bg_color $1;
@define-color accent_fg_color $2;
@define-color window_bg_color $3;
@define-color window_fg_color $4;
@define-color headerbar_bg_color $5;
@define-color view_bg_color $6;
@define-color popover_bg_color $7;
@define-color secondary $8;
@define-color tertiary $9;
EOF
    RIVET_ICONS=builtin "$BIN" "$H" --screenshot "$OUT/theme-$name-wide.png"
}
palette ember  '#ffb4a1' '#561f10' '#201a19' '#f1dfdb' '#271d1b' '#1a1110' '#352826' '#e7bdb3' '#d8c58d'
palette moss   '#a5d395' '#11380b' '#1a1c18' '#e2e3dc' '#1e201c' '#12140f' '#282b25' '#bccbb0' '#a0cfd0'
palette violet '#d4bbff' '#3b2163' '#1d1b20' '#e6e1e9' '#211f24' '#141218' '#2b2930' '#cdc2db' '#f0b7c5'
palette rose   '#8f4a5b' '#ffffff' '#f6e6e9' '#22191b' '#f1dde1' '#fff8f8' '#ffffff' '#75565c' '#7c5635'

# Keep the repository light: scale to 1100 px wide.
python3 -I - "$OUT" <<'EOF'
import sys, glob
from PIL import Image
for p in glob.glob(sys.argv[1] + "/*.png"):
    im = Image.open(p).convert("RGB")
    if im.width > 1100:
        im = im.resize((1100, round(im.height * 1100 / im.width)), Image.LANCZOS)
    im.save(p, optimize=True)
EOF

rm -rf "$T"
echo "Screenshots written to $OUT"
