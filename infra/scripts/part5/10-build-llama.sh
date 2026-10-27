#!/usr/bin/env bash
# Part 5, step 1 (server, as the normal user, no sudo): build llama.cpp from a pinned release tag, tuned to this CPU.
# Source: the project's official GitHub organisation. The tag must resolve to the pinned commit or the script stops.
set -euo pipefail
TAG=b11796
COMMIT=0a1ef309cdbb9f2024f58a38fb4c42352c5db569
W="$HOME/pero-bench"; mkdir -p "$W"; cd "$W"
[ -d llama.cpp ] || git clone --quiet --branch "$TAG" --depth 1 https://github.com/ggml-org/llama.cpp.git
cd llama.cpp
[ "$(git rev-parse HEAD)" = "$COMMIT" ] || { echo "Tag $TAG does not match the pinned commit. Stopping." >&2; exit 1; }
# GGML_NATIVE tunes for this CPU (AVX2/FMA/F16C here). No GPU backends. Tests and examples are skipped.
cmake -B build -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=ON -DGGML_CUDA=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF \
      -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache
cmake --build build --config Release -j 6 --target llama-server llama-bench
./build/bin/llama-server --version 2>&1 | head -3
echo "Step 1 done."
