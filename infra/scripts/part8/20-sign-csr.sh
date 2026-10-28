#!/usr/bin/env bash
# Part 8, step 3 (dev laptop, no sudo): sign the server's certificate request with the home CA.
# The names in the certificate are fixed here (writer.local and the server address); whatever the request asks for is ignored.
# Usage: ./20-sign-csr.sh <request.csr> <out.crt> <server-ip>
set -euo pipefail
CA_DIR="${PERO_CA_DIR:-$HOME/.pero-ca}"
CSR="${1:?usage: $0 <request.csr> <out.crt> <server-ip>}"; OUT="${2:?}"; IP="${3:?}"
[[ "$IP" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "server-ip must be an IPv4 address" >&2; exit 1; }
openssl req -in "$CSR" -noout -verify 2>&1 | grep -q 'verify OK' || { echo "Request signature is not valid." >&2; exit 1; }
EXT="$(mktemp)"; trap 'rm -f "$EXT"' EXIT
cat > "$EXT" <<CNF
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:writer.local, IP:$IP
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid
CNF
openssl x509 -req -in "$CSR" -CA "$CA_DIR/ca.crt" -CAkey "$CA_DIR/ca.key" -CAcreateserial -days 365 -sha256 -extfile "$EXT" -out "$OUT"
openssl verify -CAfile "$CA_DIR/ca.crt" "$OUT"
openssl x509 -in "$OUT" -noout -subject -issuer -enddate -ext subjectAltName
