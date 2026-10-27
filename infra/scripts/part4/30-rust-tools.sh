#!/usr/bin/env bash
# Part 4, step 3 (dev laptop, no sudo): the Rust dependency auditor and policy checker, built from crates.io
# at pinned versions with their own lockfiles (--locked).
#   cargo-audit: known vulnerabilities (RustSec advisory database)   cargo-deny: licenses, sources, bans, advisories
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cargo install --locked cargo-audit --version 0.22.2
cargo install --locked cargo-deny  --version 0.20.2
cargo audit --version
cargo deny --version
