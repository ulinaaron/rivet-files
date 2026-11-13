#!/bin/sh
# Make the `rivet` command open the build in this folder (rebuilds are picked up automatically).
#   utilities/rivet-dev link          -> system-wide command in /usr/bin  (asks for your password)
#   utilities/rivet-dev link --user   -> ~/.local/bin/rivet                (no password)
#   utilities/rivet-dev link --remove [--user]
set -e
cd "$(dirname "$0")/../.."
EXE="$(pwd)/target/debug/rivet"
[ -x "$EXE" ] || cargo build
DEST="/usr/bin/rivet"
SUDO=sudo
for a in "$@"; do
    case "$a" in
        --user) DEST="${XDG_BIN_HOME:-$HOME/.local/bin}/rivet"; SUDO=""; mkdir -p "$(dirname "$DEST")" ;;
    esac
done
if [ -e "$DEST" ] && [ ! -L "$DEST" ]; then
    echo "$DEST is a real file (probably from a package); leaving it alone." >&2
    exit 1
fi
case " $* " in
    *" --remove "*) $SUDO rm -f "$DEST"; echo "Removed $DEST" ;;
    *) $SUDO ln -sfn "$EXE" "$DEST"; echo "$DEST -> $EXE" ;;
esac
