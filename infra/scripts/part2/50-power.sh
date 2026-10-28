#!/usr/bin/env bash
# Part 2, step 5: never sleep, including when the lid is closed.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }

install -d /etc/systemd/logind.conf.d
cat > /etc/systemd/logind.conf.d/10-pero-lid.conf <<'CONF'
[Login]
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
HandleLidSwitchDocked=ignore
IdleAction=ignore
CONF
systemctl mask sleep.target suspend.target hibernate.target hybrid-sleep.target
systemctl kill -s HUP systemd-logind   # reload logind config without restarting it

systemctl is-enabled sleep.target suspend.target hibernate.target hybrid-sleep.target || true
echo "Step 5 done."
echo "Rollback: sudo rm /etc/systemd/logind.conf.d/10-pero-lid.conf && sudo systemctl unmask sleep.target suspend.target hibernate.target hybrid-sleep.target && sudo systemctl kill -s HUP systemd-logind"
