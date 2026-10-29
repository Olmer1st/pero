#!/usr/bin/env bash
# Turn 2 deploy, step 3 (server, root): install the new binary and the certificate for the reserved address, create the data
# folder, add `data_dir` to the settings and `ReadWritePaths` to the unit, restart, and check /health.
# Reads server.crt, ca.crt and pero-server from ~/pero-deploy-turn2 of the user who ran sudo ($SUDO_USER).
# Refuses to continue unless the certificate chains to the CA, matches the private key, names writer.local and the given
# address, and is valid for 30+ days. If the service does not come back healthy, everything is put back (old binary, settings,
# unit) and the script exits with an error.
# Usage: sudo ./10-install.sh <server-ip>
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
IP="${1:?usage: sudo $0 <server-ip>}"
[[ "$IP" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "server-ip must be an IPv4 address" >&2; exit 1; }
OWNER="${SUDO_USER:?run through sudo so the bundle directory is known}"
SRC="/home/$OWNER/pero-deploy-turn2"
T=/etc/pero/tls
DATA=/var/lib/pero/data
UNIT=/etc/systemd/system/pero-server.service
for f in server.crt ca.crt pero-server; do [ -f "$SRC/$f" ] || { echo "Missing $SRC/$f" >&2; exit 1; }; done

# 1. certificate checks (nothing is changed until all pass)
openssl verify -CAfile "$SRC/ca.crt" "$SRC/server.crt"
[ "$(openssl x509 -in "$SRC/server.crt" -noout -pubkey)" = "$(openssl pkey -in $T/server.key -pubout)" ] || { echo "Certificate does not match the private key." >&2; exit 1; }
SAN="$(openssl x509 -in "$SRC/server.crt" -noout -ext subjectAltName)"
grep -q 'DNS:writer.local' <<<"$SAN" || { echo "Certificate does not name writer.local." >&2; exit 1; }
grep -q "IP Address:$IP\b" <<<"$SAN" || { echo "Certificate does not name $IP." >&2; exit 1; }
openssl x509 -in "$SRC/server.crt" -noout -checkend $((30*24*3600)) >/dev/null || { echo "Certificate expires within 30 days." >&2; exit 1; }
runuser -u pero -- true   # the service user exists

# 2. remember what to put back
BK="/var/lib/pero/turn1-backup"
install -d -m 0700 -o root -g root "$BK"
[ -f "$BK/pero-server.service" ] || cp -p "$UNIT" "$BK/pero-server.service"
[ -f "$BK/server.toml" ] || cp -p /etc/pero/server.toml "$BK/server.toml"
[ -f "$BK/server.crt" ] || cp -p $T/server.crt "$BK/server.crt"
[ -f "$BK/current" ] || readlink /opt/pero/server/current > "$BK/current"

rollback() {
  echo "Putting the turn 1 setup back." >&2
  cp -p "$BK/pero-server.service" "$UNIT"; cp -p "$BK/server.toml" /etc/pero/server.toml
  cp -p "$BK/server.crt" $T/server.crt
  ln -sfn "$(cat "$BK/current")" /opt/pero/server/current
  systemctl daemon-reload; systemctl restart pero-server || true
}

# 3. binary: root-owned copy, run only as user pero
VER="$(sha256sum "$SRC/pero-server" | cut -c1-12)"
D="/opt/pero/server/$VER"
install -d -m 0755 -o root -g root /opt/pero /opt/pero/server "$D"
install -m 0755 -o root -g root "$SRC/pero-server" "$D/pero-server"
echo "binary sha256: $(sha256sum "$D/pero-server" | cut -d' ' -f1)"
runuser -u pero -- "$D/pero-server" --version

# 4. data folder (SQLite file and one Git repository per book); only the service user can read it
install -d -m 0700 -o pero -g pero /var/lib/pero "$DATA"

# 5. certificate, settings, unit
install -m 0644 -o pero -g pero "$SRC/server.crt" $T/server.crt
install -m 0644 -o pero -g pero "$SRC/ca.crt" $T/ca.crt
cat > /etc/pero/server.toml <<CFG
bind = "0.0.0.0:8443"
tls_cert = "/etc/pero/tls/server.crt"
tls_key = "/etc/pero/tls/server.key"
llm = "127.0.0.1:8080"
languagetool = "127.0.0.1:8010"
data_dir = "$DATA"
CFG
chown root:pero /etc/pero/server.toml; chmod 0640 /etc/pero/server.toml
# same unit as turn 1, plus one writable path (ProtectSystem=strict keeps everything else read-only)
sed -e '/^ProtectHome=yes$/a ReadWritePaths='"$DATA" "$BK/pero-server.service" > "$UNIT"
grep -q "^ReadWritePaths=$DATA\$" "$UNIT" || { echo "Unit edit failed." >&2; rollback; exit 1; }
ln -sfn "$D" /opt/pero/server/current
systemctl daemon-reload
systemctl restart pero-server

# 6. health over HTTPS with the new certificate; roll back if it does not come up
ok=0
for i in $(seq 1 20); do
  if curl -fsS -m 3 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1 https://writer.local:8443/health >/dev/null 2>&1; then ok=1; break; fi
  sleep 1
done
if [ "$ok" -ne 1 ]; then echo "Service did not become healthy." >&2; journalctl -u pero-server -n 20 --no-pager >&2; rollback; exit 1; fi
curl -fsS -m 5 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1 https://writer.local:8443/health && echo
systemctl is-active pero-server
journalctl -u pero-server -n 5 --no-pager | grep -i 'device' || true
echo "Install done. Turn 1 files are kept in $BK for rollback (30-rollback.sh)."
