#!/bin/bash
# Build rustpaint and install it for this user, into ~/.cargo/bin. Rerun to upgrade.
# Works from any folder: tools/install.sh, ../tools/install.sh, and so on.
# First it offers to install what building needs: Rust, and on Debian-family
# Linux (Raspberry Pi OS, Ubuntu, Mint) the packages below.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
PACKAGES=(build-essential pkg-config curl)

# What this app needs before it builds, and offers to install what's
# missing: Rust (with rustup) everywhere, and on Debian-family Linux
# (Raspberry Pi OS, Ubuntu, Mint) the system packages in PACKAGES, with apt.
ask() {
    # Yes unless answered no; no when there's no one to ask.
    [ -t 0 ] || return 1
    printf '%s [Y/n] ' "$1"
    read -r answer
    case "$answer" in [Nn]*) return 1 ;; *) return 0 ;; esac
}

if [ "$(uname -s)" = Linux ] && command -v dpkg-query >/dev/null && command -v apt-get >/dev/null; then
    missing=()
    for pkg in "${PACKAGES[@]}"; do
        dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null | grep -q "install ok installed" || missing+=("$pkg")
    done
    if [ ${#missing[@]} -gt 0 ]; then
        echo "Building needs these packages, which aren't installed: ${missing[*]}"
        if ask "Install them now (with sudo apt)?"; then
            sudo apt-get update
            sudo apt-get install -y "${missing[@]}"
        else
            echo "Install them with: sudo apt install ${missing[*]}" >&2
            exit 1
        fi
    fi
fi

# On a Mac, building needs Apple's command line tools (the linker).
if [ "$(uname -s)" = Darwin ] && ! xcode-select -p >/dev/null 2>&1; then
    echo "Building needs Apple's command line tools; macOS will offer to install them." >&2
    xcode-select --install || true
    echo "Run tools/install.sh again once they're installed." >&2
    exit 1
fi

# rustup puts cargo in ~/.cargo/bin, which a new shell may not have on its PATH yet.
cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
if ! command -v cargo >/dev/null && [ -x "$cargo_bin/cargo" ]; then
    export PATH="$cargo_bin:$PATH"
fi
if ! command -v cargo >/dev/null; then
    echo "Rust isn't installed."
    if ask "Install it now with rustup (from https://rustup.rs)?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        export PATH="$cargo_bin:$PATH"
    else
        echo "Install Rust from https://rustup.rs first." >&2
        exit 1
    fi
fi

cargo install --locked --path "$ROOT/crates/app"
