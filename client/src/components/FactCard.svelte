<script lang="ts">
  import type { Entity, Fact, HistoryEntry } from "../generated/api.ts";
  import { describe, factHistory, getPassage, patchFact, replaceEvidence, transitionFact } from "../lib/api.ts";
  import { actionsFor, formatTime, originLabel, statusLabel } from "../lib/labels.ts";
  import { href } from "../lib/route.svelte.ts";

  let {
    fact,
    entities,
    onchanged,
  }: { fact: Fact; entities: Entity[]; onchanged: () => void } = $props();

  const entityName = $derived(fact.entity_id === null ? "" : (entities.find((e) => e.id === fact.entity_id)?.name ?? "?"));
  const ACTION_LABELS = { approve: "Approve as canon", reject: "Reject", reopen: "Back to hypothesis" } as const;

  let busy = $state(false);
  let error = $state("");
  let history = $state<HistoryEntry[] | null>(null);
  let reason = $state("");
  let asking = $state<"reject" | "reopen" | null>(null);
  let editing = $state(false);
  let text = $state("");

  async function act(action: "approve" | "reject" | "reopen"): Promise<void> {
    if ((action === "reject" || action === "reopen") && asking !== action) {
      asking = action;
      return;
    }
    busy = true;
    error = "";
    try {
      await transitionFact(fact.id, action, reason.trim() === "" ? {} : { reason: reason.trim() });
      asking = null;
      reason = "";
      history = null;
      onchanged();
    } catch (e) {
      error = describe(e, "That did not work.");
    } finally {
      busy = false;
    }
  }

  async function toggleHistory(): Promise<void> {
    if (history !== null) {
      history = null;
      return;
    }
    try {
      history = (await factHistory(fact.id)).history;
    } catch (e) {
      error = describe(e, "Could not load the history.");
    }
  }

  async function stillCorrect(): Promise<void> {
    busy = true;
    error = "";
    try {
      await replaceEvidence(fact.id, fact.evidence.map((e) => e.passage_id));
      onchanged();
    } catch (e) {
      error = describe(e, "Could not confirm the evidence.");
    } finally {
      busy = false;
    }
  }

  async function saveText(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (text.trim() === "") return void (error = "The fact needs some text.");
    try {
      await patchFact(fact.id, { text: text.trim() });
      editing = false;
      onchanged();
    } catch (e) {
      error = describe(e, "Could not save the text.");
    }
  }

  /** A click on evidence opens the scene at that paragraph. */
  async function open(passageId: string): Promise<void> {
    error = "";
    try {
      const passage = await getPassage(passageId);
      if (passage.status !== "active") return void (error = "That paragraph no longer exists in the text.");
      window.location.hash = href({ name: "scene", id: passage.scene_id, passage: passageId });
    } catch (e) {
      error = describe(e, "Could not find that paragraph.");
    }
  }
</script>

<li class="card fact" data-status={fact.status}>
  <div class="row">
    <span class="badges">
      <span class="badge status {fact.status}">{statusLabel(fact.status)}</span>
      <span class="badge">by {originLabel(fact.origin)}</span>
      {#if fact.stale}<span class="badge stale" title={fact.stale_reason ?? ""}>Evidence changed</span>{/if}
      {#if entityName}<span class="badge">{entityName}</span>{/if}
    </span>
    <button type="button" class="link" onclick={() => ((editing = !editing), (text = fact.text))}>Edit</button>
  </div>

  {#if editing}
    <form class="inline" onsubmit={saveText}>
      <input type="text" bind:value={text} aria-label="Fact text" />
      <button type="submit">Save</button>
      <button type="button" class="secondary" onclick={() => (editing = false)}>Cancel</button>
    </form>
  {:else}
    <p class="fact-text">{fact.text}</p>
  {/if}

  {#if fact.stale && fact.stale_reason}<p class="muted">{fact.stale_reason}</p>{/if}
  {#if fact.approved_by}<p class="muted">Approved by {fact.approved_by}{fact.approved_at ? `, ${formatTime(fact.approved_at)}` : ""}.</p>{/if}

  {#if fact.evidence.length > 0}
    <p class="evidence">
      Evidence:
      {#each fact.evidence as ev, i (ev.passage_id)}
        <button type="button" class="chip" onclick={() => open(ev.passage_id)} disabled={ev.passage_status !== "active"}
          title={ev.passage_status !== "active" ? "This paragraph was removed from the text" : ev.changed ? "The paragraph changed since it was cited" : "Open the paragraph"}>
          {fact.evidence.length > 1 ? `Open paragraph ${i + 1}` : "Open in the text"}{ev.passage_status !== "active" ? " (removed)" : ev.changed ? " (edited)" : ""}
        </button>
      {/each}
      {#if fact.stale}
        <button type="button" class="link" onclick={stillCorrect} disabled={busy}>Evidence is still correct</button>
      {/if}
    </p>
  {:else}
    <p class="muted">No evidence cited.</p>
  {/if}

  {#if asking}
    <div class="inline">
      <input type="text" placeholder="reason (optional)" bind:value={reason} aria-label="Reason" />
      <button type="button" onclick={() => act(asking!)} disabled={busy}>{ACTION_LABELS[asking]}</button>
      <button type="button" class="secondary" onclick={() => ((asking = null), (reason = ""))}>Cancel</button>
    </div>
  {:else}
    <div class="actions">
      {#each actionsFor(fact) as action (action)}
        <button type="button" class={action === "approve" ? "" : "secondary"} onclick={() => act(action)} disabled={busy}>{ACTION_LABELS[action]}</button>
      {/each}
      <button type="button" class="link" onclick={toggleHistory}>{history === null ? "History" : "Hide history"}</button>
    </div>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#if history}
    <ol class="history">
      {#each history as h, i (i)}
        <li>
          {formatTime(h.changed_at)} — {h.from_status ? `${statusLabel(h.from_status)} → ` : "created as "}{statusLabel(h.to_status)}, by {h.changed_by}{h.reason ? ` (“${h.reason}”)` : ""}
        </li>
      {/each}
    </ol>
  {/if}
</li>
