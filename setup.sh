#!/usr/bin/env bash
set -euo pipefail

ACTION=${1:-install}
PROJECT_ROOT=$(cd -- "$(dirname -- "$0")" && pwd)
BINARY_NAME="nebula-gtk"
INSTALL_PREFIX="/usr/libexec"
ICON_SOURCE_DIR="assets/icons/hicolor"
DESKTOP_SOURCE="assets/applications/tech.geektoshi.Nebula.desktop"
DESKTOP_TARGET="/usr/share/applications/tech.geektoshi.Nebula.desktop"

require_sudo() {
    sudo -v
}

install_dependencies() {
    local deps=(
        rustup
        gtk4-devel
        libadwaita-devel
        glib-devel
        pango-devel
        pkg-config
    )

    if ! command -v xbps-query >/dev/null 2>&1; then
        echo "xbps-query not found; installing full dependency set"
        sudo xbps-install -S "${deps[@]}"
        return
    fi

    local missing=()
    echo "Checking build dependencies..."
    for dep in "${deps[@]}"; do
        if xbps-query -p pkgver "$dep" >/dev/null 2>&1; then
            echo "  present: $dep"
        else
            echo "  missing: $dep"
            missing+=("$dep")
        fi
    done

    if (( ${#missing[@]} > 0 )); then
        echo "Installing missing packages: ${missing[*]}"
        sudo xbps-install -S "${missing[@]}"
    else
        echo "All dependencies satisfied."
    fi
}

build_release() {
    echo "Building release binary..."
    if [[ -n "${SUDO_USER:-}" && "$SUDO_USER" != "root" ]]; then
        sudo -u "$SUDO_USER" env PROJECT_ROOT="$PROJECT_ROOT" bash -lc '
            cd "$PROJECT_ROOT"
            if [[ -f "$HOME/.cargo/env" ]]; then
                source "$HOME/.cargo/env"
            fi
            if ! command -v cargo >/dev/null 2>&1; then
                echo "cargo not found in user environment" >&2
                exit 1
            fi
            cargo build --release
        '
    else
        if [[ -f "$HOME/.cargo/env" ]]; then
            # shellcheck disable=SC1090
            source "$HOME/.cargo/env"
        fi
        if ! command -v cargo >/dev/null 2>&1; then
            echo "cargo not found; install Rust toolchain" >&2
            exit 1
        fi
        cargo build --release
    fi
}

install_binary() {
    local source_path="target/release/${BINARY_NAME}"
    local target_path="${INSTALL_PREFIX}/${BINARY_NAME}"

    if [[ ! -f "$source_path" ]]; then
        echo "Release binary missing at $source_path" >&2
        exit 1
    fi

    echo "Installing binary to $target_path"
    sudo install -D -m755 "$source_path" "$target_path"
}

install_icons() {
    if [[ ! -d "$ICON_SOURCE_DIR" ]]; then
        echo "Icon source directory missing: $ICON_SOURCE_DIR"
        return
    fi

    local installed_any=false
    while IFS= read -r -d '' icon; do
        local relative=${icon#${ICON_SOURCE_DIR}/}
        local target="/usr/share/icons/hicolor/${relative}"
        echo "Installing icon $icon -> $target"
        sudo install -D -m644 "$icon" "$target"
        installed_any=true
    done < <(find "$ICON_SOURCE_DIR" -type f -name '*.png' -print0)

    if [[ "$installed_any" == true ]]; then
        refresh_icon_cache
    fi
}

install_desktop_entry() {
    if [[ ! -f "$DESKTOP_SOURCE" ]]; then
        echo "Desktop entry missing: $DESKTOP_SOURCE"
        return
    fi

    echo "Installing desktop entry to $DESKTOP_TARGET"
    sudo install -D -m644 "$DESKTOP_SOURCE" "$DESKTOP_TARGET"
    refresh_desktop_database
}

refresh_icon_cache() {
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        sudo gtk-update-icon-cache -f /usr/share/icons/hicolor
    fi
}

refresh_desktop_database() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        sudo update-desktop-database /usr/share/applications
    fi
}

uninstall_binary() {
    local target_path="${INSTALL_PREFIX}/${BINARY_NAME}"
    if [[ -f "$target_path" ]]; then
        echo "Removing $target_path"
        sudo rm -f "$target_path"
    fi
}

uninstall_icons() {
    if [[ ! -d "$ICON_SOURCE_DIR" ]]; then
        return
    fi

    local removed_any=false
    while IFS= read -r -d '' icon; do
        local relative=${icon#${ICON_SOURCE_DIR}/}
        local target="/usr/share/icons/hicolor/${relative}"
        if [[ -f "$target" ]]; then
            echo "Removing icon $target"
            sudo rm -f "$target"
            removed_any=true
        fi
    done < <(find "$ICON_SOURCE_DIR" -type f -name '*.png' -print0)

    if [[ "$removed_any" == true ]]; then
        refresh_icon_cache
    fi
}

uninstall_desktop_entry() {
    if [[ -f "$DESKTOP_TARGET" ]]; then
        echo "Removing desktop entry $DESKTOP_TARGET"
        sudo rm -f "$DESKTOP_TARGET"
        refresh_desktop_database
    fi
}

case "$ACTION" in
    install)
        cd "$PROJECT_ROOT"
        require_sudo
        install_dependencies
        build_release
        install_binary
        install_icons
        install_desktop_entry
        ;;
    uninstall)
        cd "$PROJECT_ROOT"
        require_sudo
        uninstall_binary
        uninstall_icons
        uninstall_desktop_entry
        ;;
    *)
        echo "Usage: $0 [install|uninstall]" >&2
        exit 1
        ;;
 esac

 echo "Done."
