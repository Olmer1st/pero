#!/usr/bin/env bash
# Part 3 checks from the dev laptop, at home (needs the step 2 listener running for the 8443 line).
# Usage: ./verify-from-client.sh <ssh-host-or-alias>
set -uo pipefail
HOST="${1:?usage: $0 <ssh-host-or-alias>}"
ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; }

ADDR="$(ssh -G "$HOST" | awk '$1=="hostname"{print $2}')"
RES="$(getent hosts writer.local | awk '{print $1; exit}')"
[ -n "$RES" ] && ok "writer.local resolves ($RES)" || bad "writer.local does not resolve"
[ "$RES" = "$ADDR" ] && ok "writer.local is the same address as the SSH alias" || bad "address differs from the SSH alias"

A="$(ssh-keyscan -t ed25519 "$ADDR" 2>/dev/null | awk '{print $3}')"
B="$(ssh-keyscan -t ed25519 writer.local 2>/dev/null | awk '{print $3}')"
[ -n "$A" ] && [ "$A" = "$B" ] && ok "same SSH host key via writer.local" || bad "SSH host key differs or unreachable via writer.local"

curl -sS -m 5 -o /dev/null -w '' "http://writer.local:8443/" 2>/dev/null \
  && ok "port 8443 reachable via writer.local (test listener)" || bad "port 8443 via writer.local (is the step 2 listener running?)"
