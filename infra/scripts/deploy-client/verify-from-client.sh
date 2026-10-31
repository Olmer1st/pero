#!/usr/bin/env bash
# Client deploy checks from the dev laptop, at home. Needs the CA certificate (default ~/.pero-ca/ca.crt). No token is used.
# Usage: ./verify-from-client.sh <ssh-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-alias>}"
CA="${PERO_CA_DIR:-$HOME/.pero-ca}/ca.crt"
U=https://writer.local:8443
fail=0; ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; fail=1; }
code(){ curl -sS -m 10 --cacert "$CA" -o /dev/null -w '%{http_code}' "$@" 2>/dev/null; }
HF="$(mktemp)"; trap 'rm -f "$HF"' EXIT
P="$(curl -sS -m 10 --cacert "$CA" -D "$HF" $U/ 2>&1)"
grep -q '<div id="app">' <<<"$P" && ok "page served over HTTPS" || bad "page not served"
grep -qi "^content-security-policy:.*script-src 'self'" "$HF" && ok "Content-Security-Policy set" || bad "no CSP"
grep -qi '^x-frame-options: DENY' "$HF" && ok "framing refused" || bad "no X-Frame-Options"
JS="$(grep -o '/assets/[^"]*\.js' <<<"$P" | head -1)"
[ -n "$JS" ] && [ "$(code $U$JS)" = 200 ] && ok "script served" || bad "script not served"
[ "$(code $U/api/v1/books)" = 401 ] && ok "API without a token: 401" || bad "API not closed"
[ "$(code $U/api/v1/nothing)" = 401 ] && ok "unknown API path: 401" || bad "unknown API path answered differently"
curl -sS -m 10 --cacert "$CA" --tls-max 1.2 $U/ >/dev/null 2>&1 && bad "TLS 1.2 accepted" || ok "TLS 1.2 refused"
ssh -o BatchMode=yes "$HOST" 'sudo -n -l 2>&1' | grep -qi 'NOPASSWD' && bad "a passwordless sudo grant is still installed on the server" || ok "no passwordless sudo grant left on the server"
[ "$(ssh "$HOST" 'systemctl is-active pero-server pero-llm pero-languagetool' | paste -sd' ')" = "active active active" ] && ok "all three services active" || bad "a service is not active"
exit $fail
