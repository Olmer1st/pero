#!/usr/bin/env bash
# Part 7 checks from the dev laptop, at home. Usage: ./verify-from-client.sh <ssh-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-alias>}"
ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; }
ADDR="$(ssh -G "$HOST" | awk '$1=="hostname"{print $2}')"
curl -sS -m 5 -o /dev/null "http://$ADDR:8010/v2/languages" 2>/dev/null && bad "port 8010 answers on the network address" || ok "port 8010 not reachable from this machine"
ssh "$HOST" 'ss -ltn | awk "\$4 ~ /:8010\$/ {print \$4}"' | grep -qE '^(127\.0\.0\.1|\[::ffff:127\.0\.0\.1\]):8010$' && ok "listens on loopback only" || bad "bind address is not loopback"
ssh "$HOST" 'systemctl is-active pero-languagetool; systemctl is-enabled pero-languagetool' | paste -sd' ' | grep -qx 'active enabled' && ok "pero-languagetool active and enabled at boot" || bad "pero-languagetool not active/enabled"
chk(){ ssh "$HOST" "curl -s -m 120 http://127.0.0.1:8010/v2/check --data-urlencode 'language=$1' --data-urlencode 'text=$2'" \
  | python3 -c 'import json,sys; d=json.load(sys.stdin); print(" ".join(m["rule"]["id"] for m in d["matches"]))'; }
EN="$(chk en-US 'I has a apple and he go home yesterday.')"
echo "$EN" | grep -q 'BASE_FORM' && echo "$EN" | grep -q 'EN_A_VS_AN' && ok "English seeded errors found: $EN" || bad "English seeded errors not found: $EN"
RU="$(chk ru-RU 'Я пошол в магазин и купил хлеб.')"
echo "$RU" | grep -q 'MORFOLOGIK_RULE_RU_RU' && ok "Russian seeded error found: $RU" || bad "Russian seeded error not found: $RU"
