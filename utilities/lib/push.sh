#!/bin/sh
# Send your latest commits to GitHub (or create the public repo the first time).
# Makes no release: for that, use `utilities/rivet-dev release --publish`.
# First takes in commits GitHub has that you don't (e.g. the automatic "aur: X.Y.Z"
# commit after a release), so the push isn't rejected.
set -e
cd "$(dirname "$0")/../.."
if git remote get-url origin >/dev/null 2>&1; then
    git pull --rebase origin main
    git push origin main
else
    gh repo create rivet-files --public --source=. --remote=origin --push \
        --description "Linux file manager like Nemo, Thunar and Caja, in Rust/egui — no GTK or Qt, system icons, live matugen colors"
fi
gh repo view --json url -q .url
