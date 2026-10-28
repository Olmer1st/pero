# Part 5: llama.cpp and model benchmark

All steps run on the server as the normal user. **No root and no sudo grant are needed.** Everything lives under `~/pero-bench/`
(source, build, models, results); nothing is installed system-wide until part 6.

| Step | Script | What it does |
|---|---|---|
| 1 | `10-build-llama.sh` | Clone tag `b11796` (commit pinned and checked), build `llama-server` and `llama-bench` with `GGML_NATIVE` (this CPU's AVX2/FMA), ccache on |
| 2 | `15-speedtest.sh` | Measures download speed (150 MB) before the ~48 GiB of models |
| 3 | `20-fetch-model.sh <key>` | Resumable download from Hugging Face, SHA-256 checked against `models.conf` |
| 4 | `30-benchmark.sh <key>` | `llama-bench`: prompt processing and generation measured separately; 64 and 4096 prompt tokens; 4 and 8 threads |
| 5 | `40-samples.sh <key>` | One Russian and one English continuation per model via a temporary 127.0.0.1 server; saves text and speeds |
| 6 | `90-report.sh` | Collects results into one file for `docs/benchmarks/llm-cpu.md` |
| after pick | `95-delete-model.sh <key>` | Deletes a model the owner did not choose |

Candidates (all MoE, all Apache-2.0, Q4-class quantisation): Qwen3.6-35B-A3B (20.6 GiB), Gemma 4 26B-A4B-it (15.8 GiB), gpt-oss-20b (11.3 GiB, MXFP4).
Models are never committed. Unchosen models are deleted only after the owner decides.
