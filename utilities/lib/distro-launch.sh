#!/bin/sh
# Install and launch checks for a stock distro, plus X11 and Wayland.
#
#   utilities/lib/distro-launch.sh deps
#   utilities/lib/distro-launch.sh deps-display
#   utilities/lib/distro-launch.sh tarball DIST.tar.gz SHOTDIR
#   utilities/lib/distro-launch.sh appimage Rivet.AppImage SHOTDIR
#   utilities/lib/distro-launch.sh displays RIVET_BIN SHOTDIR
#   utilities/lib/distro-launch.sh all
#
# `deps` is the runtime set a desktop install needs, plus Xvfb so the GUI can
# open with no monitor. `deps-display` adds Sway and grim for the Wayland run.
# winit 0.30 picks the backend from WAYLAND_DISPLAY / DISPLAY. WINIT_UNIX_BACKEND
# is set as well; this winit does not read it.
set -eu

gl_env() {
    export LIBGL_ALWAYS_SOFTWARE=1
    export GALLIUM_DRIVER=llvmpipe
    export MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
}

png_ok() {
    file=$1
    if [ ! -s "$file" ]; then
        echo "No screenshot at $file" >&2
        return 1
    fi
    # Bytes 1-3 of a PNG are the ASCII letters PNG.
    if ! od -An -t x1 -N 4 "$file" | grep -q '89 50 4e 47'; then
        echo "Not a PNG: $file" >&2
        return 1
    fi
    size=$(wc -c < "$file")
    if [ "$size" -lt 5000 ]; then
        echo "Screenshot is only $size bytes: $file" >&2
        return 1
    fi
}

# Run rivet (or an AppImage prefix) long enough to prove it did not crash on open.
# Arguments are the program and any args that come before Rivet's own flags.
stay_up() {
    log=$(mktemp)
    gl_env
    "$@" --measure 12 "$HOME" >"$log" 2>&1 &
    pid=$!
    start=$(date +%s)
    sleep 2
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "Rivet exited in the first 2 seconds" >&2
        cat "$log" >&2
        wait "$pid" || true
        exit 1
    fi
    while [ $(($(date +%s) - start)) -lt 10 ]; do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "Rivet exited before 10 seconds" >&2
            cat "$log" >&2
            wait "$pid" || true
            exit 1
        fi
        sleep 1
    done
    set +e
    wait "$pid"
    ec=$?
    set -e
    if [ "$ec" -ne 0 ] || grep -q 'panicked at' "$log"; then
        echo "Rivet did not stay up cleanly (exit $ec)" >&2
        cat "$log" >&2
        exit 1
    fi
    rm -f "$log"
}

shot() {
    out=$1
    shift
    log=$(mktemp)
    gl_env
    if ! timeout 90 "$@" --screenshot "$out" "$HOME" >"$log" 2>&1; then
        echo "Screenshot launch failed" >&2
        cat "$log" >&2
        exit 1
    fi
    if grep -q 'panicked at' "$log"; then
        echo "Panic during screenshot" >&2
        cat "$log" >&2
        exit 1
    fi
    png_ok "$out"
    rm -f "$log"
}

start_xvfb() {
    if [ -n "${DISPLAY:-}" ] && [ -e "/tmp/.X11-unix/X${DISPLAY#:}" ]; then
        return 0
    fi
    Xvfb :99 -screen 0 1280x800x24 +extension GLX +render -noreset >/tmp/xvfb.log 2>&1 &
    i=0
    while [ ! -e /tmp/.X11-unix/X99 ]; do
        i=$((i + 1))
        if [ "$i" -gt 50 ]; then
            echo "Xvfb did not start" >&2
            cat /tmp/xvfb.log >&2 || true
            exit 1
        fi
        sleep 0.1
    done
    export DISPLAY=:99
    unset WAYLAND_DISPLAY WAYLAND_SOCKET || true
    export WINIT_UNIX_BACKEND=x11
    export XDG_SESSION_TYPE=x11
}

