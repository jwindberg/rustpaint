#!/bin/bash
# Builds RustPaint.app from this checkout and puts it in /Applications, as the
# release workflow does but for this Mac's own processor only.
#
#   tools/make_mac_app.sh            # into /Applications
#   tools/make_mac_app.sh ~/Applications
#
# Written for the bash 3.2 that comes with macOS.
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "make_mac_app.sh makes a Mac app, so it runs on a Mac." >&2
    exit 1
fi

cd "$(dirname "$0")/.."
dest="${1:-/Applications}"
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -n 1)

cargo build --release --locked -p rustpaint

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app="$work/RustPaint.app/Contents"
mkdir -p "$app/MacOS" "$app/Resources"
cp target/release/RustPaint "$app/MacOS/RustPaint"
sed "s/VERSION/$version/g" packaging/macos/Info.plist > "$app/Info.plist"

cargo run --release --locked -p rustpaint --example make_icons -- "$work/icons"
iconset="$work/RustPaint.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
    cp "$work/icons/icon_$size.png" "$iconset/icon_${size}x${size}.png"
    cp "$work/icons/icon_$((size * 2)).png" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Resources/RustPaint.icns"

mkdir -p "$dest"
rm -rf "$dest/RustPaint.app"
ditto "$work/RustPaint.app" "$dest/RustPaint.app"
# So Finder and Launchpad notice the new version and its icon.
touch "$dest/RustPaint.app"

echo "RustPaint $version is in $dest/RustPaint.app"
