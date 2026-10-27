# Architecture

## Context
- **Pero:** a local-first, bilingual (Russian/English) novel-writing assistant. Repository: `pero` (monorepo).
- **Machines:** server laptop (64 GB, older Ubuntu); two dev/backup laptops (8 GB each); the owner's client machine.
- **Final architecture:** a Rust server is the only network-facing process. llama.cpp, LanguageTool and a Python NLP sidecar run on the server, bound to localhost. The client is a web interface, also packaged as a desktop app.

## Repository layout
| Folder | Purpose |
|---|---|
| `core/` | Rust server |
| `client/` | Web interface (and desktop wrapper) |
| `shared/` | Shared API types |
| `sidecar/` | Python NLP sidecar |
| `catalog/` | Provider catalog (YAML) |
| `skills/` | Bundled LLM skills |
| `infra/` | Provisioning scripts, systemd units, firewall rules, config templates, git hooks |
| `docs/` | Rules, architecture, decisions, progress, reports |
