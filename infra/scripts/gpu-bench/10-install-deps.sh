#!/usr/bin/env bash
# GPU benchmark phase 1, step 1 (server, root): install the Vulkan runtime and build tools from Debian's official repositories.
# No third-party repository, no non-free component, no kernel module, no reboot. 21 packages in total (listed in the README).
# Rollback: sudo apt-get remove --purge --autoremove mesa-vulkan-drivers vulkan-tools libvulkan-dev glslc spirv-headers
# Usage: sudo ./10-install-deps.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
PKGS="mesa-vulkan-drivers vulkan-tools libvulkan-dev glslc spirv-headers"
apt-get install -y --no-install-recommends $PKGS
for p in $PKGS; do echo "$p $(dpkg-query -W -f '${Version}' "$p")"; done
echo "Step 1 done."
