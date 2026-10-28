#!/usr/bin/env bash
# Part 8, step 5 (server, root): install the signed certificate and the server binary, and start the service pero-server.
# Reads server.crt, ca.crt and pero-server from ~/pero-part8 of the user who ran sudo ($SUDO_USER).
# Refuses to continue unless the certificate chains to the CA, matches the private key, names writer.local and is valid for 30+ days.
# Usage: sudo ./30-install-server.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OWNER="${SUDO_USER:?run through sudo so the bundle directory is known}"
SRC="/home/$OWNER/pero-part8"
T=/etc/pero/tls
for f in server.crt ca.crt pero-server; do [ -f "$SRC/$f" ] || { echo "Missing $SRC/$f" >&2; exit 1; }; done

# 1. certificate checks
openssl verify -CAfile "$SRC/ca.crt" "$SRC/server.crt"
[ "$(openssl x509 -in "$SRC/server.crt" -noout -pubkey)" = "$(openssl pkey -in $T/server.key -pubout)" ] || { echo "Certificate does not match the private key." >&2; exit 1; }
openssl x509 -in "$SRC/server.crt" -noout -ext subjectAltName | grep -q 'DNS:writer.local' || { echo "Certificate does not name writer.local." >&2; exit 1; }
openssl x509 -in "$SRC/server.crt" -noout -checkend $((30*24*3600)) >/dev/null || { echo "Certificate expires within 30 days." >&2; exit 1; }
install -m 0644 -o pero -g pero "$SRC/server.crt" $T/server.crt
install -m 0644 -o pero -g pero "$SRC/ca.crt" $T/ca.crt

# 2. binary: root-owned copy, run only as user pero
VER="$(sha256sum "$SRC/pero-server" | cut -c1-12)"
D="/opt/pero/server/$VER"
install -d -m 0755 -o root -g root /opt/pero /opt/pero/server "$D"
install -m 0755 -o root -g root "$SRC/pero-server" "$D/pero-server"
echo "binary sha256: $(sha256sum "$D/pero-server" | cut -d' ' -f1)"
ln -sfn "$D" /opt/pero/server/current
runuser -u pero -- /opt/pero/server/current/pero-server --version

# 3. settings (not secret)
cat > /etc/pero/server.toml <<'CFG'
bind = "0.0.0.0:8443"
tls_cert = "/etc/pero/tls/server.crt"
tls_key = "/etc/pero/tls/server.key"
llm = "127.0.0.1:8080"
languagetool = "127.0.0.1:8010"
CFG
chown root:pero /etc/pero/server.toml; chmod 0640 /etc/pero/server.toml

# 4. service
cat > /etc/systemd/system/pero-server.service <<'UNIT'
[Unit]
Description=Pero server (HTTPS on 8443, the only network-facing service)
After=network.target pero-llm.service pero-languagetool.service

[Service]
Type=simple
User=pero
Group=pero
ExecStart=/opt/pero/server/current/pero-server --config /etc/pero/server.toml
Restart=on-failure
RestartSec=3
UMask=0077
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
MemoryDenyWriteExecute=yes
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
CapabilityBoundingSet=
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
SystemCallArchitectures=native
SystemCallFilter=@system-service

[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable pero-server
systemctl restart pero-server
for i in $(seq 1 20); do curl -fsS -m 3 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1 https://writer.local:8443/health >/dev/null 2>&1 && break; sleep 1; done
curl -fsS -m 5 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1 https://writer.local:8443/health && echo
systemctl is-active pero-server
echo "Step 5 done."
