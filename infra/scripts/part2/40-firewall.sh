#!/usr/bin/env bash
# Part 2, step 4: firewall, default-deny inbound; SSH, Cockpit and the Pero server (8443)
# only from the home subnet. Tailscale rules are added in part 3.
# Usage: sudo ./40-firewall.sh <home-subnet-cidr>
# Safety net: the firewall switches itself off after 5 minutes unless you confirm.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
SUBNET="${1:?usage: sudo $0 <home-subnet-cidr>}"
[[ "$SUBNET" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || { echo "Bad CIDR: $SUBNET" >&2; exit 1; }

DEBIAN_FRONTEND=noninteractive apt-get -y install ufw

ufw default deny incoming
ufw default allow outgoing
ufw allow from "$SUBNET" to any port 22 proto tcp comment 'ssh, home subnet'
ufw allow from "$SUBNET" to any port 9090 proto tcp comment 'cockpit, home subnet'
ufw allow from "$SUBNET" to any port 8443 proto tcp comment 'pero server, home subnet'

systemctl stop pero-ufw-rollback.timer 2>/dev/null || true
systemd-run --quiet --unit=pero-ufw-rollback --on-active=300 "$(command -v ufw)" disable
ufw --force enable

ufw status verbose
echo "Step 4 done. The firewall will be DISABLED automatically in 5 minutes."
echo "From another machine confirm SSH works, then cancel the rollback with:"
echo "  sudo systemctl stop pero-ufw-rollback.timer"
