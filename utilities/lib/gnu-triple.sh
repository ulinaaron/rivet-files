#!/bin/sh
# Rust target triple for this machine's portable release build.
# x86_64 is the arch the release scripts pin toolchains and bundled libraries for.
set -eu
case "$(uname -m)" in
    x86_64) echo x86_64-unknown-linux-gnu ;;
    *)
        echo "Portable release builds are set up for x86_64, not $(uname -m)." >&2
        exit 1
        ;;
esac
