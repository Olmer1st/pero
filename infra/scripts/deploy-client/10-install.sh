#!/usr/bin/env bash
# Client deploy, step 3 (server, root): installs the new server binary together with the built web client and turns the page on
# (`web_dir` in /etc/pero/server.toml), restarts, and checks /health, the page and that the API is still closed.
# Reads pero-server, web/ and web.sha256 from ~/pero-deploy-client of the user who ran sudo ($SUDO_USER); every file of the client
# is re-checked against web.sha256 first. Nothing about the data, the certificate or the unit changes.
# If the service does not come back healthy, turn 2 is put back (binary link and settings) and the script exits with an error.
# Usage: sudo ./10-install.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OWNER="${SUDO_USER:?run through sudo so the bundle directory is known}"
SRC="/home/$OWNER/pero-deploy-client"
T=/etc/pero/tls
CFG=/etc/pero/server.toml
for f in pero-server web.sha256; do [ -f "$SRC/$f" ] || { echo "Missing $SRC/$f" >&2; exit 1; }; done
[ -f "$SRC/web/index.html" ] || { echo "Missing $SRC/web/index.html" >&2; exit 1; }
grep -q '^data_dir' $CFG || { echo "Turn 2 is not installed (no data_dir in $CFG)." >&2; exit 1; }
( cd "$SRC/web" && sha256sum -c --quiet "$SRC/web.sha256" ) || { echo "The client files do not match web.sha256." >&2; exit 1; }
[ "$(cd "$SRC/web" && find . -type f | wc -l)" = "$(wc -l < "$SRC/web.sha256")" ] || { echo "Unexpected extra files in the client folder." >&2; exit 1; }
runuser -u pero -- true

BK=/var/lib/pero/turn2-backup
install -d -m 0700 -o root -g root "$BK"
cp -p $CFG "$BK/server.toml"                      # the state before this install, so a rollback goes back one step
readlink /opt/pero/server/current > "$BK/current"
rollback() {
  echo "Putting turn 2 back." >&2
  cp -p "$BK/server.toml" $CFG
  ln -sfn "$(cat "$BK/current")" /opt/pero/server/current
  systemctl restart pero-server || true
}

VER="$(sha256sum "$SRC/pero-server" | cut -c1-12)"
D="/opt/pero/server/$VER"
install -d -m 0755 -o root -g root /opt/pero /opt/pero/server "$D"
install -m 0755 -o root -g root "$SRC/pero-server" "$D/pero-server"
echo "binary sha256: $(sha256sum "$D/pero-server" | cut -d' ' -f1)"
runuser -u pero -- "$D/pero-server" --version
rm -rf "$D/web"
cp -r "$SRC/web" "$D/web"
chown -R root:root "$D/web"; find "$D/web" -type d -exec chmod 0755 {} +; find "$D/web" -type f -exec chmod 0644 {} +
echo "web files: $(find "$D/web" -type f | wc -l)"

grep -q '^web_dir' $CFG || echo 'web_dir = "/opt/pero/server/current/web"' >> $CFG
chown root:pero $CFG; chmod 0640 $CFG
ln -sfn "$D" /opt/pero/server/current
systemctl restart pero-server

C=(curl -sS -m 5 --cacert $T/ca.crt --resolve writer.local:8443:127.0.0.1)
ok=0
for i in $(seq 1 20); do
  if "${C[@]}" -fo /dev/null https://writer.local:8443/health 2>/dev/null; then ok=1; break; fi
  sleep 1
done
[ "$ok" -eq 1 ] || { echo "Service did not become healthy." >&2; journalctl -u pero-server -n 20 --no-pager >&2; rollback; exit 1; }
[ "$("${C[@]}" -o /dev/null -w '%{http_code}' https://writer.local:8443/)" = 200 ] || { echo "The page is not served." >&2; rollback; exit 1; }
[ "$("${C[@]}" -o /dev/null -w '%{http_code}' https://writer.local:8443/api/v1/books)" = 401 ] || { echo "The API is not closed." >&2; rollback; exit 1; }
systemctl is-active pero-server
journalctl -u pero-server -n 8 --no-pager | grep -iE 'web client|manuscript API' || true
echo "Install done. Turn 2 files are kept in $BK for rollback (30-rollback.sh)."
