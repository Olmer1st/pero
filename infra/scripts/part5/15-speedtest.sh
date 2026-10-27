#!/usr/bin/env bash
# Part 5, step 2 (server): measure the download speed before fetching about 48 GiB. Reads 150 MB of the smallest model, writes nothing permanent.
set -euo pipefail
IFS='|' read -r _ REPO FILE _ _ < <(grep '^gptoss' "$(dirname "$0")/models.conf")
curl --proto '=https' --tlsv1.2 -fsSL -r 0-157286399 -o /dev/null -w 'speed: %{speed_download} bytes/s, %{time_total} s for 150 MB\n' "https://huggingface.co/$REPO/resolve/main/$FILE"
