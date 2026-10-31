#!/usr/bin/env bash
# Builds the desktop app on this machine: checks and builds the web client, then the app (release), with the home CA built in.
# Needs: Node 22 on PATH, Rust, the Tauri system packages (see README.md), and the home CA's public certificate
# (default ~/.pero-ca/ca.crt, or set PERO_CA_FILE). Takes a while on a small machine; it uses 2 parallel jobs on purpose.
# Usage: desktop/build.sh
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
ROOT="$(git rev-parse --show-toplevel)"
( cd "$ROOT/client" && npm ci && npm run check && npm test && npm run build )
cd "$ROOT/desktop/src-tauri"
cargo test -j 2
cargo audit --ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0429 --ignore RUSTSEC-2026-0370 --ignore RUSTSEC-2026-0371 --ignore RUSTSEC-2026-0372
cargo deny --config ../deny.toml check
cargo build --release -j 2
sha256sum target/release/pero-desktop
echo "Built: $PWD/target/release/pero-desktop   (try it without a window: $PWD/target/release/pero-desktop --check)"
