# Progress

## Turn 1 — server bootstrap
Status: in progress.

| Part | Status |
|---|---|
| 0. Repository skeleton and shared documents | confirmed |
| 1. Inventory report | confirmed (`docs/inventory/server-inventory.md`) |
| 2. Base hardening | applied and verified from the dev laptop (`infra/scripts/part2/verify-from-client.sh`: all pass); physical lid-close test pending |
| 3. Name resolution (`writer.local`); Tailscale and remote access deferred | confirmed |
| 4. Toolchains | confirmed |
| 5. llama.cpp and model benchmark | confirmed; owner picked Gemma 4 26B-A4B (`docs/benchmarks/llm-cpu.md`) |
| 6. LLM service | installed and verified from the dev laptop; reboot test pending (owner, when near the server) |
| 7. LanguageTool service | pending |
| 8. Rust server skeleton | pending |
| 9. Security baseline report | pending |

### Setup notes
- Enable the secret-scanning hook after cloning: `infra/scripts/install-hooks.sh` (needs `gitleaks` on PATH).

### Toolchain versions (part 4)
| Where | Tool | Version | Source |
|---|---|---|---|
| dev laptop | Rust (rustc, cargo, clippy, rustfmt) | 1.99.1 | official rustup 1.29.2, installer SHA-256 pinned |
| dev laptop | cargo-audit | 0.22.2 | crates.io, `--locked` |
| dev laptop | cargo-deny | 0.20.2 | crates.io, `--locked` |
| server | gcc / cmake / git / ccache | 14.2.0 / 3.31.6 / 2.47.3 / 4.11.2 | Debian 13 |
| server | OpenJDK headless | 21.0.12.1 | Debian 13 (for LanguageTool) |
| server | build-essential, pkg-config, libcurl4-openssl-dev | 12.12, 1.8.1-4, 8.14.1-2+deb13u7 | Debian 13 |

Policy file: `deny.toml` (permissive licenses only, crates.io only, yanked crates denied). Run `cargo audit` and `cargo deny check` in the Rust workspace once it exists (part 8).

### LLM service (part 6)
- Service `pero-llm`: user `pero`, `127.0.0.1:8080` only (no firewall rule for 8080), enabled at boot, sandbox score 1.7 (`systemd-analyze security`).
- Model: Gemma 4 26B-A4B-it UD-Q4_K_M in `/var/lib/pero/models/`; llama.cpp b11796 in `/opt/pero/llama.cpp/b11796/bin` (root-owned, `current` symlink).
- Settings in `/etc/pero/llm.env` (model path, port, 8 threads, 16k context). Change, then `sudo systemctl restart pero-llm`.
- Thinking is off by default (`--reasoning off`); a request can turn it on with `chat_template_kwargs: {"enable_thinking": true}`. With thinking on, Gemma 4 spends the token budget on reasoning first (empty `content` until it ends).
- Memory when loaded: about 9.7 GB ordinary plus about 16 GB reclaimable file cache; about 22 GB stays available. Server log contains no prompt text.
- Check: `infra/scripts/part6/verify-from-client.sh pero-server`. Logs: `sudo journalctl -u pero-llm`.
- Open: reboot test. Run `verify-from-client.sh` again after a reboot; do it only when someone can reach the server physically (Wi-Fi reconnect after boot is untested).
