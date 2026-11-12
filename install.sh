#!/bin/sh
# Build Rivet Files and install it for the current user (no root needed):
#   binary        -> ~/.local/bin/rivet
#   desktop entry -> ~/.local/share/applications/rivet-files.desktop
# Pass --default to also make it the default file manager.
set -e
cd "$(dirname "$0")"
cargo build --release
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
install -Dm755 target/release/rivet "$BIN/rivet"
mkdir -p "$APPS"
sed "s|^Exec=rivet|Exec=\"$BIN/rivet\"|" data/rivet-files.desktop > "$APPS/rivet-files.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
if [ "$1" = "--default" ]; then
    xdg-mime default rivet-files.desktop inode/directory
fi
echo "Installed Rivet Files. It is now in your app launcher."
