# Progress

## Turn 1 — server bootstrap
Status: all nine parts done and checked; reboot test (parts 6–8) and lid-close test (part 2) pending on the owner's physical access. One pull request for the turn awaits the owner's merge.

| Part | Status |
|---|---|
| 0. Repository skeleton and shared documents | confirmed |
| 1. Inventory report | confirmed (`docs/inventory/server-inventory.md`) |
| 2. Base hardening | applied and verified from the dev laptop (`infra/scripts/part2/verify-from-client.sh`: all pass); physical lid-close test pending |
| 3. Name resolution (`writer.local`); Tailscale and remote access deferred | confirmed |
| 4. Toolchains | confirmed |
| 5. llama.cpp and model benchmark | confirmed; owner picked Gemma 4 26B-A4B (`docs/benchmarks/llm-cpu.md`) |
| 6. LLM service | confirmed; reboot test pending (owner, when near the server) |
| 7. LanguageTool service | confirmed; reboot test pending together with part 6 |
| 8. Rust server skeleton | installed and verified from the dev laptop; reboot test pending together with parts 6 and 7 |
| 9. Security baseline report | `docs/security/baseline.md`; only 22, 8443 and 9090 answer from the network (Cockpit 9090 is an accepted exception) |

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

### LanguageTool service (part 7)
- Service `pero-languagetool`: user `pero`, `127.0.0.1:8010` only (no firewall rule), enabled at boot, sandbox score 1.7. LanguageTool 6.8 (LGPL-2.1) from Maven Central, SHA-256 pinned, root-owned in `/opt/pero/languagetool/6.8`.
- Callers must name the language (`language=ru-RU` or `en-US`); automatic detection is not available (no fastText model installed). 60 languages are loaded; no n-gram data, no telemetry.
- Settings: `/etc/pero/languagetool.properties` (max text 200,000 chars, max check time 120 s). Java heap capped at 2 GB. A 51,000-character Russian chapter checks in about 4 seconds, about 0.9 GB resident.
- Seeded checks pass: English `BASE_FORM`, `EN_A_VS_AN`, `HE_VERB_AGR`; Russian `MORFOLOGIK_RULE_RU_RU`. Logs hold lengths and timings only, no text.
- Check: `infra/scripts/part7/verify-from-client.sh pero-server`. Logs: `sudo journalctl -u pero-languagetool`.
- Open: reboot test with part 6 (run both `verify-from-client.sh` scripts afterwards).
- Both services run together: about 22 GB of memory stays available.

### Pero server (part 8)
- `pero-server` (Rust, axum + rustls) on `0.0.0.0:8443`, TLS 1.3 only, user `pero`, systemd sandbox score 1.7. Binary root-owned in `/opt/pero/server/<sha12>/` (`current` symlink); settings in `/etc/pero/server.toml`.
- `GET /health` (no login yet): status, version, uptime and up/down with latency for the LLM and LanguageTool. Results cached 3 s.
- Certificate: private home CA in `~/.pero-ca` on the dev laptop (name-constrained to `writer.local` and `10.0.0.0/24`, valid to 2031-10-27, SHA-256 fingerprint 62:D8:0E:4B:…:48:0E). The server certificate is valid to 2027-10-28: **renew it by 2027-09-28** (steps 2, 3 and 5 in `infra/scripts/part8/README.md`). Back up `~/.pero-ca` offline. The client imports `ca.crt` once.
- Binary SHA-256 at install: `5089727028c5499fd461868c403b2e372e4269285e5bc5f15d29cd688f944f97`.
- Known gaps: `/health` is unauthenticated (login comes in a later turn); no idle timeouts after the TLS handshake.
- Listening sockets from the network: 22, 9090 (Cockpit), 8443 only; LLM and LanguageTool stay on loopback.

### Backlog
- Optional GPU benchmark for the local LLM (Intel HD 630 via Vulkan, NVIDIA 940MX via CUDA) against the CPU baseline, to run after turn 1 is merged: [issue #11](https://github.com/Olmer1st/pero/issues/11). Owner agreed.

### Operating the server
| What | Unit / script | Restart | Logs | Check from the laptop |
|---|---|---|---|---|
| LLM (Gemma 4 26B-A4B) | `pero-llm`, 127.0.0.1:8080 | `sudo systemctl restart pero-llm` | `sudo journalctl -u pero-llm` | `infra/scripts/part6/verify-from-client.sh pero-server` |
| Grammar (LanguageTool 6.8) | `pero-languagetool`, 127.0.0.1:8010 | `sudo systemctl restart pero-languagetool` | `sudo journalctl -u pero-languagetool` | `infra/scripts/part7/verify-from-client.sh pero-server` |
| Pero server | `pero-server`, :8443 (HTTPS) | `sudo systemctl restart pero-server` | `sudo journalctl -u pero-server` | `infra/scripts/part8/verify-from-client.sh pero-server` |
| Network exposure | ufw | | | `infra/scripts/part9/verify-from-client.sh pero-server` |

The server starts the three services in that order at boot. The owner reaches the server at `https://writer.local:8443/health` after importing `~/.pero-ca/ca.crt` into the client once.
Privileged steps stay reviewed scripts: the owner installs a one-time limited grant (`infra/scripts/grant-sudo.sh`) when the assistant should run them, and removes it afterwards (`sudo rm /etc/sudoers.d/pero-<bundle>` **on the server**).

### Known issues
- Reboot test pending for parts 6, 7 and 8 (Wi-Fi reconnection after boot untested); lid-close test pending for part 2. Do both when physically at the server, then rerun the three `verify-from-client.sh` scripts.
- `/health` has no login; no idle timeout after the TLS handshake.
- Wi-Fi only, DHCP lease not reserved.
- Server certificate expires 2027-10-28 (renew by 2027-09-28); back up `~/.pero-ca` offline.
- Recommendations from the baseline awaiting the owner: disable bluetooth, ModemManager and udisks2; check NetworkManager vs `networking`; remove `/opt/pero-provision/pero-part*`.

### Notes for turn 2
- Server base: `pero-server` is a minimal axum app in `core/` (config in `core/src/config.rs`, TLS in `core/src/tls.rs`, `/health` in `core/src/health.rs`). Add routes to `health::router`-style modules; keep dependencies few and run `cargo audit` and `cargo deny check` before each commit.
- Services to call from the server: LLM `127.0.0.1:8080` (llama.cpp OpenAI-style API; one request at a time, thinking off by default; 7–10 tokens/s generation, 22–32 tokens/s prompt reading) and LanguageTool `127.0.0.1:8010` (callers must pass `ru-RU` or `en-US`).
- Rust is built on the dev laptop and the binary is deployed (`infra/scripts/part8/25-build.sh`, `30-install-server.sh`); the server has no Rust toolchain.
- The local LLM is slow for long prompts; keep prompts scene-sized.
