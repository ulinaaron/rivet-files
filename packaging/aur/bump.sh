#!/bin/sh
# Point the AUR PKGBUILDs at a published release and refresh .SRCINFO.
#
#   ./packaging/aur/bump.sh 0.1.3
#
# Downloads the binary tarball and the git tag archive, then rewrites pkgver
# and the checksums. Publishing to the AUR itself is still a manual push.
set -eu
if [ $# -ne 1 ]; then
  echo "usage: $0 VERSION" >&2
  exit 2
fi
ver=$1
case "$ver" in
  v*) ver=${ver#v} ;;
esac
root=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

bin_url="https://github.com/ulinaaron/rivet-files/releases/download/v${ver}/rivet-files-${ver}-x86_64-linux.tar.gz"
src_url="https://github.com/ulinaaron/rivet-files/archive/refs/tags/v${ver}.tar.gz"
curl -fL --proto '=https' -o "$tmp/bin.tar.gz" "$bin_url"
curl -fL --proto '=https' -o "$tmp/src.tar.gz" "$src_url"
bin_sha=$(sha256sum "$tmp/bin.tar.gz" | awk '{print $1}')
src_sha=$(sha256sum "$tmp/src.tar.gz" | awk '{print $1}')

python3 - "$root" "$ver" "$bin_sha" "$src_sha" <<'PY'
import pathlib, sys
root, ver, bin_sha, src_sha = sys.argv[1:]

def rewrite(pkgbuild, srcinfo, sha, source_line):
    pb = pathlib.Path(pkgbuild)
    text = pb.read_text()
    lines = []
    for line in text.splitlines(True):
        if line.startswith("pkgver="):
            lines.append(f"pkgver={ver}\n")
        elif line.startswith("sha256sums="):
            lines.append(f"sha256sums=('{sha}')\n")
        else:
            lines.append(line)
    pb.write_text("".join(lines))
    info = pathlib.Path(srcinfo)
    out = []
    for line in info.read_text().splitlines(True):
        if line.startswith("\tpkgver = "):
            out.append(f"\tpkgver = {ver}\n")
        elif line.startswith("\tsource = "):
            out.append(f"\t{source_line}\n")
        elif line.startswith("\tsha256sums = "):
            out.append(f"\tsha256sums = {sha}\n")
        else:
            out.append(line)
    info.write_text("".join(out))

rewrite(
    f"{root}/rivet-files-bin/PKGBUILD",
    f"{root}/rivet-files-bin/.SRCINFO",
    bin_sha,
    f"source = https://github.com/ulinaaron/rivet-files/releases/download/v{ver}/rivet-files-{ver}-x86_64-linux.tar.gz",
)
rewrite(
    f"{root}/rivet-files/PKGBUILD",
    f"{root}/rivet-files/.SRCINFO",
    src_sha,
    f"source = rivet-files-{ver}.tar.gz::https://github.com/ulinaaron/rivet-files/archive/refs/tags/v{ver}.tar.gz",
)
print(f"rivet-files-bin {ver} {bin_sha}")
print(f"rivet-files {ver} {src_sha}")
PY
