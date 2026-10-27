#!/usr/bin/env bash
# Part 5 (server): gathers all results into one text file (no secrets) so it can be copied to the repo and written up.
set -euo pipefail
R="$HOME/pero-bench/results"; OUT="$R/summary.txt"
{ echo "== host"; lscpu | grep -E 'Model name|^CPU\(s\)|Thread' ; grep -o -m1 -w -E 'avx2|avx512f' /proc/cpuinfo | sort -u | tr '\n' ' '; echo; free -g | head -2
  echo "== llama.cpp"; "$HOME/pero-bench/llama.cpp/build/bin/llama-server" --version 2>&1 | head -2
  for f in "$R"/*-bench.md; do echo "== $(basename "$f")"; cat "$f"; done
  for f in "$R"/*.speed; do echo "== $(basename "$f")"; cat "$f"; done
  echo "== models on disk"; ls -l "$HOME/pero-bench/models"; } > "$OUT"
echo "Wrote $OUT"