install_pkgs() {
    if [ "$(id -u)" -ne 0 ]; then
        echo "Installing packages needs root." >&2
        exit 1
    fi
    # shellcheck disable=SC1091
    . /etc/os-release
    wayland=0
    [ "${1:-}" = "--display" ] && wayland=1
    case "$ID" in
        ubuntu|debian)
            export DEBIAN_FRONTEND=noninteractive
            apt-get update
            # Runtime: what a desktop session already has, including the X11
            # keyboard library a minimal server image often omits.
            apt-get install -y --no-install-recommends \
                libxkbcommon0 libxkbcommon-x11-0 libxcb-xkb1 \
                libwayland-client0 libwayland-cursor0 libwayland-egl1 \
                libx11-6 libxcursor1 libxrandr2 libxi6 libxext6 libxrender1 libxxf86vm1 \
                libgl1 libegl1 libgl1-mesa-dri \
                shared-mime-info fonts-dejavu-core fontconfig \
                xvfb ca-certificates
            if [ "$wayland" -eq 1 ]; then
                apt-get install -y --no-install-recommends sway grim
            fi
            ;;
        fedora)
            dnf install -y --setopt=install_weak_deps=False \
                libxkbcommon libxkbcommon-x11 libxcb \
                wayland \
                libX11 libXcursor libXrandr libXi libXext libXrender libXxf86vm \
                mesa-libGL mesa-libEGL mesa-dri-drivers \
                shared-mime-info dejavu-sans-fonts fontconfig \
                xorg-x11-server-Xvfb
            if [ "$wayland" -eq 1 ]; then
                dnf install -y --setopt=install_weak_deps=False sway grim
            fi
            ;;
        arch)
            pacman -Syu --noconfirm --needed \
                libxkbcommon libxcb wayland \
                libx11 libxcursor libxrandr libxi libxext libxrender libxxf86vm \
                mesa shared-mime-info ttf-dejavu fontconfig \
                xorg-server-xvfb
            if [ "$wayland" -eq 1 ]; then
                pacman -S --noconfirm --needed sway grim
            fi
            ;;
        *)
            echo "No package list for $ID. See docs/testing.md." >&2
            exit 1
            ;;
    esac
}

check_cli() {
    "$@" --version
    "$@" --help | grep -q -- '--version'
    "$@" --help | grep -q -- '--help'
}

check_tarball() {
    tarball=$1
    outdir=$2
    mkdir -p "$outdir"
    work=$(mktemp -d)
    tar -xzf "$tarball" -C "$work"
    top=$(find "$work" -mindepth 1 -maxdepth 1 -type d | head -n 1)
    "$top/install.sh"
    bin="${XDG_BIN_HOME:-$HOME/.local/bin}/rivet"
    check_cli "$bin"
    start_xvfb
    stay_up "$bin"
    shot "$outdir/tarball-gui.png" "$bin"
    "$top/install.sh" --uninstall
    apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications/rivet-files.desktop"
    icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons"
    if [ -e "$bin" ] || [ -e "$apps" ]; then
        echo "Uninstall left the program or the desktop entry in place." >&2
        exit 1
    fi
    if [ -d "$icons/hicolor" ] && find "$icons/hicolor" -type f -name 'rivet-files.*' | grep -q .; then
        echo "Uninstall left icon files behind." >&2
        exit 1
    fi
    echo "Tarball install, launch, and uninstall succeeded."
}

check_appimage() {
    app=$1
    outdir=$2
    mkdir -p "$outdir"
    chmod +x "$app"
    check_cli "$app" --appimage-extract-and-run
    start_xvfb
    stay_up "$app" --appimage-extract-and-run
    shot "$outdir/appimage-gui.png" "$app" --appimage-extract-and-run
    echo "AppImage launch succeeded."
}

