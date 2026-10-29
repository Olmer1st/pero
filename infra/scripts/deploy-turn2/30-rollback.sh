#!/usr/bin/env bash
# Puts the turn 1 setup back (binary, settings, unit, certificate) from the copies 10-install.sh saved. Keeps the data folder.
# Usage: sudo ./20-rollback.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
BK=/var/lib/pero/turn1-backup
for f in pero-server.service server.toml server.crt current; do [ -f "$BK/$f" ] || { echo "Missing $BK/$f" >&2; exit 1; }; done
cp -p "$BK/pero-server.service" /etc/systemd/system/pero-server.service
cp -p "$BK/server.toml" /etc/pero/server.toml
cp -p "$BK/server.crt" /etc/pero/tls/server.crt
ln -sfn "$(cat "$BK/current")" /opt/pero/server/current
systemctl daemon-reload; systemctl restart pero-server
sleep 2; systemctl is-active pero-server
