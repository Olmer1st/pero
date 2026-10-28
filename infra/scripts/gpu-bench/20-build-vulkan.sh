#!/usr/bin/env bash
# GPU benchmark phase 1, step 2 (server, normal user, no sudo): build a second copy of the pinned llama.cpp with the Vulkan backend.
# Uses the same checkout as part 5 (tag b11796, commit checked); the CPU build in build/ and the running service are untouched.
set -euo pipefail
COMMIT=0a1ef309cdbb9f2024f58a38fb4c42352c5db569
cd "$HOME/pero-bench/llama.cpp"
[ "$(git rev-parse HEAD)" = "$COMMIT" ] || { echo "Checkout does not match the pinned commit. Stopping." >&2; exit 1; }
command -v glslc >/dev/null || { echo "glslc missing: run step 1 first." >&2; exit 1; }
pkg-config --exists vulkan || { echo "Vulkan headers missing: run step 1 first." >&2; exit 1; }
cmake -B build-vulkan -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=ON -DGGML_VULKAN=ON -DGGML_CUDA=OFF \
      -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF \
      -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache
cmake --build build-vulkan --config Release -j 6 --target llama-bench
ls -l build-vulkan/bin/llama-bench
echo "Step 2 done."
