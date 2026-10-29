<script lang="ts">
  import type { Entity, Relationship } from "../generated/api.ts";
  import { ApiError, createEntity, deleteEntity, describe, listEntities, listRelationships, patchEntity } from "../lib/api.ts";
  import { kindLabel } from "../lib/labels.ts";
  import EntityEditor from "./EntityEditor.svelte";
  import RelationshipsPanel from "./RelationshipsPanel.svelte";

  let { bookId }: { bookId: string } = $props();

  let entities = $state<Entity[] | null>(null);
  let relationships = $state<Relationship[]>([]);
  let error = $state("");
  let notice = $state("");
  let kindFilter = $state("");
  let search = $state("");
  let creating = $state(false);
  let editing = $state<string | null>(null);

  async function load(): Promise<void> {
    try {
      [entities, relationships] = await Promise.all([listEntities(bookId), listRelationships(bookId)]);
      error = "";
    } catch (e) {
      error = describe(e, "Could not load the characters and places.");
    }
  }
  $effect(() => {
    void bookId;
    entities = null;
    void load();
  });

  const visible = $derived(
    (entities ?? []).filter((e) => {
      if (kindFilter !== "" && e.kind !== kindFilter) return false;
      const needle = search.trim().toLowerCase();
      return needle === "" || [e.name, ...e.aliases].some((n) => n.toLowerCase().includes(needle));
    }),
  );

  async function create(v: { kind: string; name: string; description: string; aliases: string[] }): Promise<string | null> {
    try {
      await createEntity(bookId, v);
      creating = false;
      await load();
      return null;
    } catch (e) {
      return describe(e, "Could not create it.");
    }
  }

  async function save(id: string, v: { name: string; description: string; aliases: string[] }): Promise<string | null> {
    try {
      await patchEntity(id, { name: v.name, description: v.description, aliases: v.aliases });
      editing = null;
      await load();
      return null;
    } catch (e) {
      return describe(e, "Could not save the changes.");
    }
  }

  async function remove(entity: Entity): Promise<void> {
    notice = "";
    if (!window.confirm(`Delete “${entity.name}”?`)) return;
    try {
      await deleteEntity(entity.id);
      await load();
    } catch (e) {
      if (e instanceof ApiError && e.code === "in_use") {
        if (window.confirm(`${e.message}\n\nDelete it anyway, together with those relationships and event links?`)) {
          try {
            await deleteEntity(entity.id, true);
            await load();
          } catch (e2) {
            notice = describe(e2, "Could not delete it.");
          }
        }
      } else {
        notice = describe(e, "Could not delete it.");
      }
    }
  }
</script>

<div class="toolbar">
  <select bind:value={kindFilter} aria-label="Show">
    <option value="">All</option>
    <option value="character">Characters</option>
    <option value="place">Places</option>
    <option value="organization">Organizations</option>
  </select>
  <input type="search" placeholder="Search names…" bind:value={search} aria-label="Search names" />
  <span class="spacer"></span>
  {#if !creating}<button type="button" onclick={() => (creating = true)}>New</button>{/if}
</div>

{#if creating}
  <EntityEditor onsave={create} oncancel={() => (creating = false)} />
{/if}
{#if notice}<p class="error" role="alert">{notice}</p>{/if}

{#if error}
  <p class="error" role="alert">{error}</p>
{:else if entities === null}
  <p class="muted">Loading…</p>
{:else if entities.length === 0 && !creating}
  <p class="muted">Nothing here yet. Add the characters, places and organizations of the book with “New”.</p>
{:else}
  <ul class="cards">
    {#each visible as entity (entity.id)}
      <li class="card">
        {#if editing === entity.id}
          <EntityEditor initial={entity} onsave={(v) => save(entity.id, v)} oncancel={() => (editing = null)} />
        {:else}
          <div class="row">
            <h2>{entity.name} <span class="badge">{kindLabel(entity.kind)}</span></h2>
            <span>
              <button type="button" class="link" onclick={() => (editing = entity.id)}>Edit</button>
              <button type="button" class="link danger" onclick={() => remove(entity)}>Delete</button>
            </span>
          </div>
          {#if entity.aliases.length > 0}<p class="muted">Also: {entity.aliases.join(", ")}</p>{/if}
          {#if entity.description}<p class="description">{entity.description}</p>{/if}
          <RelationshipsPanel {bookId} {entity} entities={entities ?? []} {relationships} onchanged={load} />
        {/if}
      </li>
    {:else}
      <li class="muted">No match.</li>
    {/each}
  </ul>
{/if}
