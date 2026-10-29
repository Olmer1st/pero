#!/usr/bin/env bash
# Client deploy, step 4 (server, root): read-only checks of the running service. Changes nothing and touches no data.
# Usage: sudo ./20-smoke.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
T=/etc/pero/tls
U=https://writer.local:8443
C=(curl -sS -m 10 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1)
fail=0; ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; fail=1; }
code(){ "${C[@]}" -o /dev/null -w '%{http_code}' "$@"; }

H="$("${C[@]}" $U/health)"; grep -q '"status":"ok"' <<<"$H" && ok "health ok" || bad "health: $H"
HF="$(mktemp)"; trap 'rm -f "$HF"' EXIT
P="$("${C[@]}" -D "$HF" $U/)"
grep -q '<div id="app">' <<<"$P" && ok "the page is served at /" || bad "page body unexpected"
grep -qi "^content-security-policy: default-src 'none'; script-src 'self'" "$HF" && ok "Content-Security-Policy is set" || bad "no CSP"
grep -qi '^x-content-type-options: nosniff' "$HF" && ok "nosniff is set" || bad "no nosniff"
grep -qi '^cache-control: no-store' "$HF" && ok "the page is not cached" || bad "page may be cached"
JS="$(grep -o '/assets/[^"]*\.js' <<<"$P" | head -1)"
[ -n "$JS" ] && [ "$(code $U$JS)" = 200 ] && ok "script asset served ($JS)" || bad "script asset not served"
CSS="$(grep -o '/assets/[^"]*\.css' <<<"$P" | head -1)"
[ -n "$CSS" ] && [ "$(code $U$CSS)" = 200 ] && ok "style asset served" || bad "style asset not served"
for p in '/assets/..%2f..%2fetc%2fpasswd' '/assets/..%2fsecret' '/assets/.hidden.js' '/assets/' '/etc/passwd' '/index.html'; do
  c="$(code --path-as-is "$U$p")"; [[ "$c" =~ ^(404|401)$ ]] && ok "not served: $p ($c)" || bad "unexpected answer $c for $p"
done
[ "$(code $U/api/v1/books)" = 401 ] && ok "API without a token: 401" || bad "API open?"
[ "$(code $U/api/v1/nothing)" = 401 ] && ok "unknown API path: 401" || bad "unknown API path answered differently"
runuser -u pero -- sh -c 'test -w /opt/pero/server/current/web/index.html' && bad "the service can write to its web files" || ok "the service cannot change the web files"
[ "$(stat -c %U /opt/pero/server/current/web/index.html)" = root ] && ok "web files are root-owned" || bad "web files not root-owned"
journalctl -u pero-server -n 50 --no-pager | grep -q 'web client served' && ok "log says the web client is served" || bad "no web client line in the log"
[ "$fail" -eq 0 ] && echo "SMOKE TEST PASSED" || { echo "SMOKE TEST FAILED"; exit 1; }
