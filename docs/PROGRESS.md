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
| 5. llama.cpp and model benchmark | built and benchmarked (`docs/benchmarks/llm-cpu.md`); awaiting owner's model pick |
| 6. LLM service | pending |
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
