<script lang="ts">
  import { FACT_STATUSES, type Entity, type Fact } from "../generated/api.ts";
  import { createFact, describe, listEntities, listFacts } from "../lib/api.ts";
  import { statusLabel } from "../lib/labels.ts";
  import FactCard from "./FactCard.svelte";

  let { bookId }: { bookId: string } = $props();

  let facts = $state<Fact[] | null>(null);
  let entities = $state<Entity[]>([]);
  let error = $state("");
  let status = $state("");
  let staleOnly = $state(false);
  let adding = $state(false);
  let newText = $state("");
  let newEntity = $state("");
  let addError = $state("");

  async function load(): Promise<void> {
    try {
      const [f, e] = await Promise.all([listFacts(bookId, { status: status || undefined, stale: staleOnly || undefined }), listEntities(bookId)]);
      facts = f;
      entities = e;
      error = "";
    } catch (e) {
      error = describe(e, "Could not load the canon.");
    }
  }
  $effect(() => {
    void bookId;
    void status;
    void staleOnly;
    void load();
  });

  async function add(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    addError = "";
    if (newText.trim() === "") return void (addError = "Write the fact first.");
    try {
      await createFact(bookId, { text: newText.trim(), entity_id: newEntity === "" ? undefined : newEntity });
      newText = "";
      newEntity = "";
      adding = false;
      await load();
    } catch (e) {
      addError = describe(e, "Could not add the fact.");
    }
  }

  const waiting = $derived((facts ?? []).filter((f) => f.status === "ai_hypothesis").length);
</script>

<p class="muted">
  Canon is what is true in your book. Nothing proposed by AI becomes canon until you approve it.
</p>

<div class="toolbar">
  <select bind:value={status} aria-label="Show">
    <option value="">All facts</option>
    {#each FACT_STATUSES as s (s)}<option value={s}>{statusLabel(s)}</option>{/each}
  </select>
  <label class="check"><input type="checkbox" bind:checked={staleOnly} /> only with changed evidence</label>
  <span class="spacer"></span>
  {#if !adding}<button type="button" onclick={() => (adding = true)}>New fact</button>{/if}
</div>

{#if adding}
  <form class="card" onsubmit={add}>
    <label for="f-text">A fact about the book</label>
    <input id="f-text" type="text" bind:value={newText} placeholder="e.g. Grigory has a scar on his left hand" />
    <label for="f-entity">About (optional)</label>
    <select id="f-entity" bind:value={newEntity}>
      <option value="">— nobody in particular —</option>
      {#each entities as e (e.id)}<option value={e.id}>{e.name}</option>{/each}
    </select>
    <p class="muted">A fact you add yourself is canon at once.</p>
    {#if addError}<p class="error" role="alert">{addError}</p>{/if}
    <div class="row">
      <button type="submit">Add</button>
      <button type="button" class="secondary" onclick={() => (adding = false)}>Cancel</button>
    </div>
  </form>
{/if}

{#if error}
  <p class="error" role="alert">{error}</p>
{:else if facts === null}
  <p class="muted">Loading…</p>
{:else if facts.length === 0}
  <p class="muted">{status || staleOnly ? "No facts match." : "No facts yet."}</p>
{:else}
  {#if waiting > 0}<p class="notice">{waiting} waiting for your decision.</p>{/if}
  <ul class="cards">
    {#each facts as fact (fact.id)}
      <FactCard {fact} {entities} onchanged={load} />
    {/each}
  </ul>
{/if}
