#!/usr/bin/env bash
# Turn 2 deploy checks from the dev laptop, at home. Needs the CA certificate (default ~/.pero-ca/ca.crt).
# No token is used or needed. Usage: ./verify-from-client.sh <ssh-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-alias>}"
CA="${PERO_CA_DIR:-$HOME/.pero-ca}/ca.crt"
ADDR="$(getent ahostsv4 writer.local | awk '{print $1; exit}')"   # the alias may point to the name, not a number
[ -n "$ADDR" ] || { echo "writer.local does not resolve"; exit 1; }
fail=0; ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; fail=1; }
code(){ curl -sS -m 10 --cacert "$CA" -o /dev/null -w '%{http_code}' "$@" 2>/dev/null; }
H="$(curl -sS -m 20 --cacert "$CA" https://writer.local:8443/health 2>&1)"
echo "$H" | grep -q '"status":"ok"' && ok "health over HTTPS via writer.local" || bad "health not ok: $H"
curl -sS -m 10 --cacert "$CA" -o /dev/null "https://$ADDR:8443/health" && ok "certificate also valid for the server address" || bad "address does not match the certificate"
echo | openssl s_client -connect writer.local:8443 -servername writer.local -tls1_3 2>/dev/null | openssl x509 -noout -ext subjectAltName | grep -q "IP Address:$ADDR" && ok "certificate lists the current address" || bad "certificate does not list the current address"
[ "$(code https://writer.local:8443/api/v1/books)" = 401 ] && ok "API without a token: 401" || bad "API without a token did not give 401"
[ "$(code -H 'Authorization: Bearer pero_0000000000000000000000000000000000000000000000000000000000000000' https://writer.local:8443/api/v1/books)" = 401 ] && ok "API with a wrong token: 401" || bad "API with a wrong token did not give 401"
[ "$(code https://writer.local:8443/api/v1/nothing-here)" = 401 ] && ok "unknown API path: 401 (nothing is revealed)" || bad "unknown API path answered differently"
curl -sS -m 10 --cacert "$CA" --tls-max 1.2 https://writer.local:8443/health >/dev/null 2>&1 && bad "TLS 1.2 accepted" || ok "TLS 1.2 refused"
ssh -o BatchMode=yes "$HOST" 'sudo -n -l 2>&1' | grep -qi 'NOPASSWD' && bad "a passwordless sudo grant is still installed on the server" || ok "no passwordless sudo grant left on the server"
[ "$(ssh "$HOST" 'systemctl is-active pero-server; systemctl is-enabled pero-server' | paste -sd' ')" = "active enabled" ] && ok "pero-server active and enabled" || bad "pero-server state"
exit $fail
