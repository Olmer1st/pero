#!/usr/bin/env bash
# Part 4, step 1 (server, root): Debian packages for building llama.cpp and for running LanguageTool.
# Only Debian's own repositories. No recommended extras. Versions are whatever Debian 13 ships, recorded at the end.
# Usage: sudo ./10-server-packages.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
PKGS=(build-essential cmake git ccache pkg-config libcurl4-openssl-dev openjdk-21-jre-headless)
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends "${PKGS[@]}"
echo "--- installed versions"
dpkg-query -W -f='${Package} ${Version}\n' "${PKGS[@]}"
java -version 2>&1 | head -1
cmake --version | head -1
gcc --version | head -1
echo "Step 1 done."
