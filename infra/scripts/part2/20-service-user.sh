#!/usr/bin/env bash
# Part 2, step 2: dedicated non-root service user "pero" (no login, no password).
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }

if ! id pero >/dev/null 2>&1; then
  adduser --system --group --no-create-home --home /var/lib/pero --shell /usr/sbin/nologin pero
fi
install -d -o pero -g pero -m 0750 /var/lib/pero   # service data
install -d -o pero -g pero -m 0700 /etc/pero        # config and secrets, owner-only

getent passwd pero
passwd -S pero
stat -c '%U:%G %a %n' /var/lib/pero /etc/pero
echo "Step 2 done."
