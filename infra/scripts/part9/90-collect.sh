#!/usr/bin/env bash
# Part 9 (server, root): read-only collection of security baseline facts.
# Changes nothing on the system. Writes /var/tmp/pero-part9-baseline.txt (world-readable, so it must contain no secrets):
# no key material, no environment files, no passwords, no manuscript data.
# Usage: sudo ./90-collect.sh
set -uo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OUT=/var/tmp/pero-part9-baseline.txt
sec(){ printf '\n== %s\n' "$1"; }
{
sec "date and system"
date -Is; . /etc/os-release; echo "$PRETTY_NAME"; uname -r; uptime -p
sec "listening sockets (TCP and UDP) with owner"
ss -tulnp | awk 'NR==1 || /LISTEN|UNCONN/'
sec "firewall"
ufw status verbose
sec "ssh effective settings"
sshd -T 2>/dev/null | grep -E '^(port|listenaddress|permitrootlogin|passwordauthentication|kbdinteractiveauthentication|pubkeyauthentication|permitemptypasswords|x11forwarding|maxauthtries|allowusers|allowgroups|authenticationmethods|usepam|logingracetime) '
ls /etc/ssh/sshd_config.d/
sec "accounts with a login shell or uid >= 1000"
getent passwd | awk -F: '($3>=1000 && $3<65534) || $7 ~ /(bash|sh|zsh)$/ {print $1":uid="$3":home="$6":shell="$7}'
echo "service user:"; getent passwd pero | awk -F: '{print $1":uid="$3":home="$6":shell="$7}'
echo "members of sudo group: $(getent group sudo | cut -d: -f4)"
echo "password state of pero (L or NP means no password login): $(passwd -S pero | awk '{print $2}')"
sec "sudoers drop-ins (any file here is a grant)"
ls -l /etc/sudoers.d/
echo "NOPASSWD rules visible to sudo:"; grep -rEh '^[^#].*NOPASSWD' /etc/sudoers /etc/sudoers.d 2>/dev/null || echo "none"
echo "provisioning script copies (root-owned, no grant left means they are inert):"; ls -l /opt/pero-provision/ 2>/dev/null
sec "Pero services"
for s in pero-llm pero-languagetool pero-server; do
  echo "-- $s: $(systemctl is-enabled $s) $(systemctl is-active $s) user=$(systemctl show $s -p User --value) restarts=$(systemctl show $s -p NRestarts --value)"
  systemd-analyze security "$s" --no-pager 2>/dev/null | tail -n 1
done
echo "-- other enabled services:"; systemctl list-unit-files --type=service --state=enabled --no-legend --no-pager | awk '{print $1}'
echo "-- enabled sockets:"; systemctl list-unit-files --type=socket --state=enabled --no-legend --no-pager | awk '{print $1}'
echo "-- failed units:"; systemctl --failed --no-legend --no-pager
sec "Pero files: owners and permissions"
ls -ld /etc/pero /etc/pero/tls /var/lib/pero /var/lib/pero/models /opt/pero /opt/pero/*
ls -l /etc/pero /etc/pero/tls
echo "world-writable files or directories under /opt/pero /etc/pero /var/lib/pero:"; find /opt/pero /etc/pero /var/lib/pero -perm -o+w 2>/dev/null | head; echo "(end)"
echo "files under /opt/pero not owned by root:"; find /opt/pero ! -user root 2>/dev/null | head; echo "(end)"
sec "SHA-256 of installed programs and model"
sha256sum /opt/pero/llama.cpp/current/bin/llama-server /opt/pero/server/current/pero-server 2>/dev/null
sha256sum /opt/pero/languagetool/current/languagetool-server.jar 2>/dev/null
sha256sum /var/lib/pero/models/*.gguf 2>/dev/null
ls -l /opt/pero/llama.cpp /opt/pero/languagetool /opt/pero/server
sec "apt sources"
cat /etc/apt/sources.list 2>/dev/null | grep -v '^#' | grep .; for f in /etc/apt/sources.list.d/*; do [ -f "$f" ] && { echo "-- $f"; grep -v '^#' "$f" | grep .; }; done
sec "packages installed locally (not from a repository)"
apt list --installed 2>/dev/null | grep -F 'installed,local' || echo "none"
sec "packages requested explicitly (apt history; dependencies pulled in automatically are left out), oldest first"
{ for f in /var/log/apt/history.log.*.gz; do [ -f "$f" ] && zcat "$f"; done; cat /var/log/apt/history.log; } 2>/dev/null \
  | awk '/^Start-Date:/{d=$2} /^Install:/{n=split(substr($0,10),a,"), "); o=""; for(i=1;i<=n;i++) if(a[i] !~ /automatic/){x=a[i]; sub(/:amd64 \(/," ",x); sub(/\)$/,"",x); o=o x "; "} if(o!="") print d" "o}' | sort -s -k1,1
sec "telemetry check"
echo "popularity-contest: $(dpkg-query -W -f '${Status}' popularity-contest 2>/dev/null || echo not installed)"
grep -E '^PARTICIPATE' /etc/popularity-contest.conf 2>/dev/null || echo "no popularity-contest config"
sec "manually installed packages with version"
apt-mark showmanual | xargs dpkg-query -W -f '${Package} ${Version}\n' 2>/dev/null | sort
sec "pending updates"
apt-get -s upgrade 2>/dev/null | grep -c '^Inst' | sed 's/^/pending upgrades: /'
apt-get -s upgrade 2>/dev/null | grep '^Inst' | grep -i security | head -20
echo "unattended-upgrades: $(systemctl is-enabled unattended-upgrades 2>&1) $(systemctl is-active unattended-upgrades 2>&1)"
echo "reboot required: $([ -f /var/run/reboot-required ] && echo yes || echo no)"
sec "setuid binaries (baseline list)"
find / -xdev -perm -4000 -type f 2>/dev/null | sort
sec "kernel network settings"
sysctl net.ipv4.ip_forward net.ipv4.conf.all.accept_redirects net.ipv4.conf.all.rp_filter net.ipv6.conf.all.accept_ra 2>/dev/null
} > "$OUT" 2>&1
chmod 0644 "$OUT"; echo "Baseline facts written to $OUT ($(wc -l < "$OUT") lines)"
