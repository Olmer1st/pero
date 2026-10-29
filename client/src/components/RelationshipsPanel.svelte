<script lang="ts">
  import type { Entity, Relationship } from "../generated/api.ts";
  import { createRelationship, deleteRelationship, describe } from "../lib/api.ts";

  let {
    bookId,
    entity,
    entities,
    relationships,
    onchanged,
  }: {
    bookId: string;
    entity: Entity;
    entities: Entity[];
    relationships: Relationship[];
    onchanged: () => void;
  } = $props();

  const name = (id: string): string => entities.find((e) => e.id === id)?.name ?? "?";
  const mine = $derived(relationships.filter((r) => r.from_entity_id === entity.id || r.to_entity_id === entity.id));

  /** "sister of Anna", "← employs Boris" (the arrow marks the other direction), "friend of Anna" for two-way ones. */
  function line(r: Relationship): string {
    const outgoing = r.from_entity_id === entity.id;
    const other = name(outgoing ? r.to_entity_id : r.from_entity_id);
    if (!r.directed) return `${r.kind} · ${other}`;
    return outgoing ? `${r.kind} → ${other}` : `${other} → ${r.kind} → this`;
  }

  let adding = $state(false);
  let other = $state("");
  let kind = $state("");
  let directed = $state(true);
  let error = $state("");

  async function add(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    error = "";
    if (other === "" || kind.trim() === "") return void (error = "Choose the other entity and say how they are related.");
    try {
      await createRelationship(bookId, { from_entity_id: entity.id, to_entity_id: other, kind: kind.trim(), directed });
      adding = false;
      other = kind = "";
      onchanged();
    } catch (e) {
      error = describe(e, "Could not add the relationship.");
    }
  }

  async function remove(r: Relationship): Promise<void> {
    if (!window.confirm("Remove this relationship?")) return;
    try {
      await deleteRelationship(r.id);
      onchanged();
    } catch (e) {
      error = describe(e, "Could not remove the relationship.");
    }
  }
</script>

<div class="relationships">
  {#if mine.length > 0}
    <ul>
      {#each mine as r (r.id)}
        <li>{line(r)} <button type="button" class="link" onclick={() => remove(r)} aria-label="Remove relationship">remove</button></li>
      {/each}
    </ul>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if adding}
    <form class="inline" onsubmit={add}>
      <input type="text" placeholder="how (e.g. sister of)" bind:value={kind} aria-label="Relationship" />
      <select bind:value={other} aria-label="Other entity">
        <option value="">— who —</option>
        {#each entities.filter((e) => e.id !== entity.id) as e (e.id)}<option value={e.id}>{e.name}</option>{/each}
      </select>
      <label class="check"><input type="checkbox" bind:checked={directed} /> one-way</label>
      <button type="submit">Add</button>
      <button type="button" class="secondary" onclick={() => (adding = false)}>Cancel</button>
    </form>
  {:else}
    <button type="button" class="link" onclick={() => (adding = true)}>+ relationship</button>
  {/if}
</div>
