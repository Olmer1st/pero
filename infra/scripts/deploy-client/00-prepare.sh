#!/usr/bin/env bash
# Client deploy, step 1 (dev laptop, no sudo, touches nothing on the server): checks and builds the web client, builds the
# release server, and assembles target/deploy-client/:
#   bundle/   the scripts, sudo-allow.txt and SHA256SUMS (what grant-sudo.sh installs on the server)
#   payload/  pero-server, web/ (the built client) and web.sha256 (what 10-install.sh installs and re-checks)
# Needs Node 22 on PATH (see client/README.md). Usage: ./00-prepare.sh
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
ROOT="$(git rev-parse --show-toplevel)"
HERE="$ROOT/infra/scripts/deploy-client"
OUT="$ROOT/target/deploy-client"
rm -rf "$OUT"; mkdir -p "$OUT/bundle" "$OUT/payload"

( cd "$ROOT/client" && npm ci && npm run check && npm test && npm run build && npm audit --omit=dev )
"$ROOT/infra/scripts/part8/25-build.sh"
cp "$ROOT/target/release/pero-server" "$OUT/payload/pero-server"
cp -r "$ROOT/client/dist" "$OUT/payload/web"
[ -f "$OUT/payload/web/index.html" ] || { echo "The client build has no index.html." >&2; exit 1; }
( cd "$OUT/payload/web" && find . -type f | sort | xargs sha256sum ) > "$OUT/payload/web.sha256"

cp "$HERE"/10-install.sh "$HERE"/20-smoke.sh "$HERE"/30-rollback.sh "$OUT/bundle/"
cp "$HERE/sudo-allow.txt.tmpl" "$OUT/bundle/sudo-allow.txt"
( cd "$OUT/bundle" && sha256sum *.sh sudo-allow.txt > SHA256SUMS )
( cd "$OUT/payload" && sha256sum pero-server web.sha256 )
echo "Bundle ready in $OUT (nothing was sent to the server)."
