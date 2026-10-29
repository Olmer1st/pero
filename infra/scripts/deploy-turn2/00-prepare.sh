#!/usr/bin/env bash
# Turn 2 deploy, step 1 (dev laptop, no sudo, touches nothing on the server): builds the release binary, signs a new
# certificate for the reserved address, and assembles the bundle in target/deploy-turn2/:
#   bundle/   the scripts, sudo-allow.txt and SHA256SUMS (this is what grant-sudo.sh installs on the server)
#   payload/  pero-server, server.crt, ca.crt (what 10-install.sh installs)
# The new certificate re-uses the server's existing public key (read from the certificate the server already presents,
# so no access to the server's key or files is needed); the CA key stays on this laptop. Names in the certificate are
# fixed here: writer.local and the given address.
# Usage: ./00-prepare.sh <server-ip>
set -euo pipefail
IP="${1:?usage: $0 <server-ip>}"
[[ "$IP" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "server-ip must be an IPv4 address" >&2; exit 1; }
CA_DIR="${PERO_CA_DIR:-$HOME/.pero-ca}"
ROOT="$(git rev-parse --show-toplevel)"
HERE="$ROOT/infra/scripts/deploy-turn2"
OUT="$ROOT/target/deploy-turn2"
rm -rf "$OUT"; mkdir -p "$OUT/bundle" "$OUT/payload"

"$ROOT/infra/scripts/part8/25-build.sh"
cp "$ROOT/target/release/pero-server" "$OUT/payload/pero-server"

echo | openssl s_client -connect writer.local:8443 -servername writer.local -tls1_3 2>/dev/null | openssl x509 > "$OUT/current.crt"
openssl x509 -in "$OUT/current.crt" -noout -pubkey > "$OUT/server.pub"
EXT="$(mktemp)"; trap 'rm -f "$EXT"' EXIT
cat > "$EXT" <<CNF
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:writer.local, IP:$IP
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid
CNF
openssl x509 -new -force_pubkey "$OUT/server.pub" -subj "/CN=writer.local" -CA "$CA_DIR/ca.crt" -CAkey "$CA_DIR/ca.key" -CAcreateserial -days 365 -sha256 -extfile "$EXT" -out "$OUT/payload/server.crt"
openssl verify -CAfile "$CA_DIR/ca.crt" "$OUT/payload/server.crt"
[ "$(openssl x509 -in "$OUT/payload/server.crt" -noout -pubkey)" = "$(cat "$OUT/server.pub")" ] || { echo "Key mismatch." >&2; exit 1; }
cp "$CA_DIR/ca.crt" "$OUT/payload/ca.crt"
openssl x509 -in "$OUT/payload/server.crt" -noout -subject -issuer -enddate -ext subjectAltName

cp "$HERE"/10-install.sh "$HERE"/20-smoke.sh "$HERE"/30-rollback.sh "$OUT/bundle/"
sed "s|<server-ip>|$IP|" "$HERE/sudo-allow.txt.tmpl" > "$OUT/bundle/sudo-allow.txt"
(cd "$OUT/bundle" && sha256sum *.sh sudo-allow.txt > SHA256SUMS)
(cd "$OUT/payload" && sha256sum pero-server server.crt ca.crt)
echo "Bundle ready in $OUT (nothing was sent to the server)."
