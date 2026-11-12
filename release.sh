#!/bin/sh
# Make a release of Rivet Files.
#
#   ./release.sh            build dist/rivet-files-VERSION-ARCH-linux.tar.gz (+ .sha256)
#                           and tag the current commit vVERSION (locally)
#   ./release.sh --publish  …then push the tag and create the GitHub release with that file
#
# The version comes from Cargo.toml; the release notes from its section in CHANGELOG.md.
# For a new release: bump `version` in Cargo.toml, add a CHANGELOG section, commit, run this.
set -e
cd "$(dirname "$0")"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)
NAME="rivet-files-$VERSION-$ARCH-linux"
TAG="v$VERSION"

if [ -n "$(git status --porcelain)" ]; then
    echo "Commit your changes first (the release is built from the current commit)." >&2
    exit 1
fi
grep -q "^## $VERSION" CHANGELOG.md || { echo "CHANGELOG.md has no section for $VERSION." >&2; exit 1; }

# A published version never moves to a different commit: ask for a new version instead.
HEAD_COMMIT=$(git rev-parse HEAD)
LOCAL_TAG=$(git rev-parse -q --verify "refs/tags/$TAG^{}" || true)
REMOTE_TAG=$(git ls-remote origin "refs/tags/$TAG^{}" 2>/dev/null | cut -f1)
for T in "$LOCAL_TAG" "$REMOTE_TAG"; do
    if [ -n "$T" ] && [ "$T" != "$HEAD_COMMIT" ]; then
        echo "$TAG already exists for an earlier commit (it may already be published)." >&2
        echo "Bump 'version' in Cargo.toml, add a CHANGELOG section for it, commit, and run this again." >&2
        exit 1
    fi
done

cargo build --release
mkdir -p dist
STAGE="dist/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE"
strip -o "$STAGE/rivet" target/release/rivet
cp data/rivet-files.desktop data/rivet-files.svg install.sh README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE "$STAGE/"
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
rm -rf "$STAGE"
(cd dist && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
./appimage.sh
APPIMAGE="dist/Rivet_Files-$VERSION-$ARCH.AppImage"

# Release notes = this version's CHANGELOG section.
awk -v v="## $VERSION" '$0 ~ "^## " { p = (index($0, v) == 1) } p' CHANGELOG.md | tail -n +2 > "dist/notes-$VERSION.md"
cat >> "dist/notes-$VERSION.md" <<EOF

### Install

**AppImage (one file, nothing to install):** download \`Rivet_Files-$VERSION-$ARCH.AppImage\`, then

\`\`\`sh
chmod +x Rivet_Files-$VERSION-$ARCH.AppImage
./Rivet_Files-$VERSION-$ARCH.AppImage
\`\`\`

Inside Rivet, Preferences → System → *Add Rivet to the app launcher* puts it in your menus.

**Archive:** download \`$NAME.tar.gz\`, then:

\`\`\`sh
tar xzf $NAME.tar.gz
cd $NAME
./install.sh          # adds Rivet to your app launcher (add --default to make it the default file manager)
\`\`\`

Built on $(. /etc/os-release && echo "$NAME" "$VERSION_ID") for $ARCH; needs glibc $(objdump -T target/release/rivet | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1 | cut -d_ -f2) or newer.
Older systems can build from source: \`cargo build --release\`.
EOF

git rev-parse -q --verify "refs/tags/$TAG" >/dev/null || git tag -a "$TAG" -m "Rivet Files $VERSION"
echo "Built $APPIMAGE and dist/$NAME.tar.gz, tagged $TAG."

if [ "$1" = "--publish" ]; then
    git push origin main "$TAG"
    gh release create "$TAG" "$APPIMAGE" "$APPIMAGE.sha256" "dist/$NAME.tar.gz" "dist/$NAME.tar.gz.sha256" \
        --title "Rivet Files $VERSION" --notes-file "dist/notes-$VERSION.md"
    gh release view "$TAG" --json url -q .url
fi
