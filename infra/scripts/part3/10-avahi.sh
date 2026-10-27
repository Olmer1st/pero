#!/usr/bin/env bash
# Part 3, step 1: announce this server as writer.local on the home network (mDNS).
# Usage: sudo ./10-avahi.sh <home-subnet-cidr>
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
SUBNET="${1:?usage: sudo $0 <home-subnet-cidr>}"
[[ "$SUBNET" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || { echo "Bad CIDR: $SUBNET" >&2; exit 1; }

DEBIAN_FRONTEND=noninteractive apt-get -y install --no-install-recommends avahi-daemon
[ -f /etc/avahi/avahi-daemon.conf.orig ] || cp /etc/avahi/avahi-daemon.conf /etc/avahi/avahi-daemon.conf.orig

cat > /etc/avahi/avahi-daemon.conf <<'CONF'
[server]
host-name=writer
use-ipv4=yes
use-ipv6=no

[wide-area]
enable-wide-area=no

[publish]
publish-hinfo=no
publish-workstation=no
publish-domain=no

[reflector]

[rlimits]
rlimit-core=0
rlimit-data=4194304
rlimit-fsize=0
rlimit-nofile=768
rlimit-stack=4194304
rlimit-nproc=3
CONF

ufw allow from "$SUBNET" to any port 5353 proto udp comment 'mDNS, home subnet'
systemctl enable avahi-daemon
systemctl restart avahi-daemon
sleep 2
systemctl is-active avahi-daemon
journalctl -u avahi-daemon --no-pager -n 20 | grep -E 'Host name|conflict|failed' || true
echo "Step 1 done. Original config kept as /etc/avahi/avahi-daemon.conf.orig"
