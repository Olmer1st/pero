#!/usr/bin/env bash
# Part 6 checks from the dev laptop, at home. Usage: ./verify-from-client.sh <ssh-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-alias>}"
ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; }
ADDR="$(ssh -G "$HOST" | awk '$1=="hostname"{print $2}')"
# unreachable from another machine: the LLM port must not answer on the network address
curl -sS -m 5 -o /dev/null "http://$ADDR:8080/health" 2>/dev/null && bad "port 8080 answers on the network address" || ok "port 8080 not reachable from this machine"
ssh "$HOST" 'ss -ltn | awk "\$4 ~ /:8080\$/ {print \$4}"' | grep -qx '127.0.0.1:8080' && ok "listens on 127.0.0.1 only" || bad "bind address is not 127.0.0.1:8080"
ssh "$HOST" 'systemctl is-active pero-llm; systemctl is-enabled pero-llm' | paste -sd' ' | grep -qx 'active enabled' && ok "pero-llm active and enabled at boot" || bad "pero-llm not active/enabled"
ANS="$(ssh "$HOST" "curl -sS -m 300 http://127.0.0.1:8080/v1/chat/completions -H 'Content-Type: application/json' -d '{\"messages\":[{\"role\":\"user\",\"content\":\"Reply with one short sentence in Russian about the sea.\"}],\"max_tokens\":120}'" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d["choices"][0]["message"]["content"].strip())')"
[ -n "$ANS" ] && ok "completion works: $ANS" || bad "empty completion"
