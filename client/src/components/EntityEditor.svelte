<script lang="ts">
  import type { Entity } from "../generated/api.ts";
  import { ENTITY_KINDS } from "../generated/api.ts";
  import { kindLabel, parseAliases } from "../lib/labels.ts";

  let {
    initial,
    onsave,
    oncancel,
  }: {
    initial?: Entity;
    onsave: (v: { kind: string; name: string; description: string; aliases: string[] }) => Promise<string | null>;
    oncancel: () => void;
  } = $props();

  // The form starts from the entity it edits; it does not follow later changes to it.
  const start = (() => initial)();
  let kind = $state(start?.kind ?? "character");
  let name = $state(start?.name ?? "");
  let description = $state(start?.description ?? "");
  let aliases = $state((start?.aliases ?? []).join(", "));
  let error = $state("");
  let busy = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (name.trim() === "") return void (error = "A name is required.");
    busy = true;
    error = (await onsave({ kind, name: name.trim(), description: description.trim(), aliases: parseAliases(aliases) })) ?? "";
    busy = false;
  }
</script>

<form class="card" onsubmit={submit}>
  {#if !initial}
    <label for="e-kind">Kind</label>
    <select id="e-kind" bind:value={kind}>
      {#each ENTITY_KINDS as k (k)}<option value={k}>{kindLabel(k)}</option>{/each}
    </select>
  {/if}
  <label for="e-name">Name</label>
  <input id="e-name" type="text" bind:value={name} />
  <label for="e-aliases">Other names (separated by commas)</label>
  <input id="e-aliases" type="text" bind:value={aliases} />
  <label for="e-desc">Description</label>
  <textarea id="e-desc" rows="4" bind:value={description}></textarea>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="row">
    <button type="submit" disabled={busy}>{busy ? "Saving…" : "Save"}</button>
    <button type="button" class="secondary" onclick={oncancel} disabled={busy}>Cancel</button>
  </div>
</form>
