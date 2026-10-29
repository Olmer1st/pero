#!/usr/bin/env bash
# Client deploy, optional step (server, root): adds ONE sample "AI hypothesis" fact to the owner's book so the Canon screen has
# something to approve (real hypotheses come from the AI features of later turns). It registers a throw-away device
# "seed-sample-<random>" (its token lives only in this script's memory: never printed, stored or logged), reads the first paragraph of the
# first scene, adds the fact citing it, then revokes the device and checks the token is refused.
# It stops without changing anything unless the data folder holds exactly one book and no "Sample hypothesis" fact exists yet.
# The fact can be rejected in the page afterwards, but facts are never deleted (the ledger keeps its history).
# Usage: sudo ./25-seed-sample.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
NAME="seed-sample-$(head -c 4 /dev/urandom | od -An -tx1 | tr -d ' \n')"   # revoked names are never reused, so each run needs its own
T=/etc/pero/tls
BIN=/opt/pero/server/current/pero-server
CFG=/etc/pero/server.toml
URL=https://writer.local:8443/api/v1
C=(curl -sS -m 20 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1)
first(){ python3 -I -c 'import json,sys; d=json.load(sys.stdin); print(d[0]["id"] if d else "")'; }

OUT="$(runuser -u pero -- $BIN device add "$NAME" --config $CFG)"
TOKEN="$(grep -o 'pero_[0-9a-f]\{64\}' <<<"$OUT")"; [ -n "$TOKEN" ] || { echo "No token produced." >&2; exit 1; }
cleanup(){ runuser -u pero -- $BIN device revoke "$NAME" --config $CFG >/dev/null 2>&1 || true; unset TOKEN; }
trap cleanup EXIT
A=(-H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json')

BOOKS="$("${C[@]}" "${A[@]}" $URL/books)"
[ "$(python3 -I -c 'import json,sys; print(len(json.load(sys.stdin)))' <<<"$BOOKS")" = 1 ] || { echo "Expected exactly one book; nothing was changed." >&2; exit 1; }
BOOK="$(first <<<"$BOOKS")"
[[ "$BOOK" =~ ^bk_[0-9a-z]{8}$ ]] || { echo "Unexpected book id." >&2; exit 1; }
if "${C[@]}" "${A[@]}" $URL/books/$BOOK/facts | grep -q 'Sample hypothesis'; then echo "A sample hypothesis already exists; nothing was changed."; exit 0; fi
CH="$("${C[@]}" "${A[@]}" $URL/books/$BOOK/chapters | first)"
SC="$("${C[@]}" "${A[@]}" $URL/chapters/$CH/scenes | first)"
PA="$("${C[@]}" "${A[@]}" $URL/scenes/$SC | python3 -I -c 'import json,sys; print(next(p["id"] for p in json.load(sys.stdin)["passages"] if p["id"]))')"
F="$("${C[@]}" "${A[@]}" -X POST $URL/books/$BOOK/facts -d "{\"text\":\"Sample hypothesis: something stated in the first paragraph\",\"origin\":\"ai\",\"source_ref\":\"sample\",\"evidence\":[{\"passage_id\":\"$PA\"}]}")"
python3 -I -c 'import json,sys; f=json.loads(sys.argv[1]); print("Added fact", f["id"], "status", f["status"])' "$F"

runuser -u pero -- $BIN device revoke "$NAME" --config $CFG >/dev/null
[ "$("${C[@]}" -o /dev/null -w '%{http_code}' "${A[@]}" $URL/books)" = 401 ] && echo "The temporary device is revoked; its token is refused." || { echo "FAIL: token still accepted" >&2; exit 1; }
journalctl -u pero-server -n 100 --no-pager > /tmp/pero-seed-log.$$ 2>&1 || true
grep -q "$TOKEN" /tmp/pero-seed-log.$$ && { rm -f /tmp/pero-seed-log.$$; echo "FAIL: token found in the log" >&2; exit 1; }
rm -f /tmp/pero-seed-log.$$
echo "SEED DONE"
