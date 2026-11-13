#!/bin/sh
# Build a single-file AppImage: dist/Rivet_Files-VERSION-ARCH.AppImage
#
# Needs mksquashfs (squashfs-tools). The AppImage "type 2 runtime" (the small
# program at the front of every AppImage) is downloaded once from the AppImage
# project and cached in ~/.cache/rivet-build/.
set -e
cd "$(dirname "$0")/../.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)
OUT="dist/Rivet_Files-$VERSION-$ARCH.AppImage"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/rivet-build"
RUNTIME="$CACHE/runtime-$ARCH"

command -v mksquashfs >/dev/null || { echo "Please install squashfs-tools (mksquashfs)." >&2; exit 1; }

if [ ! -s "$RUNTIME" ]; then
    mkdir -p "$CACHE"
    echo "Downloading the AppImage runtime for $ARCH…"
    curl -fL --proto '=https' -o "$RUNTIME.part" \
        "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$ARCH"
    # Sanity check: it must be an ELF program.
    head -c 4 "$RUNTIME.part" | od -An -c | grep -q 'E   L   F' || { echo "Download is not an ELF file." >&2; rm -f "$RUNTIME.part"; exit 1; }
    mv "$RUNTIME.part" "$RUNTIME"
fi

# Linked for glibc 2.31, not whatever this machine's libm exports.
./utilities/lib/portable-build.sh >/dev/null
RIVET_BIN=$(./utilities/lib/portable-bin.sh)
APPDIR="dist/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons"
strip -o "$APPDIR/usr/bin/rivet" "$RIVET_BIN"
# winit dlopens these; ldd on rivet does not see them. Ubuntu 22.04 builds,
# so the bundle itself does not need glibc 2.43.
./utilities/lib/bundle-runtime-libs.sh "$APPDIR/usr/lib"
cp data/rivet-files.desktop "$APPDIR/rivet-files.desktop"
cp data/rivet-files.desktop "$APPDIR/usr/share/applications/"
cp data/rivet-files.svg "$APPDIR/rivet-files.svg"
cp -r data/icons/hicolor "$APPDIR/usr/share/icons/"
cp data/icons/hicolor/256x256/apps/rivet-files.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
# Bundled libxkbcommon, libxkbcommon-x11 and libxcb-xkb. The system provides
# libc, libX11, Wayland and OpenGL so they match the running desktop.
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$HERE/usr/bin/rivet" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# The file we ship, plus every bundled library, must stay under the ceiling.
ROOT=$(pwd)
# shellcheck disable=SC1091
. ./utilities/lib/ensure-zigbuild.sh
set -- "$APPDIR/usr/bin/rivet"
for lib in "$APPDIR"/usr/lib/lib*.so.*; do
    [ -f "$lib" ] || continue
    set -- "$@" "$lib"
done
./utilities/lib/check-glibc.sh --max "$GLIBC_MAX_VERSION" --record dist/glibc-highest "$@"

rm -f "$OUT" dist/AppDir.squashfs
mksquashfs "$APPDIR" dist/AppDir.squashfs -root-owned -noappend -comp zstd -quiet
cat "$RUNTIME" dist/AppDir.squashfs > "$OUT"
chmod +x "$OUT"
rm -rf "$APPDIR" dist/AppDir.squashfs
(cd dist && sha256sum "$(basename "$OUT")" > "$(basename "$OUT").sha256")
echo "Built $OUT"
