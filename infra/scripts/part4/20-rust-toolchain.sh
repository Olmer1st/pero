#!/usr/bin/env bash
# Part 4, step 2 (dev laptop, as the normal user, no sudo): Rust through the official rustup installer.
# The installer is downloaded from the pinned rustup release, checked against a pinned SHA-256, and only then run.
# Nothing is piped into a shell. Rust goes to ~/.rustup and ~/.cargo; shell startup files are NOT modified.
set -euo pipefail
RUSTUP_VERSION=1.29.2
RUSTUP_SHA256=5be191e8c70a5007074254cb74d78dbfdd25cd5a61c27de7b8e5410122ec24f4
RUST_VERSION=1.99.1
URL="https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init"
D="$(mktemp -d)"; trap 'rm -rf "$D"' EXIT
curl --proto '=https' --tlsv1.2 -fsSL -o "$D/rustup-init" "$URL"
echo "${RUSTUP_SHA256}  $D/rustup-init" | sha256sum -c -
chmod 0700 "$D/rustup-init"
"$D/rustup-init" -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION" -c clippy -c rustfmt
echo "Add to your shell when you want it:  export PATH=\"\$HOME/.cargo/bin:\$PATH\""
"$HOME/.cargo/bin/rustc" --version
