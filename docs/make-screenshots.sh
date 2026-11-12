#!/bin/sh
# Regenerate the README screenshots from a throwaway sample home folder, so they
# never show anyone's real files, bookmarks, drives or desktop colours.
set -e
cd "$(dirname "$0")/.."
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
