#!/bin/sh
# Fail if an ELF needs a GLIBC symbol version newer than the ceiling.
#
#   ./scripts/check-glibc.sh --max 2.35 path/to/rivet [more ELFs...]
#
# Prints every GLIBC version objdump -T finds, then the highest. The release
# ceiling is 2.35 so the binary still starts on Ubuntu 22.04 and Debian 12.
set -eu

max=
record=
while [ $# -gt 0 ]; do
    case "$1" in
        --max)
            max=$2
            shift 2
            ;;
        --max=*)
            max=${1#--max=}
            shift
            ;;
        --record)
            record=$2
            shift 2
            ;;
        *)
            break
            ;;
    esac
done

if [ -z "$max" ] || [ $# -eq 0 ]; then
    echo "usage: $0 --max VERSION [--record FILE] ELF..." >&2
    exit 2
fi

# True when $1 <= $2 as dotted versions (GNU sort -V).
# The comparison sits inside `if` so a "greater than" result does not trip `set -e`.
version_le() {
    if [ "$1" = "$2" ]; then
        return 0
    fi
    if printf '%s\n%s\n' "$1" "$2" | sort -C -V; then
        return 0
    fi
    return 1
}

status=0
overall=
for bin in "$@"; do
    if [ ! -f "$bin" ]; then
        echo "error: not a file: $bin" >&2
        status=1
        continue
    fi
    tmp=$(mktemp)
    if ! objdump -T "$bin" >"$tmp" 2>"$tmp.err"; then
        echo "error: objdump -T failed for $bin" >&2
        cat "$tmp.err" >&2
        rm -f "$tmp" "$tmp.err"
        status=1
        continue
    fi
    rm -f "$tmp.err"
    # Versions appear as GLIBC_2.35 or (GLIBC_2.2.5) on undefined symbols.
    versions=$(sed -n 's/.*GLIBC_\([0-9][0-9.]*\).*/\1/p' "$tmp" | sort -Vu)
    rm -f "$tmp"
    if [ -z "$versions" ]; then
        echo "error: $bin has no GLIBC symbol versions (not a glibc ELF?)" >&2
        status=1
        continue
    fi
    highest=$(printf '%s\n' "$versions" | tail -n 1)
    echo "$bin"
    printf '%s\n' "$versions" | sed 's/^/  GLIBC_/'
    echo "  highest: GLIBC_$highest (ceiling GLIBC_$max)"
    if ! version_le "$highest" "$max"; then
        echo "error: $bin needs GLIBC_$highest, newer than GLIBC_$max" >&2
        status=1
    fi
    if [ -z "$overall" ] || ! version_le "$highest" "$overall"; then
        overall=$highest
    fi
done

if [ -n "$record" ] && [ -n "$overall" ]; then
    mkdir -p "$(dirname "$record")"
    printf '%s\n' "$overall" >"$record"
fi

exit "$status"
