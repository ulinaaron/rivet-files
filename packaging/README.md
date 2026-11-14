# Distributing Rivet Files

The AppImage and the `.tar.gz` archive stay as they are. Flatpak and Arch packages are added beside them. Neither Flatpak on Flathub nor the AUR package can be published from this repository alone.

## What the sandbox allows

The manifest grants `--filesystem=host` plus `host-os` and `host-etc`, because a file manager has to read and write the computer's files. Inside the sandbox Rivet also ignores Flatpak's private `XDG_CONFIG_HOME` and `XDG_DATA_HOME`, so these stay on the real home directory:

| Feature | Where it lives | In the Flatpak |
| --- | --- | --- |
| Browsing home and other folders | `/home`, and any path you type | Read and write, including `/mnt`, `/media` and `/run/media` |
| Trash | `~/.local/share/Trash` | The desktop Trash, shared with other file managers |
| Drives | mount points from `/proc/self/mounts` | Listed when they are mounted in those directories. Unmount calls `udisksctl`, which is not inside the runtime, so Unmount can fail even though the drive opens |
| Icon theme | `~/.icons`, `~/.local/share/icons`, and the host theme under `/run/host/usr/share/icons` | Read from the host. GNOME's theme name is read from the host dconf |
| matugen, Noctalia, GTK colors | `~/.config/rivet`, `~/.config/gtk-3.0`, `~/.config/gtk-4.0`, `~/.config/matugen` | Read and watched on the real files, so a theme tool that rewrites them restyles Rivet |

Flatpak cannot put the host's `/usr` and `/etc` at `/usr` and `/etc`, because those paths are the runtime. The sidebar entry **Host system files** opens `/run/host`, which is the computer's `/usr`, `/bin` and `/etc`.

Opening a file uses the runtime's `xdg-open`, which asks the desktop portal to launch the usual application. **Open With** a specific program, and **Open in Terminal**, start a host binary by its path. Those binaries are not in the sandbox, so those two actions do not work in the Flatpak. Default open, browsing, copy, and Trash do.

Build it from this checkout:

```sh
flatpak install -y flathub org.freedesktop.Platform//26.08 org.freedesktop.Sdk//26.08 org.freedesktop.Sdk.Extension.rust-stable//26.08 org.flatpak.Builder
flatpak run org.flatpak.Builder --force-clean --install --user flatpak-build packaging/flatpak/io.github.ulinaaron.rivet-files.yml
flatpak run io.github.ulinaaron.rivet-files
```

CI builds `rivet-files.flatpak` on pull requests and attaches that file to a published GitHub release. Installing that build on a clean distro, and opening it under X11 and Wayland, is a separate workflow described in [docs/testing.md](../docs/testing.md). It does not run on pull requests.

When `Cargo.lock` changes, regenerate the offline crate list (no network during the Flatpak build):

```sh
python3 flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
```

The script is [flatpak-cargo-generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo). CI does not refresh it for you.

## Submitting to Flathub

This needs the GitHub account that owns the app. Flathub will not accept the pull request from anyone else as an upstream submission, and a file manager is only considered when the upstream author asks for it. Do this yourself in a browser. Flathub's policy also says the manifest in the Flathub repository must not be AI-generated content, and that an agent must not open the pull request. Read the manifest, the desktop file and the metainfo, edit anything you want to change, and submit that reviewed copy.

Wait until the Flatpak path fixes are in a tagged release. The copy in this repo builds the checkout (`type: dir`). Flathub rejects a `dir` source. The submission has to name a git tag and the full commit of that tag.

