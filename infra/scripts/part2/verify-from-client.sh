#!/usr/bin/env bash
# Part 2 checks, run from ANOTHER machine (no root needed).
# Usage: ./verify-from-client.sh <ssh-host-or-alias> [ssh-key]
set -uo pipefail
HOST="${1:?usage: $0 <ssh-host-or-alias> [ssh-key]}"
KEY="${2:-}"
KEYOPT=(); [ -n "$KEY" ] && KEYOPT=(-i "$KEY" -o IdentitiesOnly=yes)
O=(-o BatchMode=yes -o ConnectTimeout=6)
fail=0
ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; fail=1; }

ssh "${O[@]}" "${KEYOPT[@]}" "$HOST" true >/dev/null 2>&1 && ok "key login works" || bad "key login works"

out="$(ssh "${O[@]}" -o PreferredAuthentications=password -o PubkeyAuthentication=no "$HOST" true 2>&1)"
echo "$out" | grep -q 'Permission denied (publickey)' && ok "password login refused" || bad "password login refused ($out)"

out="$(ssh "${O[@]}" "${KEYOPT[@]}" -l root "$HOST" true 2>&1)"
echo "$out" | grep -q 'Permission denied' && ok "root login refused" || bad "root login refused ($out)"

ADDR="$(ssh -G "$HOST" | awk '$1=="hostname"{print $2}')"
for p in 22 9090; do
  timeout 4 bash -c "</dev/tcp/$ADDR/$p" 2>/dev/null && ok "port $p reachable from this machine" || bad "port $p reachable from this machine"
done
for p in 80 3000 5432 8080 11434; do
  timeout 4 bash -c "</dev/tcp/$ADDR/$p" 2>/dev/null && bad "port $p should be closed" || ok "port $p closed"
done

echo "---- server report ----"
ssh "${O[@]}" "${KEYOPT[@]}" "$HOST" 'cat /var/tmp/pero-part2-report.txt' 2>&1
exit $fail
