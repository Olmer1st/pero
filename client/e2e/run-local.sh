#!/usr/bin/env bash
# Starts a throw-away local server (own temp CA, own data folder, the built client) and runs e2e.ts against it.
# Needs: the built client (npm run build), the server binary (cargo build -p pero-server), node on PATH.
# Usage: client/e2e/run-local.sh
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
BIN="${PERO_SERVER_BIN:-$ROOT/target/debug/pero-server}"
[ -x "$BIN" ] || { echo "Build the server first: cargo build -p pero-server" >&2; exit 1; }
[ -f "$ROOT/client/dist/index.html" ] || { echo "Build the client first: npm run build" >&2; exit 1; }
T="$(mktemp -d)"; trap 'kill $(cat "$T/pid" 2>/dev/null) 2>/dev/null; rm -rf "$T"' EXIT
PORT="${PERO_E2E_PORT:-18443}"
# throw-away CA and server certificate for 127.0.0.1 (nothing to do with the home CA)
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 1 -subj "/CN=pero-e2e-ca" \
  -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign" -keyout "$T/ca.key" -out "$T/ca.crt" 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj "/CN=localhost" -keyout "$T/server.key" -out "$T/server.csr" 2>/dev/null
printf 'subjectAltName=IP:127.0.0.1,DNS:localhost\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' > "$T/ext"
openssl x509 -req -in "$T/server.csr" -CA "$T/ca.crt" -CAkey "$T/ca.key" -CAcreateserial -days 1 -extfile "$T/ext" -out "$T/server.crt" 2>/dev/null
mkdir -p "$T/data"
cat > "$T/server.toml" <<CFG
bind = "127.0.0.1:$PORT"
tls_cert = "$T/server.crt"
tls_key = "$T/server.key"
llm = "127.0.0.1:1"
languagetool = "127.0.0.1:2"
data_dir = "$T/data"
web_dir = "$ROOT/client/dist"
CFG
TOKEN="$("$BIN" device add e2e --config "$T/server.toml" | grep -o 'pero_[0-9a-f]\{64\}')"
"$BIN" --config "$T/server.toml" 2> "$T/log" & echo $! > "$T/pid"
for _ in $(seq 1 50); do curl -s --cacert "$T/ca.crt" "https://127.0.0.1:$PORT/health" >/dev/null 2>&1 && break; sleep 0.2; done
cd "$ROOT/client"
if [ -n "${PERO_SERVE:-}" ]; then   # keep the server up for looking at the page by hand
  echo "server https://127.0.0.1:$PORT  token $TOKEN  (CA: $T/ca.crt)"; echo "ready"; wait "$(cat "$T/pid")"; exit 0
fi
NODE_EXTRA_CA_CERTS="$T/ca.crt" PERO_E2E_URL="https://127.0.0.1:$PORT" PERO_E2E_TOKEN="$TOKEN" \
  node --experimental-strip-types --no-warnings e2e/e2e.ts | tee "$T/e2e.out"
# the saves really are Git commits in the book's repository, authored by the device
REPO="$(ls -d "$T"/data/books/*/ | head -1)"
for c in $(grep '^COMMIT ' "$T/e2e.out" | cut -d' ' -f2); do
  git -C "$REPO" cat-file -e "$c" || { echo "FAIL: commit $c is not in the repository" >&2; exit 1; }
done
[ "$(git -C "$REPO" log --format=%an | sort -u | paste -sd,)" = "e2e" ] || { echo "FAIL: commits are not authored by the device" >&2; exit 1; }
echo "saves are Git commits authored by the device ($(git -C "$REPO" rev-list --count HEAD) commits in the book)"
grep -q "$TOKEN" "$T/log" && { echo "FAIL: token found in the server log" >&2; exit 1; } || echo "token not in the server log"
