#!/usr/bin/env bash
# Turn 2 deploy, step 4 (server, root): smoke test of the real service, then wipe the test data.
# It registers a throw-away device "smoke-test" (its token lives only in this script's memory, never printed or stored),
# then through HTTPS on this machine: checks that the API refuses requests without a token, creates a book, a chapter and a
# scene with Russian text, saves it, reads it back, exports it, and checks that Git inside the service sandbox made commits
# authored "smoke-test". It revokes the device, checks the token is refused, and removes the test data (database and the one
# test book folder), so the data folder is empty for real use.
# It REFUSES to run unless the data folder is empty (no database yet, no books): it never touches real data.
# Usage: sudo ./20-smoke.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
DATA=/var/lib/pero/data
T=/etc/pero/tls
BIN=/opt/pero/server/current/pero-server
CFG=/etc/pero/server.toml
URL=https://writer.local:8443/api/v1
C=(curl -sS -m 20 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1)
fail=0; ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; fail=1; }

[ -d "$DATA" ] || { echo "No data folder; run 10-install.sh first." >&2; exit 1; }
[ -z "$(ls -A "$DATA/books" 2>/dev/null)" ] || { echo "The data folder already has books; refusing to run (it would leave test data)." >&2; exit 1; }
if [ -f "$DATA/pero.db" ]; then
  [ "$(runuser -u pero -- $BIN device list --config $CFG)" = "no devices registered" ] || { echo "Devices are already registered; refusing to run on a folder in use." >&2; exit 1; }
fi

code(){ "${C[@]}" -o /dev/null -w '%{http_code}' "$@"; }
[ "$(code $URL/books)" = 401 ] && ok "no token: 401" || bad "no token did not give 401"
[ "$(code -H 'Authorization: Bearer pero_0000000000000000000000000000000000000000000000000000000000000000' $URL/books)" = 401 ] && ok "wrong token: 401" || bad "wrong token did not give 401"

OUT="$(runuser -u pero -- $BIN device add smoke-test --config $CFG)"
TOKEN="$(grep -o 'pero_[0-9a-f]\{64\}' <<<"$OUT")"; [ -n "$TOKEN" ] || { echo "No token produced." >&2; exit 1; }
A=(-H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json')
id_of(){ grep -o '"id":"[a-z]\{2\}_[0-9a-z]\{8\}"' | head -1 | cut -d'"' -f4; }

BOOK="$("${C[@]}" "${A[@]}" -X POST $URL/books -d '{"title":"Smoke test","language":"ru"}' | id_of)"
[[ "$BOOK" =~ ^bk_[0-9a-z]{8}$ ]] && ok "book created" || { bad "book not created ($BOOK)"; BOOK=""; }
if [ -n "$BOOK" ]; then
  CH="$("${C[@]}" "${A[@]}" -X POST $URL/books/$BOOK/chapters -d '{"title":"Глава 1"}' | id_of)"
  SC="$("${C[@]}" "${A[@]}" -X POST $URL/chapters/$CH/scenes -d '{"title":"Сцена"}' | id_of)"
  [[ "$CH" =~ ^ch_ && "$SC" =~ ^sc_ ]] && ok "chapter and scene created" || bad "chapter or scene not created"
  REV="$("${C[@]}" "${A[@]}" $URL/scenes/$SC | grep -o '"revision":"[0-9a-f]*"' | cut -d'"' -f4)"
  S="$("${C[@]}" "${A[@]}" -X PUT $URL/scenes/$SC -d "{\"base_revision\":\"$REV\",\"fields\":{\"title\":\"Сцена\"},\"passages\":[{\"text\":\"Съешь ещё этих мягких французских булок.\"},{\"text\":\"— Ёлка, — сказала она.\"}]}")"
  grep -q '"commit"' <<<"$S" && ok "scene saved (a commit was made)" || bad "save failed: $(head -c 200 <<<"$S")"
  "${C[@]}" "${A[@]}" $URL/scenes/$SC | grep -q 'Съешь ещё этих' && ok "Russian text read back intact" || bad "text did not come back"
  "${C[@]}" "${A[@]}" $URL/books/$BOOK/export | grep -q 'Ёлка' && ok "export contains the text" || bad "export failed"
  REPO="$DATA/books/$BOOK"
  AUTH="$(runuser -u pero -- git -C "$REPO" log --format=%an 2>/dev/null | sort -u | paste -sd,)"
  [ "$AUTH" = "smoke-test" ] && ok "Git commits in the sandbox are authored by the device" || bad "git authors: '$AUTH'"
  [ -z "$(runuser -u pero -- git -C "$REPO" status --porcelain)" ] && ok "working tree clean after the save" || bad "working tree not clean"
fi

runuser -u pero -- $BIN device revoke smoke-test --config $CFG >/dev/null
[ "$(code "${A[@]}" $URL/books)" = 401 ] && ok "revoked token refused at once" || bad "revoked token still accepted"
journalctl -u pero-server -n 100 --no-pager | grep -q "$TOKEN" && bad "token found in the log" || ok "token not in the log"

# remove the test data (only the exact files and the one book this script created)
if [ "$fail" -eq 0 ] || [ -n "$BOOK" ]; then
  systemctl stop pero-server
  [[ "$BOOK" =~ ^bk_[0-9a-z]{8}$ ]] && rm -rf -- "$DATA/books/$BOOK"
  rm -f -- "$DATA/pero.db" "$DATA/pero.db-wal" "$DATA/pero.db-shm"
  systemctl start pero-server
fi
unset TOKEN
echo "Test data removed; the service recreated an empty database."
[ "$fail" -eq 0 ] && echo "SMOKE TEST PASSED" || { echo "SMOKE TEST FAILED"; exit 1; }
