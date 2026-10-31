<script lang="ts">
  import type { HealthReport } from "../generated/api.ts";
  import { health } from "../lib/api.ts";

  type State = { kind: "checking" } | { kind: "down" } | { kind: "up"; report: HealthReport };

  let state = $state<State>({ kind: "checking" });

  async function poll(): Promise<void> {
    try {
      state = { kind: "up", report: await health() };
    } catch {
      state = { kind: "down" };
    }
  }

  $effect(() => {
    void poll();
    const timer = setInterval(poll, 10_000);
    return () => clearInterval(timer);
  });

  const label = (s: State): string => {
    if (s.kind === "checking") return "Checking the server…";
    if (s.kind === "down") return "Server unreachable";
    const llm = s.report.services.llm.status === "up";
    const grammar = s.report.services.languagetool.status === "up";
    if (llm && grammar) return "Connected";
    return `Connected, but ${[!llm && "the language model", !grammar && "grammar check"].filter(Boolean).join(" and ")} is down`;
  };
  const level = (s: State): string =>
    s.kind === "up" ? (s.report.status === "ok" ? "ok" : "warn") : s.kind === "down" ? "bad" : "wait";
</script>

<div class="status" role="status" aria-live="polite">
  <span class="dot {level(state)}" aria-hidden="true"></span>
  <span>{label(state)}</span>
  {#if state.kind === "up"}
    <span class="muted">· server {state.report.version}</span>
  {/if}
</div>
