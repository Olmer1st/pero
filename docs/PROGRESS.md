# Progress

## Turn 1 — server bootstrap
Status: in progress.

| Part | Status |
|---|---|
| 0. Repository skeleton and shared documents | confirmed |
| 1. Inventory report | confirmed (`docs/inventory/server-inventory.md`) |
| 2. Base hardening | applied and verified from the dev laptop (`infra/scripts/part2/verify-from-client.sh`: all pass); physical lid-close test pending |
| 3. Name resolution (`writer.local`); Tailscale and remote access deferred | in progress |
| 4. Toolchains | pending |
| 5. llama.cpp and model benchmark | pending |
| 6. LLM service | pending |
| 7. LanguageTool service | pending |
| 8. Rust server skeleton | pending |
| 9. Security baseline report | pending |

### Setup notes
- Enable the secret-scanning hook after cloning: `infra/scripts/install-hooks.sh` (needs `gitleaks` on PATH).
