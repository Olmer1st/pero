#!/usr/bin/env bash
# Part 5, step 3 (server): download one model from Hugging Face (resumable), then verify the pinned SHA-256. Deletes it if the hash is wrong.
# Usage: ./20-fetch-model.sh <key from models.conf>
set -euo pipefail
KEY="${1:?usage: $0 <model key>}"
IFS='|' read -r _ REPO FILE SHA SIZE < <(grep "^$KEY|" "$(dirname "$0")/models.conf") || true
[ -n "${FILE:-}" ] || { echo "Unknown key $KEY" >&2; exit 1; }
D="$HOME/pero-bench/models"; mkdir -p "$D"; cd "$D"
for attempt in $(seq 1 40); do
  [ "$(stat -c %s "$FILE" 2>/dev/null || echo 0)" = "$SIZE" ] && break
  curl --proto '=https' --tlsv1.2 -fL -C - --retry 5 --retry-delay 5 -o "$FILE" "https://huggingface.co/$REPO/resolve/main/$FILE" || sleep 10
done
[ "$(stat -c %s "$FILE" 2>/dev/null || echo 0)" = "$SIZE" ] || { echo "Download incomplete or wrong size for $FILE" >&2; exit 1; }
echo "$SHA  $FILE" | sha256sum -c - || { echo "Hash mismatch, removing $FILE" >&2; rm -f "$FILE"; exit 1; }
ls -l "$FILE"
