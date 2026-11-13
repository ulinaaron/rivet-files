#!/bin/sh
# Path of the glibc-pinned release binary (after scripts/portable-build.sh).
set -eu
here=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
triple=$("$here/gnu-triple.sh")
printf '%s\n' "$root/target/$triple/release/rivet"
