# Pinned Zig and cargo-zigbuild, downloaded once into the Rivet build cache.
# Source this from a script that has set ROOT to the repository root.
# The linker is Zig's, so the host glibc (Fedora 44's 2.43, say) is not what
# the release binary records for atan2f / acosf.
: "${ROOT:?ROOT must be set to the repository root}"

ZIG_VERSION=0.15.1
# https://ziglang.org/download/index.json → 0.15.1 x86_64-linux shasum
ZIG_SHA256=c61c5da6edeea14ca51ecd5e4520c6f4189ef5250383db33d01848293bfafe05
ZIGBUILD_VERSION=0.24.1
# GitHub release v0.24.1 asset cargo-zigbuild-x86_64-unknown-linux-gnu.tar.xz
ZIGBUILD_SHA256=32581c8e13f27a807bb244aaaac870522f5b51263c766cf77b1a99771ce9fa5c

# glibc that cargo-zigbuild is asked to link against. Ubuntu 20.04 / Debian 11.
GLIBC_LINK_VERSION=2.31
# Highest symbol version a release ELF may require. Ubuntu 22.04 is 2.35,
# Debian 12 is 2.36, so 2.35 still runs on both.
GLIBC_MAX_VERSION=2.35

CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/rivet-build
ZIG_DIR=$CACHE/zig-$ZIG_VERSION
ZIGBUILD_DIR=$CACHE/cargo-zigbuild-$ZIGBUILD_VERSION

fetch() {
    url=$1
    dest=$2
    sum=$3
    if [ -f "$dest" ] && echo "$sum  $dest" | sha256sum -c - >/dev/null 2>&1; then
        return 0
    fi
    echo "Downloading $url"
    mkdir -p "$(dirname "$dest")"
    curl -fL --proto '=https' --retry 3 -o "$dest.part" "$url"
    echo "$sum  $dest.part" | sha256sum -c -
    mv "$dest.part" "$dest"
}

if [ ! -x "$ZIG_DIR/zig" ]; then
    tarball=$CACHE/zig-$ZIG_VERSION.tar.xz
    fetch "https://ziglang.org/download/$ZIG_VERSION/zig-x86_64-linux-$ZIG_VERSION.tar.xz" "$tarball" "$ZIG_SHA256"
    rm -rf "$ZIG_DIR"
    mkdir -p "$ZIG_DIR"
    tar -xJf "$tarball" -C "$ZIG_DIR" --strip-components=1
fi

if [ ! -x "$ZIGBUILD_DIR/cargo-zigbuild" ]; then
    tarball=$CACHE/cargo-zigbuild-$ZIGBUILD_VERSION.tar.xz
    fetch "https://github.com/rust-cross/cargo-zigbuild/releases/download/v$ZIGBUILD_VERSION/cargo-zigbuild-x86_64-unknown-linux-gnu.tar.xz" \
        "$tarball" "$ZIGBUILD_SHA256"
    rm -rf "$ZIGBUILD_DIR"
    mkdir -p "$ZIGBUILD_DIR"
    tar -xJf "$tarball" -C "$ZIGBUILD_DIR" --strip-components=1
fi

export PATH="$ZIGBUILD_DIR:$ZIG_DIR:$PATH"
export CARGO_ZIGBUILD_ZIG_PATH=$ZIG_DIR/zig
