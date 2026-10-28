#!/usr/bin/env bash
# Part 4 check. Read-only: prints the versions to record in docs/PROGRESS.md. Works on either machine.
# With "audit" it also proves the auditor and policy checker run clean on a scratch crate using the repo's deny.toml.
# Usage: ./90-verify.sh [audit <path-to-deny.toml>]
set -uo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
for c in rustc cargo cargo-audit cargo-deny gcc cmake git ccache java; do
  printf '%-12s' "$c"; if command -v "$c" >/dev/null; then
    case "$c" in java) java -version 2>&1 | head -1;; *) "$c" --version 2>&1 | head -1;; esac
  else echo "not installed"; fi
done
if [ "${1:-}" = audit ]; then
  DENY="$(readlink -f "${2:?path to deny.toml}")"
  D="$(mktemp -d)"; trap 'rm -rf "$D"' EXIT
  ( cd "$D" && cargo init --quiet --name scratch . \
    && sed -i '/^edition/a license = "Apache-2.0"' Cargo.toml && cargo generate-lockfile --quiet \
    && cargo audit && cargo deny --config "$DENY" check ) && echo "RESULT: auditor and policy checker clean" || echo "RESULT: FAILED"
fi
