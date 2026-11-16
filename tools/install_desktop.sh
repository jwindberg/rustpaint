#!/bin/bash
# Puts RustPaint in the desktop's app menu, for this user: its menu entry (from
# packaging/linux) and its icons, pointing at the program tools/install.sh
# installed. For Linux and other systems with freedesktop menus; rerun it
# after installing a new version. On a Mac, use tools/make_mac_app.sh.
set -euo pipefail

if [ "$(uname -s)" = "Darwin" ]; then
    echo "On a Mac, use tools/make_mac_app.sh instead." >&2
    exit 1
fi

cd "$(dirname "$0")/.."
# The menu may not search ~/.cargo/bin, so the entry names the program's path.
bin=$(command -v RustPaint || true)
if [ -z "$bin" ] && [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/RustPaint" ]; then
    bin="${CARGO_HOME:-$HOME/.cargo}/bin/RustPaint"
fi
if [ -z "$bin" ]; then
    echo "RustPaint isn't installed yet; run tools/install.sh first." >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo run --release --locked -p rustpaint --example make_icons -- "$work/icons"

data="${XDG_DATA_HOME:-$HOME/.local/share}"
for size in 16 32 64 128 256 512; do
    install -D -m 644 "$work/icons/icon_$size.png" "$data/icons/hicolor/${size}x${size}/apps/rustpaint.png"
done
mkdir -p "$data/applications"
sed "s|^Exec=[^ ]*|Exec=$bin|" packaging/linux/rustpaint.desktop > "$data/applications/rustpaint.desktop"

# So menus notice the new entry and icon right away.
if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$data/applications" || true
fi
if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -q -t "$data/icons/hicolor" || true
fi
echo "RustPaint is in the app menu, running $bin."
