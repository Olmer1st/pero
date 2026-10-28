#!/usr/bin/env bash
# Points this clone at the versioned git hooks.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
git config core.hooksPath infra/git-hooks
echo "git hooks enabled (core.hooksPath=infra/git-hooks)"
