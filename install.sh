#!/bin/sh
# Install Rivet Files for the current user (no root needed). Uses the ready-made
# `rivet` program next to this script (release download), or builds it from source:
#   binary        -> ~/.local/bin/rivet
#   desktop entry -> ~/.local/share/applications/rivet-files.desktop
# Pass --default to also make it the default file manager.
set -e
cd "$(dirname "$0")"
if [ -x ./rivet ] && [ ! -d ./src ]; then
    SRC=./rivet                                  # release download
else
    cargo build --release && SRC=target/release/rivet   # source checkout
fi
DESKTOP=data/rivet-files.desktop
[ -f "$DESKTOP" ] || DESKTOP=rivet-files.desktop
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
install -Dm755 "$SRC" "$BIN/rivet"
mkdir -p "$APPS"
# Logo at every standard size (data/icons in a checkout, icons/ in a release download).
ICONS=data/icons
[ -d "$ICONS" ] || ICONS=icons
ICON_HOME="${XDG_DATA_HOME:-$HOME/.local/share}/icons"
(cd "$ICONS" && find hicolor -type f) | while read -r f; do install -Dm644 "$ICONS/$f" "$ICON_HOME/$f"; done
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$ICON_HOME/hicolor" 2>/dev/null || true
sed "s|^Exec=rivet|Exec=\"$BIN/rivet\"|" "$DESKTOP" > "$APPS/rivet-files.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
if [ "$1" = "--default" ]; then
    xdg-mime default rivet-files.desktop inode/directory
fi
echo "Installed Rivet Files. It is now in your app launcher."
