#!/usr/bin/env bash
# Part 8 (server, root): no-secrets status report for remote checking.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OUT=/var/tmp/pero-part8-report.txt
{ echo "== date"; date -Is; echo "== service"; systemctl is-enabled pero-server; systemctl is-active pero-server
  systemctl show pero-server -p User -p MainPID -p NRestarts -p ActiveEnterTimestamp
  echo "== every listening TCP socket (only 22, 9090, 8443 may be on a network address)"; ss -ltn | awk 'NR>1{print $4}' | sort -u
  echo "== firewall"; ufw status | grep -E '^(Status|22|8443|9090)'
  echo "== certificate"; openssl x509 -in /etc/pero/tls/server.crt -noout -subject -issuer -enddate -ext subjectAltName
  echo "== files"; ls -l /etc/pero/tls /etc/pero/server.toml /opt/pero/server/
  echo "== sandbox score"; systemd-analyze security pero-server --no-pager 2>/dev/null | tail -n 1
  echo "== memory"; free -m | sed -n 2p
} > "$OUT" 2>&1
chmod 0644 "$OUT"; echo "Report written to $OUT"
