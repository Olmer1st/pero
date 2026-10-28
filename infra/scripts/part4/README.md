# Part 4: toolchains

| Step | Where | Script | Needs root | What it does |
|---|---|---|---|---|
| 1 | server | `10-server-packages.sh` | yes | Debian packages: build-essential, cmake, git, ccache, pkg-config, libcurl dev, OpenJDK 21 headless (for LanguageTool) |
| 2 | dev laptop | `20-rust-toolchain.sh` | no | Rust 1.99.1 via rustup 1.29.2; installer SHA-256 pinned and checked before it runs; shell files untouched |
| 3 | dev laptop | `30-rust-tools.sh` | no | `cargo-audit` 0.22.2 and `cargo-deny` 0.20.2, `--locked` |
| 4 | either | `90-verify.sh [audit ../../../deny.toml]` | no | Prints versions; `audit` runs both tools on a scratch crate with the repo `deny.toml` |

Rust is installed on the dev laptop only: Rust is built there and the binary is deployed (see DECISIONS.md), which keeps
compilers for Rust, and the crates they download, off the server. Server code uses rustls, so no OpenSSL dev package is needed.
The server does get gcc and cmake, because llama.cpp is built there for its CPU (part 5).

Rollback: `sudo apt-get remove <packages>` on the server; `rustup self uninstall` on the laptop.
