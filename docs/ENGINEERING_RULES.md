# Engineering rules

These rules apply to every turn and every contributor. They are non-negotiable unless the owner approves a change and it is recorded in `DECISIONS.md`.

## Model roles and GitHub workflow
- **Opus — main session:** orchestration, architecture, design and every decision. Only Opus chooses between options, records entries in `docs/DECISIONS.md` and presents plans to the owner. Subagents surface options; they never decide.
- **Sonnet — subagents:** coding, tests, codebase exploration and web research.
- **Haiku — subagents:** all git and GitHub operations (branches, worktrees, commits, pushes, pull requests, issues, labels, comments), file operations (copying, moving, bulk renames) and browser operations.
- **GitHub issues as the work log:** after the owner approves the plan, create one issue per part with its acceptance check, grouped under a milestone for this turn. Reference the issue in every commit and pull request. Update the issue with the result when its check passes; the merged pull request closes it. Anything discovered along the way (bug, follow-up, open question) becomes its own issue instead of silent scope growth.
- **GitHub access:** a fine-grained token the owner creates, scoped to this one repository. Never paste the token into files, issues, logs or commits. Issues and pull requests never contain secrets, private manuscript text or server addresses.

## Locked decisions
- **No credentials in code or repository** — not in source, scripts, units, templates, commits or logs. Secrets live in a file outside the repo, owned by the service user, owner-only permissions. The repo holds templates with placeholders.
- **Trusted sources only:** Ubuntu official repos; vendor-signed apt repos (Tailscale); crates.io; PyPI; official npm registry; releases from the project's official GitHub organization. No unofficial PPAs, mirrors or forks.
- **Verify downloads** with published checksums or signatures. Model files from the author's official Hugging Face organization or a widely used quantizer; record SHA-256.
- **Audit every dependency before adding it:** known vulnerabilities, maintenance, adoption, license, typosquatting. Prefer fewer dependencies.
- **Pin versions;** commit lockfiles; build llama.cpp from a pinned release tag.
- **Least privilege:** root only for installation and system configuration. Services run as a dedicated non-root user. Day-to-day work runs as a non-root account.
- **Exposure:** only the Rust server listens on the network, port 8443. Everything else binds to 127.0.0.1.
- **Stack:** Rust (axum, tokio, sqlx/SQLite). llama.cpp or ik_llama.cpp. Mixture-of-experts models only.

## Git workflow
- `main` is protected: changes arrive only through pull requests; the owner merges. Never commit or push directly to `main`.
- One branch per turn as a git worktree, with its own upstream branch.
- Commit each part when it passes its check; open one pull request per turn.
- Infrastructure is versioned: provisioning scripts, systemd units, firewall rules, config templates, reports.

## Working method
- One part at a time: implement → check → self-review → owner confirms → next.
- Prefer distribution packages over manual installs where a supported version exists.
- Never pipe a remote script into a shell without downloading it, verifying it, and showing it to the owner.
- Capture every manual step as a script or documented step in `infra/`.
- Do not regress what already works without approval.
- Any destructive or irreversible step (OS reinstall, disk changes, deleting users, opening ports) needs separate owner approval.
