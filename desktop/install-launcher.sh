#!/usr/bin/env bash
# Adds "Pero" to this user's application menu (no root, nothing system-wide): a .desktop file in ~/.local/share/applications
# that starts the built app. Remove it again with: rm ~/.local/share/applications/pero.desktop
# Usage: desktop/install-launcher.sh
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
BIN="$ROOT/desktop/src-tauri/target/release/pero-desktop"
[ -x "$BIN" ] || { echo "Build the app first: desktop/build.sh" >&2; exit 1; }
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/pero.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Pero
Comment=Novel-writing assistant
Exec=$BIN
Icon=$ROOT/desktop/src-tauri/icons/icon.png
Terminal=false
Categories=Office;TextEditor;
DESKTOP
echo "Installed ~/.local/share/applications/pero.desktop"
