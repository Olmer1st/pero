#!/usr/bin/env bash
# Adds ONE sample "AI hypothesis" fact to a book, so the Canon screen has something to approve. Real hypotheses come from
# the AI features in later turns; until then this is the only way to see the approval flow. It reads the first paragraph of
# the first scene and cites it as evidence for the fact "Sample hypothesis: ..." (reject it afterwards if you like).
# The device token is asked for without echo and is never written to a file or the shell history.
# Usage: seed-hypothesis.sh <server-url> <book-id> [ca-file]      e.g.  seed-hypothesis.sh https://writer.local:8443 bk_xxxxxxxx
set -euo pipefail
URL="${1:?usage: $0 <server-url> <book-id> [ca-file]}"; BOOK="${2:?}"; CA="${3:-$HOME/.pero-ca/ca.crt}"
[[ "$BOOK" =~ ^bk_[0-9a-z]{8}$ ]] || { echo "book id looks like bk_xxxxxxxx" >&2; exit 1; }
read -rsp "Device token: " TOKEN; echo
[[ "$TOKEN" =~ ^pero_[0-9a-f]{64}$ ]] || { echo "That is not a device token." >&2; exit 1; }
api() { curl -sS --fail-with-body -m 20 --cacert "$CA" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' "$@"; }
CH="$(api "$URL/api/v1/books/$BOOK/chapters" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)[0]["id"])')"
SC="$(api "$URL/api/v1/chapters/$CH/scenes" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)[0]["id"])')"
PA="$(api "$URL/api/v1/scenes/$SC" | python3 -I -c 'import json,sys; print(next(p["id"] for p in json.load(sys.stdin)["passages"] if p["id"]))')"
api -X POST "$URL/api/v1/books/$BOOK/facts" -d "{\"text\":\"Sample hypothesis: something stated in the first paragraph\",\"origin\":\"ai\",\"source_ref\":\"sample\",\"evidence\":[{\"passage_id\":\"$PA\"}]}" | python3 -I -c 'import json,sys; f=json.load(sys.stdin); print("Added:", f["id"], f["status"])'
