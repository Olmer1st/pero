#!/usr/bin/env bash
# Part 5 (server): delete a model file that was not chosen. Run only after the owner has picked. Usage: ./95-delete-model.sh <key>
set -euo pipefail
KEY="${1:?usage: $0 <model key>}"
IFS='|' read -r _ _ FILE _ _ < <(grep "^$KEY|" "$(dirname "$0")/models.conf")
rm -v -- "$HOME/pero-bench/models/$FILE"
