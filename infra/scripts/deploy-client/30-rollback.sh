#!/usr/bin/env bash
# Puts turn 2 back (binary link and settings, so the page is off again) from the copies 10-install.sh saved. Data is untouched.
# Usage: sudo ./30-rollback.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
BK=/var/lib/pero/turn2-backup
for f in server.toml current; do [ -f "$BK/$f" ] || { echo "Missing $BK/$f" >&2; exit 1; }; done
cp -p "$BK/server.toml" /etc/pero/server.toml
ln -sfn "$(cat "$BK/current")" /opt/pero/server/current
systemctl restart pero-server
sleep 2; systemctl is-active pero-server
