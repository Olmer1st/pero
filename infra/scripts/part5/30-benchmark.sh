#!/usr/bin/env bash
# Part 5, step 4 (server): speed of one model. llama-bench reports prompt processing (pp) and generation (tg) separately.
# Short prompt = 64 tokens, long prompt = 4096 tokens, 128 generated tokens, with 4 and 8 threads (4 cores / 8 threads CPU).
# Usage: ./30-benchmark.sh <key>      Results: ~/pero-bench/results/<key>-bench.md
set -euo pipefail
KEY="${1:?usage: $0 <model key>}"
IFS='|' read -r _ _ FILE _ _ < <(grep "^$KEY|" "$(dirname "$0")/models.conf")
W="$HOME/pero-bench"; mkdir -p "$W/results"
"$W/llama.cpp/build/bin/llama-bench" -m "$W/models/$FILE" -t 4,8 -p 64,4096 -n 128 -r 1 -o md | tee "$W/results/$KEY-bench.md"
