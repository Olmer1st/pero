#!/usr/bin/env bash
# Part 8 checks from the dev laptop, at home. Needs the CA certificate (default ~/.pero-ca/ca.crt).
# Usage: ./verify-from-client.sh <ssh-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-alias>}"
CA="${PERO_CA_DIR:-$HOME/.pero-ca}/ca.crt"
ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; }
H="$(curl -sS -m 20 --cacert "$CA" https://writer.local:8443/health 2>&1)"
echo "$H" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("      ", d)' 2>/dev/null
echo "$H" | grep -q '"status":"ok"' && ok "health over HTTPS (certificate trusted via the home CA): status ok" || bad "health not ok: $H"
echo "$H" | grep -q '"llm":{"status":"up"' && echo "$H" | grep -q '"languagetool":{"status":"up"' && ok "both local services report up" || bad "a local service reports down"
curl -sS -m 10 https://writer.local:8443/health >/dev/null 2>&1 && bad "certificate is trusted without the home CA" || ok "refused without the home CA (not publicly trusted, as intended)"
curl -sS -m 10 --cacert "$CA" --tls-max 1.2 https://writer.local:8443/health >/dev/null 2>&1 && bad "TLS 1.2 accepted" || ok "TLS 1.2 refused (TLS 1.3 only)"
curl -sS -m 10 http://writer.local:8443/health >/dev/null 2>&1 && bad "plain HTTP answered on 8443" || ok "plain HTTP not served"
ADDR="$(ssh -G "$HOST" | awk '$1=="hostname"{print $2}')"
curl -sS -m 10 --cacert "$CA" "https://$ADDR:8443/health" >/dev/null 2>&1 && ok "also trusted by server address" || bad "address SAN mismatch"
systemctl_state="$(ssh "$HOST" 'systemctl is-active pero-server; systemctl is-enabled pero-server' | paste -sd' ')"
[ "$systemctl_state" = "active enabled" ] && ok "pero-server active and enabled at boot" || bad "pero-server state: $systemctl_state"
