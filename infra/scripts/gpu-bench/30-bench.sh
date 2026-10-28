#!/usr/bin/env bash
# GPU benchmark phase 1, step 3 (server, root): CPU baseline against every Vulkan GPU, same model and settings as part 5 (8 threads,
# 64- and 4096-token prompts, 128 generated tokens, one run). The model is read from /var/lib/pero/models as user "pero".
# The service pero-llm is STOPPED while this runs (normally 30-60 minutes; at most 5 runs of 20 minutes) and started again at the end, even if the script fails.
# Per GPU two layouts: (A) attention and shared weights on the GPU, expert weights on the CPU; (B) everything on the GPU
# (may fail when it does not fit; failures are recorded, not fatal). Software renderers (llvmpipe) are skipped.
# Nothing is installed or changed. Results: /home/<owner>/pero-bench/results/gpu-phase1.txt, copied from /var/tmp/pero-gpu-bench.
# Usage: sudo ./30-bench.sh
set -uo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OWNER="${SUDO_USER:?run through sudo}"; HOMEDIR="$(getent passwd "$OWNER" | cut -d: -f6)"
SRC="$HOMEDIR/pero-bench/llama.cpp"; W=/var/tmp/pero-gpu-bench
[ -x "$SRC/build/bin/llama-bench" ] && [ -x "$SRC/build-vulkan/bin/llama-bench" ] || { echo "Build both CPU and Vulkan llama-bench first." >&2; exit 1; }
MODEL="$(ls /var/lib/pero/models/*.gguf | head -n 1)"; [ -f "$MODEL" ] || { echo "No model found." >&2; exit 1; }

restart_llm(){ systemctl start pero-llm
  for _ in $(seq 1 60); do curl -sf -m 2 http://127.0.0.1:8080/health >/dev/null 2>&1 && { echo "pero-llm is back and healthy."; return; }; sleep 5; done
  echo "WARNING: pero-llm did not become healthy within 5 minutes; check: sudo journalctl -u pero-llm" >&2; }
trap restart_llm EXIT
START=$SECONDS

rm -rf "$W"; install -d -m 0755 -o root -g root "$W"
cp -a "$SRC/build/bin" "$W/cpu"; cp -a "$SRC/build-vulkan/bin" "$W/vulkan"; chown -R root:root "$W"; chmod -R go+rX "$W"
systemctl stop pero-llm
OUT="$W/gpu-phase1.txt"
as_pero(){ runuser -u pero -g render -- env "$@"; }
temps(){ for h in /sys/class/hwmon/hwmon*; do n="$(cat "$h/name" 2>/dev/null)"
  case "$n" in coretemp|nouveau|nvme) [ -r "$h/temp1_input" ] && printf '%s=%s°C ' "$n" "$(( $(cat "$h/temp1_input") / 1000 ))";; esac; done; echo; }
B=(-m "$MODEL" -t 8 -p 64,4096 -n 128 -r 1 -o md)
{
echo "== date"; date -Is; echo "model: $(basename "$MODEL")"; echo "temps before: $(temps)"
echo "== devices seen by llama.cpp (as user pero, group render; name and memory)"
DEVS="$(as_pero LD_LIBRARY_PATH="$W/vulkan" "$W/vulkan/llama-bench" --list-devices 2>&1)"; echo "$DEVS"
as_pero vulkaninfo --summary 2>/dev/null | grep -E "deviceName|driverName|apiVersion"
echo; echo "== CPU baseline (no GPU)"
as_pero LD_LIBRARY_PATH="$W/cpu" timeout 1200 "$W/cpu/llama-bench" "${B[@]}" 2>&1 | grep -v '^$'; echo "temps after: $(temps)"
while read -r DEV NAME; do
  echo; echo "== $DEV: $NAME"
  case "$NAME" in *llvmpipe*|*lavapipe*) echo "software renderer, skipped"; continue;; esac
  echo "-- layout A: attention on GPU, experts on CPU"
  as_pero LD_LIBRARY_PATH="$W/vulkan" timeout 1200 "$W/vulkan/llama-bench" "${B[@]}" -dev "$DEV" -ngl 99 -ot 'exps=CPU' </dev/null 2>&1 | grep -v '^$'; echo "temps after: $(temps)"
  echo "-- layout B: everything on the GPU"
  as_pero LD_LIBRARY_PATH="$W/vulkan" timeout 1200 "$W/vulkan/llama-bench" "${B[@]}" -dev "$DEV" -ngl 99 </dev/null 2>&1 | grep -v '^$' | tail -n 12; echo "temps after: $(temps)"
done < <(echo "$DEVS" | sed -nE 's/^ *(Vulkan[0-9]+): *(.*)$/\1 \2/p')
echo; echo "== done: $(date -Is), pero-llm was stopped for $((SECONDS-START)) s"
} > "$OUT" 2>&1
install -d -o "$OWNER" -g "$OWNER" "$HOMEDIR/pero-bench/results"
install -m 0644 -o "$OWNER" -g "$OWNER" "$OUT" "$HOMEDIR/pero-bench/results/gpu-phase1.txt"
echo "Results written to $HOMEDIR/pero-bench/results/gpu-phase1.txt"
