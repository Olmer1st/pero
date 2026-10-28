# LLM with a GPU: phase 1 result (issue #11)

Question: does a GPU make the local Gemma 4 26B-A4B-it (UD-Q4_K_M, 15.77 GiB) noticeably faster than CPU only?
Server: Dell Latitude 5580, Intel Core i7-7820HQ, 31 GiB RAM, Intel HD 630 and an NVIDIA GeForce 940MX (Maxwell). llama.cpp tag `b11796` (commit `0a1ef30`) built twice from the same checkout: CPU (part 5) and Vulkan (`GGML_VULKAN=ON`). Measured 2026-10-28 with `infra/scripts/gpu-bench/`, as the unprivileged `pero` user, 8 threads, one run per setting.
Phase 1 used only Debian trixie official packages (Mesa 25.0.7 Vulkan driver, no non-free component, no kernel module, no reboot).

## Result: no GPU numbers; phase 1 closed with no change

| Run | Prompt 64 | Prompt 4096 | Writing 128 | Outcome |
|---|---|---|---|---|
| CPU, 8 threads | 26.42 | 23.96 | 7.48 tok/s | ran (matches the part 5 baseline: 26.60, 23.57, 7.45) |
| Intel HD 630, attention on GPU, experts on CPU (layout A) | – | – | – | failed: `ErrorOutOfDeviceMemory` on a 1 MiB allocation while loading |
| Intel HD 630, everything on GPU (layout B) | – | – | – | failed: `ErrorOutOfDeviceMemory` on a 257 MiB allocation while loading |
| NVIDIA GeForce 940MX | – | – | – | not offered by the open-source stack (see below) |

What the log shows:
- Mesa exposed exactly one real Vulkan device, the Intel HD 630 (Mesa driver, shared memory: 15 989 MiB heap, 14 390 MiB free). The software renderer (llvmpipe) was skipped.
- Layout B cannot fit: the model is 15.77 GiB and the GPU heap offers 14.4 GiB, before the context cache.
- Layout A should need only a small part of the model on the GPU, yet failed on the first 1 MiB allocation. **The cause was not found.** It was not investigated further because of the decision below.
- The 940MX was not offered: Mesa's `nouveau` Vulkan support does not cover this Maxwell card. Testing it needs the NVIDIA proprietary driver (phase 2, not approved and not done).
- Log noise: many `Permission denied` lines about the library search in `/home/olmer` (the home directory is 0700). The Vulkan backend still loaded; this is harmless for the benchmark.

Stability and temperature:
- `pero-llm` was stopped 376 s for the run and came back healthy by itself; the check from the laptop passed afterwards (`infra/scripts/part6/verify-from-client.sh`).
- The CPU package temperature rose from 45 °C to 82 °C during the CPU baseline (84 °C after the failed GPU attempts, which were short). The package throttle counter read 35 at the end of the session (counted since boot, including the build, so not attributable to one run). Under sustained full load this laptop runs hot and throttles.
- The GPU runs never produced results, so no GPU temperature or stability data exists.

## Decision
Decision rule (issue #11): change the running service only if prompt processing is at least 1.5 times faster, writing is not slower, and temperatures and stability are fine. There are no GPU measurements, so the rule is not met and **the service stays CPU only**.

The owner chose to close phase 1 here and not to retry layout A or start phase 2. Reasons recorded:
- The Intel GPU shares the same RAM as the CPU. Writing speed is limited by memory bandwidth, so a gain there is not expected. (This is an expectation from the architecture, not a measurement.)
- A faster prompt reading on the HD 630 would have to beat eight AVX2 threads by 1.5 times; not expected for this small GPU, also unmeasured.
- The 940MX has 2–4 GB of memory, a small part of a 15.8 GiB model, and phase 2 means a non-free driver, a kernel module and a reboot on the machine that serves the writing, plus extra heat on a laptop that already throttles under CPU load.

Revisit only if the hardware or model changes (a bigger or newer GPU, a much smaller model), or the layout A failure turns out to be a simple configuration problem.

## Rollback of the phase 1 installation
- Server, with the owner's own sudo: `sudo apt-get remove --purge --autoremove mesa-vulkan-drivers vulkan-tools libvulkan-dev glslc spirv-headers`.
- Server, normal user: `rm -rf ~/pero-bench/llama.cpp/build-vulkan`; with sudo: `rm -rf /var/tmp/pero-gpu-bench`.
- The sudo grant: `sudo rm /etc/sudoers.d/pero-pero-gpu1` on the server. Removed by the owner and verified on 2026-10-28 (`sudo -n -l` asks for a password; `/etc/sudoers.d` holds only the stock README). The first attempt was run on the wrong machine and left the grant in place.
- The part 5 CPU build and the `pero-llm` service were never changed.
