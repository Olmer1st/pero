#!/usr/bin/env bash
# Part 3, step 2: TEMPORARY test listener on port 8443 so the client can prove it reaches the server.
# Runs as the unprivileged "pero" user from an empty temporary directory and stops by itself.
# Usage: sudo ./20-listener-test.sh [seconds]   (default 600; Ctrl-C stops it early)
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
SECS="${1:-600}"
[[ "$SECS" =~ ^[0-9]+$ ]] || { echo "seconds must be a number" >&2; exit 1; }
TMPD="$(mktemp -d)"; chown pero:pero "$TMPD"; chmod 0700 "$TMPD"
trap 'rm -rf "$TMPD"' EXIT

echo "Listening on 8443 for $SECS seconds. From the client:"
echo "  curl http://writer.local:8443/"
sudo -u pero bash -c "cd '$TMPD' && timeout $SECS python3 -m http.server 8443 --bind 0.0.0.0" || true
echo "Listener stopped."
