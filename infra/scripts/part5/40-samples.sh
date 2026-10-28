#!/usr/bin/env bash
# Part 5, step 5 (server): one Russian and one English sample per model, through llama-server on 127.0.0.1 (temporary, stopped at the end).
# Saves the text and the measured speeds. Usage: ./40-samples.sh <key> [threads]
set -euo pipefail
KEY="${1:?usage: $0 <model key>}"; T="${2:-4}"
HERE="$(cd "$(dirname "$0")" && pwd)"
IFS='|' read -r _ _ FILE _ _ < <(grep "^$KEY|" "$HERE/models.conf")
W="$HOME/pero-bench"; R="$W/results"; mkdir -p "$R"
"$W/llama.cpp/build/bin/llama-server" -m "$W/models/$FILE" --host 127.0.0.1 --port 18080 -t "$T" -c 8192 > "$R/$KEY-server.log" 2>&1 &
PID=$!; trap 'kill $PID 2>/dev/null || true; wait $PID 2>/dev/null || true' EXIT
for i in $(seq 1 180); do curl -fsS -m 2 http://127.0.0.1:18080/health >/dev/null 2>&1 && break; sleep 2; done
for L in ru en; do
  python3 - "$HERE/samples/$L.txt" "$R/$KEY-sample-$L" <<'PY'
import json, sys, urllib.request
prompt = open(sys.argv[1], encoding="utf-8").read()
body = {"messages": [{"role": "user", "content": prompt}], "max_tokens": 400, "temperature": 0.7,
        "chat_template_kwargs": {"enable_thinking": False}, "reasoning_effort": "low"}
req = urllib.request.Request("http://127.0.0.1:18080/v1/chat/completions", json.dumps(body).encode(), {"Content-Type": "application/json"})
d = json.load(urllib.request.urlopen(req, timeout=900))
open(sys.argv[2] + ".txt", "w", encoding="utf-8").write(d["choices"][0]["message"].get("content") or "")
t = d.get("timings", {})
open(sys.argv[2] + ".speed", "w").write("prompt tokens/s %.1f, generation tokens/s %.1f, prompt tokens %s, generated tokens %s\n" % (
    t.get("prompt_per_second", 0), t.get("predicted_per_second", 0), t.get("prompt_n"), t.get("predicted_n")))
PY
  echo "$L: $(cat "$R/$KEY-sample-$L.speed")"
done
