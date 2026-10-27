#!/usr/bin/env bash
# Part 7 (server, root): no-secrets status report for remote checking.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OUT=/var/tmp/pero-part7-report.txt
{ echo "== date"; date -Is; echo "== service"; systemctl is-enabled pero-languagetool; systemctl is-active pero-languagetool
  systemctl show pero-languagetool -p User -p MainPID -p NRestarts -p ActiveEnterTimestamp
  echo "== listening on 8010"; ss -ltnp | grep ':8010' | sed 's/users:.*//'
  echo "== firewall (no 8010 rule expected)"; ufw status | grep -E '^(Status|8010)'
  echo "== files"; ls -l /opt/pero/languagetool/ /etc/pero/languagetool.properties
  echo "== sandbox score"; systemd-analyze security pero-languagetool --no-pager 2>/dev/null | tail -n 1
  echo "== memory"; free -m | sed -n 2p
  echo "== log lines containing the seeded test words (must be 0)"; journalctl -u pero-languagetool --no-pager | grep -c -E 'пошол|has a apple' || true
} > "$OUT" 2>&1
chmod 0644 "$OUT"; echo "Report written to $OUT"
