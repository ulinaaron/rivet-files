#!/bin/sh
# Put Rivet on GitHub as a public repo (first time), or push new commits (later).
# Safe to run from any folder: it switches to the folder this script lives in.
set -e
cd "$(dirname "$0")/../.."
if git remote get-url origin >/dev/null 2>&1; then
    git push origin main
else
    gh repo create rivet-files --public --source=. --remote=origin --push \
        --description "Linux file manager like Nemo, Thunar and Caja, in Rust/egui — no GTK or Qt, system icons, live matugen colors"
fi
gh repo view --json url -q .url
