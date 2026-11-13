#!/bin/sh
# Build the release binary linked for an old glibc, regardless of the host.
#
# Host linkers bind atan2f/acosf to whatever libm they have. On glibc 2.43 that
# is GLIBC_2.43, and the binary then refuses to start on Debian 13 and older.
# cargo-zigbuild links those symbols against glibc 2.31 instead.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
# shellcheck disable=SC1091
. "$ROOT/utilities/lib/ensure-zigbuild.sh"

triple=$("$ROOT/utilities/lib/gnu-triple.sh")
target=$triple.$GLIBC_LINK_VERSION
echo "Linking rivet for glibc $GLIBC_LINK_VERSION ($target) with $($CARGO_ZIGBUILD_ZIG_PATH version)"
cargo zigbuild --release --locked --target "$target"
bin=$("$ROOT/utilities/lib/portable-bin.sh")
[ -x "$bin" ] || { echo "expected $bin" >&2; exit 1; }
"$ROOT/utilities/lib/check-glibc.sh" --max "$GLIBC_MAX_VERSION" "$bin"
echo "$bin"
