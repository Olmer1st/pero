#!/usr/bin/env bash
# Part 6, step 1 (server, root): install the llama.cpp server and the chosen model, and run them as the systemd service pero-llm.
#  - binaries are copied to a root-owned directory (the service user cannot modify them);
#  - the model is copied to /var/lib/pero/models, its SHA-256 re-checked, and the benchmark copy removed;
#  - the service runs as user "pero", listens on 127.0.0.1:8080 only, no web UI, thinking off by default.
# Usage: sudo ./10-install-llm.sh      (needs sudo, not a root login: it reads the owner's build via $SUDO_USER)
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OWNER="${SUDO_USER:?run through sudo so the build directory of the owner is known}"
SRC="/home/$OWNER/pero-bench"
TAG=b11796; COMMIT=0a1ef30
MODEL=gemma-4-26B-A4B-it-UD-Q4_K_M.gguf
MODEL_SHA=f2c28b3dc4776931ac6f879e11f203dec637ea0f14267a86ec8f6165f63f293f
BIN="/opt/pero/llama.cpp/$TAG/bin"; MDIR=/var/lib/pero/models

[ -x "$SRC/llama.cpp/build/bin/llama-server" ] && [ -f "$SRC/models/$MODEL" ] || [ -f "$MDIR/$MODEL" ] || { echo "Build or model not found under $SRC" >&2; exit 1; }

# 1. binaries (only if not already installed), then run nothing from the owner's directory as root
if [ ! -x "$BIN/llama-server" ]; then
  install -d -m 0755 -o root -g root /opt/pero /opt/pero/llama.cpp "$BIN"
  install -m 0755 -o root -g root "$SRC/llama.cpp/build/bin/llama-server" "$SRC/llama.cpp/build/bin/llama-bench" "$BIN/"
  for f in "$SRC"/llama.cpp/build/bin/*.so*; do cp -P "$f" "$BIN/"; done
  chown -R root:root "$BIN"; chmod -R go-w "$BIN"
fi
ln -sfn "/opt/pero/llama.cpp/$TAG" /opt/pero/llama.cpp/current
runuser -u pero -- env LD_LIBRARY_PATH="$BIN" "$BIN/llama-server" --version 2>&1 | grep -q "commit $COMMIT" || { echo "Installed llama-server is not commit $COMMIT" >&2; exit 1; }

# 2. model
install -d -m 0750 -o pero -g pero "$MDIR"
if [ ! -f "$MDIR/$MODEL" ]; then
  install -m 0640 -o pero -g pero "$SRC/models/$MODEL" "$MDIR/$MODEL"
fi
echo "$MODEL_SHA  $MDIR/$MODEL" | sha256sum -c -
[ -f "$SRC/models/$MODEL" ] && rm -v -- "$SRC/models/$MODEL"

# 3. settings (not secret). Change the model or context here, then: systemctl restart pero-llm
cat > /etc/pero/llm.env <<ENV
LLM_MODEL=$MDIR/$MODEL
LLM_PORT=8080
LLM_THREADS=8
LLM_CTX=16384
ENV
chown root:pero /etc/pero/llm.env; chmod 0640 /etc/pero/llm.env

# 4. the service
cat > /etc/systemd/system/pero-llm.service <<'UNIT'
[Unit]
Description=Pero local LLM (llama.cpp server, loopback only)
After=network.target

[Service]
Type=simple
User=pero
Group=pero
EnvironmentFile=/etc/pero/llm.env
Environment=LD_LIBRARY_PATH=/opt/pero/llama.cpp/current/bin
ExecStart=/opt/pero/llama.cpp/current/bin/llama-server -m ${LLM_MODEL} --host 127.0.0.1 --port ${LLM_PORT} -t ${LLM_THREADS} -c ${LLM_CTX} --parallel 1 --no-webui --jinja --reasoning off
Restart=on-failure
RestartSec=5
TimeoutStartSec=300
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
ReadOnlyPaths=/var/lib/pero/models

[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable --now pero-llm
for i in $(seq 1 150); do curl -fsS -m 2 http://127.0.0.1:8080/health >/dev/null 2>&1 && break; sleep 2; done
curl -fsS -m 5 http://127.0.0.1:8080/health && echo
systemctl is-active pero-llm
echo "Step 1 done."
