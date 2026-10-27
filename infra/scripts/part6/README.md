# Part 6: LLM service

Model: Gemma 4 26B-A4B-it (UD-Q4_K_M), chosen by the owner after the part 5 benchmark. llama.cpp tag `b11796`.

| Step | Script | Root | What it does |
|---|---|---|---|
| 1 | `10-install-llm.sh` | yes (sudo) | Copies the binaries to `/opt/pero/llama.cpp/b11796/bin` (root-owned), the model to `/var/lib/pero/models` (hash re-checked, benchmark copy removed), writes `/etc/pero/llm.env` and the sandboxed unit `pero-llm.service`, starts it |
| 2 | `90-report.sh` | yes | No-secrets status report |
| 3 | `verify-from-client.sh <alias>` | no | From the laptop: port 8080 unreachable from the network, loopback bind, enabled at boot, a real completion |
| 4 | reboot, then `verify-from-client.sh` again | owner | Survives reboot |

Service: user `pero`, `127.0.0.1:8080`, no web UI, thinking off by default (a request can turn it on with `chat_template_kwargs: {"enable_thinking": true}`), one request slot, 16k context.
Change model or context: edit `/etc/pero/llm.env`, then `sudo systemctl restart pero-llm`.
Restart: `sudo systemctl restart pero-llm`. Logs: `sudo journalctl -u pero-llm` (the server does not log prompt text).
Rollback: `sudo systemctl disable --now pero-llm && sudo rm /etc/systemd/system/pero-llm.service`.