check_displays() {
    bin=$1
    outdir=$2
    mkdir -p "$outdir"
    command -v Xvfb >/dev/null
    command -v sway >/dev/null
    command -v grim >/dev/null

    start_xvfb
    stay_up "$bin"
    shot "$outdir/x11.png" "$bin"
    echo "X11 launch succeeded."

    runtime=${XDG_RUNTIME_DIR:-/tmp/rivet-runtime-$$}
    mkdir -p "$runtime"
    chmod 700 "$runtime"
    export XDG_RUNTIME_DIR=$runtime
    unset DISPLAY || true
    export WLR_BACKENDS=headless
    export WLR_RENDERER=pixman
    export WLR_LIBINPUT_NO_DEVICES=1
    cfg=$(mktemp)
    cat >"$cfg" <<'EOF'
default_border none
output * bg #333333 solid_color
EOF
    sway -c "$cfg" >/tmp/sway.log 2>&1 &
    sway_pid=$!
    i=0
    sock=
    while [ -z "$sock" ]; do
        i=$((i + 1))
        if [ "$i" -gt 50 ]; then
            echo "Sway did not open a Wayland socket" >&2
            cat /tmp/sway.log >&2 || true
            exit 1
        fi
        sleep 0.1
        sock=$(find "$runtime" -maxdepth 1 -type s -name 'wayland-*' | head -n 1 || true)
    done
    WAYLAND_DISPLAY=$(basename "$sock")
    export WAYLAND_DISPLAY
    export WINIT_UNIX_BACKEND=wayland
    export XDG_SESSION_TYPE=wayland
    log=$(mktemp)
    gl_env
    start=$(date +%s)
    "$bin" --measure 25 "$HOME" >"$log" 2>&1 &
    pid=$!
    sleep 4
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "Rivet exited before a Wayland screenshot" >&2
        cat "$log" >&2
        cat /tmp/sway.log >&2 || true
        exit 1
    fi
    grabbed=0
    i=0
    while [ "$i" -lt 12 ]; do
        if grim "$outdir/wayland.png" && png_ok "$outdir/wayland.png"; then
            grabbed=1
            break
        fi
        i=$((i + 1))
        sleep 1
        if ! kill -0 "$pid" 2>/dev/null; then
            break
        fi
    done
    if [ "$grabbed" -ne 1 ]; then
        echo "grim could not take a Wayland screenshot" >&2
        cat /tmp/sway.log >&2 || true
        cat "$log" >&2
        kill "$pid" 2>/dev/null || true
        exit 1
    fi
    elapsed=$(($(date +%s) - start))
    if [ "$elapsed" -lt 10 ]; then
        sleep $((10 - elapsed))
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "Rivet exited before 10 seconds on Wayland" >&2
        cat "$log" >&2
        exit 1
    fi
    set +e
    wait "$pid"
    ec=$?
    set -e
    kill "$sway_pid" 2>/dev/null || true
    if [ "$ec" -ne 0 ] || grep -q 'panicked at' "$log"; then
        echo "Wayland launch failed (exit $ec)" >&2
        cat "$log" >&2
        exit 1
    fi
    png_ok "$outdir/wayland.png"
    echo "Wayland launch succeeded."
}

cmd=${1:-}
case "$cmd" in
    deps) install_pkgs ;;
    deps-display) install_pkgs --display ;;
    tarball) check_tarball "$2" "$3" ;;
    appimage) check_appimage "$2" "$3" ;;
    displays) check_displays "$2" "$3" ;;
    x11)
        mkdir -p "$3"
        start_xvfb
        stay_up "$2"
        shot "$3/x11.png" "$2"
        ;;
    all)
        install_pkgs --display
        tarball=$(echo dist/rivet-files-*-linux.tar.gz)
        app=$(echo dist/Rivet_Files-*.AppImage)
        check_tarball "$tarball" shots
        check_appimage "$app" shots
        bin=$(find dist -maxdepth 2 -type f -name rivet -perm -111 | head -n 1 || true)
        if [ -z "$bin" ]; then
            work=$(mktemp -d)
            tar -xzf "$tarball" -C "$work"
            bin=$(find "$work" -type f -name rivet -perm -111 | head -n 1)
        fi
        check_displays "$bin" shots
        ;;
    *)
        echo "Usage: $0 deps|deps-display|tarball TAR SHOTDIR|appimage APP SHOTDIR|displays BIN SHOTDIR|all" >&2
        exit 2
        ;;
esac
