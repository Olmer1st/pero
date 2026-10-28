#!/usr/bin/env bash
# Part 8, step 2 (server, root): create the server's private key (it never leaves the server) and a certificate request.
# The key is only generated if it does not exist yet. The request is written to /var/tmp/pero-server.csr (public) for signing.
# Usage: sudo ./10-tls-csr.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
install -d -m 0700 -o pero -g pero /etc/pero/tls
if [ ! -f /etc/pero/tls/server.key ]; then
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out /etc/pero/tls/server.key
fi
chown pero:pero /etc/pero/tls/server.key; chmod 0600 /etc/pero/tls/server.key
openssl req -new -key /etc/pero/tls/server.key -subj "/CN=writer.local" -out /var/tmp/pero-server.csr
chmod 0644 /var/tmp/pero-server.csr
openssl req -in /var/tmp/pero-server.csr -noout -subject
echo "Step 2 done: request at /var/tmp/pero-server.csr"
