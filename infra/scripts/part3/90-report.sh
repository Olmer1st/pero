#!/usr/bin/env bash
# Part 3: writes a world-readable status report (no secrets) so it can be checked remotely.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OUT=/var/tmp/pero-part3-report.txt
{
  echo "== date";       date -Is
  echo "== avahi";      systemctl is-active avahi-daemon; journalctl -u avahi-daemon --no-pager -n 30 | grep -E 'Host name|conflict' || true
  echo "== ufw";        ufw status verbose
  echo "== listening";  ss -ltnu
} > "$OUT" 2>&1
chmod 0644 "$OUT"
echo "Report written to $OUT"
