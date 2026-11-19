#!/bin/sh
# Builds Rexling and installs it for the current user: the binary, its launcher entry, its icon and its
# app metadata. PREFIX defaults to ~/.local. Run with --uninstall to remove them.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
binary=$prefix/bin/rexling
entry=$prefix/share/applications/dev.shulhi.Rexling.desktop
icon=$prefix/share/icons/hicolor/512x512/apps/dev.shulhi.Rexling.png
metainfo=$prefix/share/metainfo/dev.shulhi.Rexling.metainfo.xml

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$binary" "$entry" "$icon" "$metainfo"
    echo "Removed Rexling from $prefix"
    exit 0
fi

cargo build --release --locked --manifest-path "$here/Cargo.toml"
install -Dm755 "$here/target/release/rexling" "$binary"
install -Dm644 "$here/data/dev.shulhi.Rexling.desktop" "$entry"
# The launcher finds the binary even when ~/.local/bin is not on the session's PATH.
sed -i "s|^Exec=rexling$|Exec=$binary|" "$entry"
install -Dm644 "$here/data/dev.shulhi.Rexling.metainfo.xml" "$metainfo"
artwork=$here/../rexling-android/app/src/main/res/drawable-nodpi/rexlink_dinosaur.png
mkdir -p "$(dirname "$icon")"
# The artwork is larger than the icon slot; scale it when ImageMagick is available.
if command -v magick >/dev/null; then magick "$artwork" -resize 512x512 "$icon"; else install -m644 "$artwork" "$icon"; fi
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" 2>/dev/null || true
echo "Installed $binary"
