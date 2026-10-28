#!/usr/bin/env bash
# Part 8, step 1 (dev laptop, no sudo): create a small private certificate authority for the home server.
# The CA key stays on this laptop (default ~/.pero-ca, mode 0700), never on the server and never in the repository.
# The CA is name-constrained: even if its key were stolen it could only vouch for writer.local and the home subnet,
# not for any other website. The owner imports ca.crt (public, safe to share) into the client once.
# Usage: ./05-make-ca.sh [home-subnet-address netmask]     default: 10.0.0.0 255.255.255.0
set -euo pipefail
CA_DIR="${PERO_CA_DIR:-$HOME/.pero-ca}"
NET="${1:-10.0.0.0}"; MASK="${2:-255.255.255.0}"
[ ! -e "$CA_DIR/ca.key" ] || { echo "CA already exists in $CA_DIR; not overwriting." >&2; exit 1; }
umask 077; mkdir -p "$CA_DIR"; cd "$CA_DIR"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out ca.key
cat > ca.cnf <<CNF
[req]
distinguished_name = dn
x509_extensions = v3_ca
prompt = no
[dn]
CN = Pero home CA
[v3_ca]
basicConstraints = critical, CA:TRUE, pathlen:0
keyUsage = critical, keyCertSign, cRLSign
subjectKeyIdentifier = hash
nameConstraints = critical, permitted;DNS:writer.local, permitted;IP:$NET/$MASK
CNF
openssl req -x509 -new -key ca.key -sha256 -days 1825 -config ca.cnf -out ca.crt
chmod 0644 ca.crt
openssl x509 -in ca.crt -noout -subject -enddate -fingerprint -sha256
echo "CA created in $CA_DIR. Keep ca.key private. Give only ca.crt to the client."
