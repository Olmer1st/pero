#!/usr/bin/env bash
# Part 2, step 1: apply updates and make sure automatic security updates are on.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get -y -o Dpkg::Options::=--force-confold upgrade
apt-get -y install unattended-upgrades

cat > /etc/apt/apt.conf.d/20auto-upgrades <<'CONF'
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
CONF
systemctl enable --now unattended-upgrades

unattended-upgrade --dry-run 2>&1 | tail -3 || true
[ -e /var/run/reboot-required ] && echo "NOTE: a reboot is required." || echo "No reboot required."
echo "Step 1 done."
