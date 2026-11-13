#!/bin/sh
# Copy pinned Ubuntu 22.04 keyboard libraries into DEST.
#
# winit dlopens these; they are not NEEDED entries on the rivet binary, so a
# bundler that only follows ldd misses them. The packages are jammy's, and the
# newest GLIBC symbol among them is 2.33 (libxkbcommon 1.4), which still runs
# on Ubuntu 22.04 (2.35) and Debian 12 (2.36).
set -eu

if [ $# -ne 1 ]; then
    echo "usage: $0 DEST_LIB_DIR" >&2
    exit 2
fi
dest=$1
mkdir -p "$dest"

cache=${XDG_CACHE_HOME:-$HOME/.cache}/rivet-build/debs
mkdir -p "$cache"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

# name, sha256, url — Ubuntu 22.04 (jammy) packages.
while read -r name sum url; do
    [ -n "$name" ] || continue
    deb=$cache/$name
    if ! echo "$sum  $deb" | sha256sum -c - >/dev/null 2>&1; then
        echo "Downloading $name"
        curl -fL --proto '=https' --retry 3 -o "$deb.part" "$url"
        echo "$sum  $deb.part" | sha256sum -c -
        mv "$deb.part" "$deb"
    fi
    pkgdir=$stage/$name
    mkdir -p "$pkgdir"
    if command -v dpkg-deb >/dev/null 2>&1; then
        dpkg-deb -x "$deb" "$pkgdir"
    else
        unpacked=$stage/ar-$name
        mkdir -p "$unpacked"
        (cd "$unpacked" && ar x "$deb")
        data=$(echo "$unpacked"/data.tar.*)
        tar -C "$pkgdir" -xf "$data"
    fi
    found=0
    for dir in "$pkgdir"/usr/lib/x86_64-linux-gnu "$pkgdir"/lib/x86_64-linux-gnu; do
        [ -d "$dir" ] || continue
        # cp -a keeps the soname symlinks next to the real objects.
        cp -a "$dir"/. "$dest"/
        found=1
    done
    [ "$found" -eq 1 ] || { echo "no libraries extracted from $name" >&2; exit 1; }
done <<'EOF'
libxkbcommon0_1.4.0-1_amd64.deb a0470edb237d52b71ad41f755c043164abed01423585ed5ea19525b3baaf84f7 https://archive.ubuntu.com/ubuntu/pool/main/libx/libxkbcommon/libxkbcommon0_1.4.0-1_amd64.deb
libxkbcommon-x11-0_1.4.0-1_amd64.deb e29c40fab6e0f055218aee4c7e60cd61ea82283229d7ac7ac02e226e92a5b0d5 https://archive.ubuntu.com/ubuntu/pool/main/libx/libxkbcommon/libxkbcommon-x11-0_1.4.0-1_amd64.deb
libxcb-xkb1_1.14-3ubuntu3_amd64.deb 4cf69af3ff6a60b63f9f6ea6e7d27471114c6bcbcf3f981f7b88f361029e5eb8 https://archive.ubuntu.com/ubuntu/pool/main/libx/libxcb/libxcb-xkb1_1.14-3ubuntu3_amd64.deb
EOF

for soname in libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb-xkb.so.1; do
    if [ ! -e "$dest/$soname" ]; then
        echo "bundle is missing $soname" >&2
        exit 1
    fi
done
echo "Bundled keyboard libraries in $dest"
