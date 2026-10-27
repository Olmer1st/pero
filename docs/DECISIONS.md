# Decision log

Dated entries, newest last. Format: date — decision — reason.

- 2026-10-26 — Work runs on the 8 GB dev laptop; Rust is built there and binaries are deployed to the server; llama.cpp is built on the server for its CPU. — Handoff topology recommendation.
- 2026-10-26 — Secret scanning uses gitleaks 8.30.1 (official gitleaks GitHub release, SHA-256 verified), run from a versioned pre-commit hook in `infra/git-hooks/`. — Official source, single static binary, no extra dependencies.
- 2026-10-26 — Issues and labels are created through the GitHub REST API with the repository-scoped token held by git's credential helper; the `gh` CLI is not used. — Owner instruction.
- 2026-10-26 — Server installation steps are performed through Cockpit on the server. — Owner instruction.
