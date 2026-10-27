#!/usr/bin/env bash
# Part 7, step 1 (server, root): install LanguageTool 6.8 and run it as the systemd service pero-languagetool.
#  - source: Maven Central (the official release channel of the LanguageTool project), over HTTPS;
#  - SHA-256 pinned below (it equals the .sha256 file Maven Central publishes; the GPG signature by the LanguageTool
#    maintainer key C3DF B410 CA3A 9FAC 0550 8C4D 0AEE 3156 CB8E 4846 was also checked when the version was chosen);
#  - installed root-owned; the service runs as user "pero", on 127.0.0.1:8010 only.
# Usage: sudo ./10-install-languagetool.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
VER=6.8
ZIP="languagetool-standalone-$VER.zip"
URL="https://repo1.maven.org/maven2/org/languagetool/languagetool-standalone/$VER/$ZIP"
SHA=f04aecf37e35ef17d44b336da9668a5a3a871edd14bd83a766a7e110b9ebcd21
DEST="/opt/pero/languagetool/$VER"

if [ ! -f "$DEST/LanguageTool-$VER/languagetool-server.jar" ]; then
  T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
  curl --proto '=https' --tlsv1.2 -fsSL -o "$T/$ZIP" "$URL"
  echo "$SHA  $T/$ZIP" | sha256sum -c -
  install -d -m 0755 -o root -g root /opt/pero /opt/pero/languagetool "$DEST"
  python3 -m zipfile -e "$T/$ZIP" "$DEST"
  chown -R root:root "$DEST"; chmod -R go-w "$DEST"
fi
ln -sfn "$DEST/LanguageTool-$VER" /opt/pero/languagetool/current

cat > /etc/pero/languagetool.properties <<'PROPS'
maxTextLength=200000
maxCheckTimeMillis=120000
PROPS
chown root:pero /etc/pero/languagetool.properties; chmod 0640 /etc/pero/languagetool.properties

cat > /etc/systemd/system/pero-languagetool.service <<'UNIT'
[Unit]
Description=Pero LanguageTool (grammar and spelling checks, loopback only)
After=network.target

[Service]
Type=simple
User=pero
Group=pero
ExecStart=/usr/bin/java -Xms128m -Xmx2g -cp /opt/pero/languagetool/current/languagetool-server.jar org.languagetool.server.HTTPServer --port 8010 --config /etc/pero/languagetool.properties
Restart=on-failure
RestartSec=5
TimeoutStartSec=120
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
systemctl enable --now pero-languagetool
for i in $(seq 1 60); do curl -fsS -m 2 http://127.0.0.1:8010/v2/languages >/dev/null 2>&1 && break; sleep 2; done
curl -fsS -m 5 http://127.0.0.1:8010/v2/languages | python3 -c 'import json,sys; l=json.load(sys.stdin); print(len(l), "languages; ru:", any(x["code"]=="ru" for x in l), "en:", any(x["code"]=="en" for x in l))'
systemctl is-active pero-languagetool
echo "Step 1 done."
