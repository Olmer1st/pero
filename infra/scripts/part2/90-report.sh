#!/usr/bin/env bash
# Part 2: writes a world-readable status report (no secrets) so it can be checked remotely.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OUT=/var/tmp/pero-part2-report.txt
{
  echo "== date";          date -Is
  echo "== updates";       apt list --upgradable 2>/dev/null | tail -n +2 | wc -l
  systemctl is-enabled unattended-upgrades; cat /etc/apt/apt.conf.d/20auto-upgrades
  echo "== service user";  getent passwd pero; passwd -S pero
  stat -c '%U:%G %a %n' /var/lib/pero /etc/pero
  echo "== sshd";         sshd -T | grep -E '^(permitrootlogin|passwordauthentication|kbdinteractiveauthentication|pubkeyauthentication|x11forwarding|maxauthtries) '
  echo "== ufw";          ufw status verbose
  echo "== rollback timer"; systemctl list-timers pero-ufw-rollback.timer --no-legend || true
  echo "== logind";       cat /etc/systemd/logind.conf.d/10-pero-lid.conf
  systemctl is-enabled sleep.target suspend.target hibernate.target hybrid-sleep.target || true
  echo "== listening";    ss -ltnu
} > "$OUT" 2>&1
chmod 0644 "$OUT"
echo "Report written to $OUT"
