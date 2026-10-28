#!/usr/bin/env bash
# Part 8, step 4 (dev laptop, no sudo): reproducible release build of the server from the committed lockfile.
# Usage: ./25-build.sh      Prints the path and SHA-256 of the binary.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(git rev-parse --show-toplevel)"
cargo build --release --locked -p pero-server
sha256sum target/release/pero-server
