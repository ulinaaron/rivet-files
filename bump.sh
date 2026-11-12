#!/bin/sh
# Bump the version for a new release.
#
#   ./bump.sh            patch: 0.1.1 -> 0.1.2
#   ./bump.sh minor      0.1.1 -> 0.2.0
#   ./bump.sh major      0.1.1 -> 1.0.0
#   ./bump.sh 0.3.0      an exact version
#
# Updates Cargo.toml and Cargo.lock, starts a CHANGELOG.md section from the commit
# messages since the last release tag, and commits it all as "Release X.Y.Z".
# Edit the changelog afterwards if you like (then `git commit --amend -a --no-edit`),
# and publish with ./release.sh --publish
set -e
cd "$(dirname "$0")"

if [ -n "$(git status --porcelain)" ]; then
    echo "Commit your changes first." >&2
    exit 1
fi

OLD=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
MAJOR=$(echo "$OLD" | cut -d. -f1)
MINOR=$(echo "$OLD" | cut -d. -f2)
PATCH=$(echo "$OLD" | cut -d. -f3)
case "${1:-patch}" in
    patch) NEW="$MAJOR.$MINOR.$((PATCH + 1))" ;;
    minor) NEW="$MAJOR.$((MINOR + 1)).0" ;;
    major) NEW="$((MAJOR + 1)).0.0" ;;
    [0-9]*.[0-9]*.[0-9]*) NEW="$1" ;;
    *) echo "Usage: ./bump.sh [patch|minor|major|X.Y.Z]" >&2; exit 1 ;;
esac
if git rev-parse -q --verify "refs/tags/v$NEW" >/dev/null; then
    echo "v$NEW already exists." >&2
    exit 1
fi

# Cargo.toml: only the package's own version line (the first one).
sed -i "0,/^version = \"$OLD\"/s//version = \"$NEW\"/" Cargo.toml
# Cargo.lock: record the new version without touching any dependency.
cargo update --workspace --offline >/dev/null 2>&1 || cargo update --workspace >/dev/null

# CHANGELOG: a new section from the commit subjects since the last tag.
LAST=$(git describe --tags --abbrev=0 2>/dev/null || true)
RANGE=${LAST:+$LAST..}HEAD
NOTES=$(git log --no-merges --format='- %s' $RANGE)
[ -n "$NOTES" ] || NOTES="- Maintenance release"
{
    sed -n '1,2p' CHANGELOG.md
    printf '## %s\n\n%s\n\n' "$NEW" "$NOTES"
    sed -n '3,$p' CHANGELOG.md
} > CHANGELOG.md.new
mv CHANGELOG.md.new CHANGELOG.md

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -q -m "Release $NEW"
echo "Bumped $OLD -> $NEW and committed. Review CHANGELOG.md, then run ./release.sh --publish"