1. Install [GitHub CLI](https://cli.github.com/) and log in as the owner (`gh auth login`), or use the GitHub website for the fork and the pull request.
2. Confirm two-factor authentication is turned on for that GitHub account. Flathub requires it before they grant write access.
3. Fork `flathub/flathub`. On the website, uncheck **Copy the master branch only** so the `new-pr` branch exists. With the CLI, from an empty directory:

   ```sh
   gh repo fork --clone flathub/flathub
   cd flathub
   git checkout --track origin/new-pr
   git checkout -b add-io.github.ulinaaron.rivet-files new-pr
   ```

4. Copy only these two files into that branch, at the top of the tree (not under `packaging/`):

   - `packaging/flatpak/io.github.ulinaaron.rivet-files.yml`, renamed to `io.github.ulinaaron.rivet-files.yml`
   - `packaging/flatpak/cargo-sources.json`

   Do not copy the Rust source, the icons, or a built Flatpak. Flathub builds from the git tag. The desktop file, metainfo and icons are installed from that tag by the manifest's `build-commands`.

5. In the copied manifest, replace the `dir` source with the release you are shipping. `COMMIT` is the full `git rev-parse vX.Y.Z^{}`:

   ```yaml
       - type: git
         url: https://github.com/ulinaaron/rivet-files.git
         tag: vX.Y.Z
         commit: COMMIT
       - cargo-sources.json
   ```

   If `Cargo.lock` on that tag differs from the one used to generate `cargo-sources.json`, regenerate the JSON from that tag first.

6. Lint it the way Flathub does:

   ```sh
   flatpak install -y flathub org.flatpak.Builder
   flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest io.github.ulinaaron.rivet-files.yml
   flatpak run org.flatpak.Builder --force-clean --repo=repo build io.github.ulinaaron.rivet-files.yml
   flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
   ```

   Fix every error except the three filesystem permissions in the next step. Those stay.

7. The linter reports three errors for permissions a file manager cannot drop: `finish-args-host-filesystem-access`, `finish-args-host-os-filesystem-access`, and `finish-args-host-etc-filesystem-access`. Flathub only ignores those after an exception is merged in [flathub-infra/flatpak-builder-lint](https://github.com/flathub-infra/flatpak-builder-lint), in `flatpak_builder_lint/staticfiles/exceptions.json`. Open that pull request yourself. Flathub says not to have an agent file exception requests. One entry, with a reason in your own words, along these lines: Rivet is a file manager and has to read and write files outside the home directory, including mounted drives (`host`); host icon themes and the MIME database live on the host `/usr` (`host-os`); host `/etc` is the computer's configuration, not the runtime's (`host-etc`).

8. Commit, push the fork, and open a pull request **against `flathub/flathub`'s `new-pr` branch**, not `master`. Title: `Add io.github.ulinaaron.rivet-files`.

   In the description, state that you are the upstream author and that you will maintain the Flatpak. Explain `filesystem=host`: Rivet has to browse, copy and trash files anywhere on the computer, and it has to read the icon theme and the color files in `~/.config`. Mention the limitations above (host `/usr` is under `/run/host`, Unmount may fail, Open With and Open in Terminal do not launch host programs).

9. Answer review comments on that pull request. When a reviewer asks for a test build, comment `bot, build` on the pull request. Do not close and reopen it to update the branch.

10. After it is merged, Flathub creates `flathub/io.github.ulinaaron.rivet-files` and invites the GitHub account. Accept within a week. Later updates are commits to that repository's `master`, not another submission through `new-pr`.

The app id `io.github.ulinaaron.rivet-files` is the Flathub id for `https://github.com/ulinaaron/rivet-files`.

## Publishing to the AUR

Two packages live here:

- `packaging/aur/rivet-files-bin` installs the release tarball (`rivet` plus the desktop file and icons).
- `packaging/aur/rivet-files` builds from the git tag with cargo.

They conflict with each other. `rivet-files-bin` is the one most people want.

The published 0.1.2 tarball was linked on Fedora 44 and does not start on older glibc. Let the portable release go out first. The workflow `.github/workflows/aur-bump.yml` rewrites `pkgver` and the checksums on `main` when a GitHub release is published. If branch protection rejects that push, run `./packaging/aur/bump.sh X.Y.Z` yourself and commit it.

The AUR is a separate git host. This repository cannot push there.

1. Create an account at <https://aur.archlinux.org/register> and add an SSH public key in the account settings.
2. Clone each empty package (the first push creates it) and copy the files from this repo:

   ```sh
   git clone ssh://aur@aur.archlinux.org/rivet-files-bin.git
   cp packaging/aur/rivet-files-bin/PKGBUILD packaging/aur/rivet-files-bin/.SRCINFO rivet-files-bin/
   cd rivet-files-bin
   git add PKGBUILD .SRCINFO
   git commit -m "Initial import of rivet-files-bin 0.1.2"
   git push origin master

   cd ..
   git clone ssh://aur@aur.archlinux.org/rivet-files.git
   cp packaging/aur/rivet-files/PKGBUILD packaging/aur/rivet-files/.SRCINFO rivet-files/
   cd rivet-files
   git add PKGBUILD .SRCINFO
   git commit -m "Initial import of rivet-files 0.1.2"
   git push origin master
   ```

3. For a later release, after `bump.sh` has updated the files here, copy the new `PKGBUILD` and `.SRCINFO` into those clones, commit, and push `master` again.

`.SRCINFO` has to be committed. `makepkg --printsrcinfo > .SRCINFO` on an Arch machine regenerates it; `bump.sh` updates the version and checksum lines in place so you do not need Arch just to bump them.
