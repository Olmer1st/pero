<script lang="ts">
  import type { Entity, Event, SceneInfo } from "../generated/api.ts";
  import { createEvent, deleteEvent, describe, listChapters, listEntities, listEvents, listScenes, patchEvent } from "../lib/api.ts";
  import type { EventValues } from "../lib/forms.ts";
  import { href } from "../lib/route.svelte.ts";
  import EventEditor from "./EventEditor.svelte";

  let { bookId }: { bookId: string } = $props();

  type SceneChoice = { scene: SceneInfo; label: string };
  let events = $state<Event[] | null>(null);
  let entities = $state<Entity[]>([]);
  let scenes = $state<SceneChoice[]>([]);
  let error = $state("");
  let notice = $state("");
  let creating = $state(false);
  let editing = $state<string | null>(null);

  async function load(): Promise<void> {
    try {
      const [ev, en, chapters] = await Promise.all([listEvents(bookId), listEntities(bookId), listChapters(bookId)]);
      const perChapter = await Promise.all(chapters.map((c) => listScenes(c.id)));
      events = ev;
      entities = en;
      scenes = chapters.flatMap((c, i) =>
        (perChapter[i] ?? []).map((scene, j) => ({ scene, label: `${c.title} › ${scene.title || `Scene ${j + 1}`}` })),
      );
      error = "";
    } catch (e) {
      error = describe(e, "Could not load the timeline.");
    }
  }
  $effect(() => {
    void bookId;
    events = null;
    void load();
  });

  const name = (id: string | null): string => (id === null ? "" : (entities.find((e) => e.id === id)?.name ?? "?"));
  const sceneLabel = (id: string | null): string => scenes.find((s) => s.scene.id === id)?.label ?? "scene";


  async function create(v: EventValues): Promise<string | null> {
    try {
      await createEvent(bookId, {
        title: v.title,
        description: v.description,
        when_text: v.when_text ?? undefined,
        scene_id: v.scene_id ?? undefined,
        place_id: v.place_id ?? undefined,
        participants: v.participants,
      });
      creating = false;
      await load();
      return null;
    } catch (e) {
      return describe(e, "Could not add the event.");
    }
  }

  async function save(id: string, v: EventValues): Promise<string | null> {
    try {
      await patchEvent(id, v);
      editing = null;
      await load();
      return null;
    } catch (e) {
      return describe(e, "Could not save the event.");
    }
  }

  async function remove(event: Event): Promise<void> {
    notice = "";
    if (!window.confirm(`Delete “${event.title}” from the timeline?`)) return;
    try {
      await deleteEvent(event.id);
      await load();
    } catch (e) {
      notice = describe(e, "Could not delete the event.");
    }
  }

  /** Swaps the order position with the neighbour (the author's order is the `sort_order` number). */
  async function move(index: number, by: -1 | 1): Promise<void> {
    notice = "";
    const list = events ?? [];
    const a = list[index];
    const b = list[index + by];
    if (!a || !b) return;
    try {
      if (a.sort_order === b.sort_order) {
        await patchEvent(a.id, { sort_order: by === -1 ? b.sort_order - 1 : b.sort_order + 1 });
      } else {
        await patchEvent(a.id, { sort_order: b.sort_order });
        await patchEvent(b.id, { sort_order: a.sort_order });
      }
      await load();
    } catch (e) {
      notice = describe(e, "Could not move the event.");
      await load();
    }
  }
</script>

<div class="toolbar">
  <span class="muted">In story order. Use the arrows to reorder.</span>
  <span class="spacer"></span>
  {#if !creating}<button type="button" onclick={() => (creating = true)}>New event</button>{/if}
</div>

{#if creating}
  <EventEditor {entities} {scenes} onsave={create} oncancel={() => (creating = false)} />
{/if}
{#if notice}<p class="error" role="alert">{notice}</p>{/if}

{#if error}
  <p class="error" role="alert">{error}</p>
{:else if events === null}
  <p class="muted">Loading…</p>
{:else if events.length === 0 && !creating}
  <p class="muted">The timeline is empty. Add the events of the story with “New event”.</p>
{:else}
  <ol class="cards timeline">
    {#each events as event, i (event.id)}
      <li class="card">
        {#if editing === event.id}
          <EventEditor initial={event} {entities} {scenes} onsave={(v) => save(event.id, v)} oncancel={() => (editing = null)} />
        {:else}
          <div class="row">
            <h2>
              {#if event.when_text}<span class="when">{event.when_text}</span>{/if}
              {event.title}
            </h2>
            <span>
              <button type="button" class="link" onclick={() => move(i, -1)} disabled={i === 0} aria-label="Move earlier">↑</button>
              <button type="button" class="link" onclick={() => move(i, 1)} disabled={i === events.length - 1} aria-label="Move later">↓</button>
              <button type="button" class="link" onclick={() => (editing = event.id)}>Edit</button>
              <button type="button" class="link danger" onclick={() => remove(event)}>Delete</button>
            </span>
          </div>
          {#if event.description}<p class="description">{event.description}</p>{/if}
          <p class="muted">
            {#if event.place_id}At {name(event.place_id)}. {/if}
            {#if event.participants.length > 0}
              With {event.participants.map((p) => (p.role ? `${name(p.entity_id)} (${p.role})` : name(p.entity_id))).join(", ")}.
            {/if}
            {#if event.scene_id}<a href={href({ name: "scene", id: event.scene_id })}>{sceneLabel(event.scene_id)}</a>{/if}
          </p>
        {/if}
      </li>
    {/each}
  </ol>
{/if}
