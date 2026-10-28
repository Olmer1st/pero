#!/usr/bin/env bash
# Part 9 checks from the dev laptop, at home (no root needed): from the network only SSH, Pero (8443) and Cockpit (9090) may answer.
# Scans all 65535 TCP ports of the server (takes about 2 minutes). Usage: ./verify-from-client.sh <ssh-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-alias>}"
EXPECTED="22 8443 9090"
O=(-o BatchMode=yes -o ConnectTimeout=8)
fail=0; ok(){ echo "PASS  $1"; }; bad(){ echo "FAIL  $1"; fail=1; }
ADDR="$(ssh -G "$HOST" | awk '$1=="hostname"{print $2}')"

OPEN="$(python3 -I - "$ADDR" <<'PY'
import asyncio, sys
addr = sys.argv[1]
sem = asyncio.Semaphore(800)
async def probe(p):
    async with sem:
        try:
            r, w = await asyncio.wait_for(asyncio.open_connection(addr, p), 2.5)
            w.close()
            return p
        except Exception:
            return None
async def main():
    res = await asyncio.gather(*(probe(p) for p in range(1, 65536)))
    print(" ".join(str(p) for p in res if p))
asyncio.run(main())
PY
)"
[ "$OPEN" = "$EXPECTED" ] && ok "TCP ports answering from the network: $OPEN (only SSH, Pero and Cockpit)" || bad "TCP ports answering: '$OPEN' (expected: $EXPECTED)"

out="$(ssh "${O[@]}" -o PreferredAuthentications=password -o PubkeyAuthentication=no "$HOST" true 2>&1)"
echo "$out" | grep -q 'Permission denied (publickey)' && ok "SSH password login refused" || bad "SSH password login refused ($out)"
out="$(ssh "${O[@]}" -l root "$HOST" true 2>&1)"
echo "$out" | grep -q 'Permission denied' && ok "SSH root login refused" || bad "SSH root login refused ($out)"
ssh "${O[@]}" "$HOST" 'sudo -n -l 2>&1' | grep -q NOPASSWD && bad "a passwordless sudo grant is still installed" || ok "no passwordless sudo grant left"

echo "---- server facts ----"
ssh "${O[@]}" "$HOST" 'cat /var/tmp/pero-part9-baseline.txt' 2>&1
exit $fail
