# Launch tests on stock distros

Pull-request CI builds and tests Rivet. It does not install it on a clean distro or open a window. That check is a separate workflow, [Distro launch](../.github/workflows/distro-launch.yml). It runs when you start it by hand, and when a `v*` tag is pushed. It does not run on every push or pull request.

## What it covers

One job builds the portable release tarball and the AppImage (glibc 2.31, same as `utilities/rivet-dev portable`). The install jobs download those files. They do not compile again.

| Job | Distros | What it checks |
| --- | --- | --- |
| Install | Ubuntu 22.04, Ubuntu 24.04, Debian 12, Fedora latest, Arch latest | The runtime packages below, `install.sh`, `rivet --version` and `--help`, a GUI open under Xvfb that stays up for 10 seconds and writes a screenshot, `install.sh --uninstall` removes the binary, desktop entry and icons, then the same launch through the AppImage (`--appimage-extract-and-run` when FUSE is missing) |
| X11 and Wayland | Ubuntu 24.04 | Xvfb with `WAYLAND_DISPLAY` unset, then Sway with `WLR_BACKENDS=headless` and a grim screenshot. `WINIT_UNIX_BACKEND` is set too; winit 0.30.13 actually chooses from `WAYLAND_DISPLAY` and `DISPLAY` |
| Flatpak sockets | the runner | The manifest contains `--socket=wayland` and `--socket=fallback-x11` |
| Flatpak under Xvfb | only if you check the box | Builds the Flatpak and opens it under Xvfb. Left off by default because it compiles inside the SDK |

Screenshots are uploaded as workflow artifacts.

## Runtime packages

These are the libraries a normal desktop already has. A minimal server image often still needs `libxkbcommon-x11` (and on Debian and Ubuntu, `libxcb-xkb`). The workflow installs this set and nothing broader (`--no-install-recommends`, no weak Fedora dependencies). Xvfb is only there so the test can open a window. Sway and grim are only on the Wayland job.

| Distro | Packages |
| --- | --- |
| Ubuntu 22.04, Ubuntu 24.04, Debian 12 | `libxkbcommon0` `libxkbcommon-x11-0` `libxcb-xkb1` `libwayland-client0` `libwayland-cursor0` `libwayland-egl1` `libx11-6` `libxcursor1` `libxrandr2` `libxi6` `libxext6` `libxrender1` `libxxf86vm1` `libgl1` `libegl1` `libgl1-mesa-dri` `shared-mime-info` `fonts-dejavu-core` `fontconfig` |
| Fedora | `libxkbcommon` `libxkbcommon-x11` `libxcb` `wayland` `libX11` `libXcursor` `libXrandr` `libXi` `libXext` `libXrender` `libXxf86vm` `mesa-libGL` `mesa-libEGL` `mesa-dri-drivers` `shared-mime-info` `dejavu-sans-fonts` `fontconfig` |
| Arch | `libxkbcommon` `libxcb` `wayland` `libx11` `libxcursor` `libxrandr` `libxi` `libxext` `libxrender` `libxxf86vm` `mesa` `shared-mime-info` `ttf-dejavu` `fontconfig` |

The lists live in `utilities/lib/distro-launch.sh`, which is what CI runs.

## Run it on GitHub

Actions → **Distro launch** → **Run workflow**. Leave the boxes checked for a full run, or clear a distro to skip it. **Also build the Flatpak and open it under Xvfb** is off unless you turn it on.

From the command line, defaults are the full distro set and both display servers, without the Flatpak GUI:

```sh
gh workflow run distro-launch.yml --ref main
```

Skip Arch, for example:

```sh
gh workflow run distro-launch.yml --ref main -f arch=false
```

## Run it on this computer

Build the same artifacts CI uses, then hand them to a stock container. Docker has to be able to install packages, so the container needs a network.

```sh
utilities/rivet-dev portable
utilities/rivet-dev appimage
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
name="rivet-files-$version-$arch-linux"
stage="dist/$name"
rm -rf "$stage" && mkdir -p "$stage"
strip -o "$stage/rivet" "$(utilities/rivet-dev portable-bin)"
cp data/rivet-files.desktop data/rivet-files.svg utilities/lib/install.sh README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE "$stage/"
cp -r data/icons "$stage/icons"
tar -C dist -czf "dist/$name.tar.gz" "$name"

docker run --rm --user root -v "$PWD:/src" -w /src ubuntu:24.04 \
  utilities/lib/distro-launch.sh all
```

`all` installs the Ubuntu packages, checks the tarball and the AppImage, then X11 and Wayland. For one piece:

```sh
docker run --rm --user root -v "$PWD:/src" -w /src debian:12 \
  sh -c 'utilities/lib/distro-launch.sh deps && utilities/lib/distro-launch.sh tarball dist/rivet-files-*-linux.tar.gz shots'
```

Screenshots land in `shots/`. `install.sh --uninstall` (or `utilities/rivet-dev install --uninstall` from a checkout) removes `~/.local/bin/rivet`, the desktop entry, and the `rivet-files` icons.
