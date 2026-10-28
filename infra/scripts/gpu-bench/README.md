# GPU benchmark, phase 1: Intel HD 630 and Vulkan (issue #11)

Question: does a GPU make the local Gemma 4 26B-A4B model noticeably faster than CPU only (baseline in `docs/benchmarks/llm-cpu.md`: prompt reading 22–32 tokens/s, writing 7–10 tokens/s)?
Phase 1 uses only Debian's official packages: no non-free component, no kernel module, no reboot. If Mesa's open-source driver also exposes the NVIDIA GeForce 940MX, it is measured too. The NVIDIA proprietary driver (phase 2) needs separate approval.

| Step | Script | Where | What it does |
|---|---|---|---|
| 1 | `10-install-deps.sh` | server, root (sudo) | Installs 5 packages (21 with dependencies) from Debian trixie: `mesa-vulkan-drivers` 25.0.7, `vulkan-tools` 1.4.304, `libvulkan-dev` 1.4.309, `glslc` 2025.2, `spirv-headers` 1.6.1, plus libraries such as libvulkan1, libllvm19, libdrm, libxcb-*, libshaderc1 |
| 2 | `20-build-vulkan.sh` | server, normal user | Builds llama-bench with Vulkan from the same pinned checkout (tag b11796, commit verified) into `build-vulkan/`; the part 5 CPU build and the service are untouched |
| 3 | `30-bench.sh` | server, root (sudo) | Stops `pero-llm`, copies both builds to `/var/tmp/pero-gpu-bench` (root-owned), runs the CPU baseline and then each Vulkan device in two layouts as user `pero` (A: attention on the GPU, experts on the CPU; B: everything on the GPU), logs temperatures, restarts `pero-llm` and waits for its health check (also when the script fails) |

Same settings as part 5: 8 threads, prompts of 64 and 4096 tokens, 128 generated tokens, one run. The software renderer (llvmpipe) is skipped. Layout B may fail to fit; failures are recorded.
`pero-llm` is down while step 3 runs: normally 30–60 minutes, worst case about 100 minutes (five runs with a 20-minute limit each). Nothing else on the server uses it right now.
Results: `~/pero-bench/results/gpu-phase1.txt`, then `docs/benchmarks/llm-gpu.md`.
Decision rule (issue #11): change the running service only if prompt reading is at least 1.5 times faster, writing is not slower, and temperatures and stability are fine.

Rollback: `sudo apt-get remove --purge --autoremove mesa-vulkan-drivers vulkan-tools libvulkan-dev glslc spirv-headers`, then `rm -rf ~/pero-bench/llama.cpp/build-vulkan /var/tmp/pero-gpu-bench`.
