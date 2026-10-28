#!/usr/bin/env bash
# Part 6 (server, root): no-secrets status report for remote checking. Includes where the service listens and its sandbox score.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OUT=/var/tmp/pero-part6-report.txt
{ echo "== date"; date -Is; echo "== service"; systemctl is-enabled pero-llm; systemctl is-active pero-llm
  systemctl show pero-llm -p User -p MainPID -p NRestarts -p ActiveEnterTimestamp
  echo "== listening on 8080"; ss -ltnp | grep ':8080' | sed 's/users:.*//'
  echo "== firewall"; ufw status | grep -E '^(Status|8080)'
  echo "== files"; ls -l /opt/pero/llama.cpp/ /var/lib/pero/models /etc/pero/llm.env
  echo "== sandbox score"; systemd-analyze security pero-llm --no-pager 2>/dev/null | tail -n 3
  echo "== memory"; free -m | sed -n 2p
  echo "== recent log lines mentioning the request text (must be 0)"; journalctl -u pero-llm --no-pager | grep -c "Say hello" || true
} > "$OUT" 2>&1
chmod 0644 "$OUT"; echo "Report written to $OUT"
