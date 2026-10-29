<script lang="ts">
  import type { Entity, Event, Participant, SceneInfo } from "../generated/api.ts";
  import type { EventValues } from "../lib/forms.ts";

  type SceneChoice = { scene: SceneInfo; label: string };

  let {
    initial,
    entities,
    scenes,
    onsave,
    oncancel,
  }: {
    initial?: Event;
    entities: Entity[];
    scenes: SceneChoice[];
    onsave: (v: EventValues) => Promise<string | null>;
    oncancel: () => void;
  } = $props();

  // The form starts from the event it edits; it does not follow later changes to it.
  const start = (() => initial)();
  let title = $state(start?.title ?? "");
  let when = $state(start?.when_text ?? "");
  let description = $state(start?.description ?? "");
  let sceneId = $state(start?.scene_id ?? "");
  let placeId = $state(start?.place_id ?? "");
  let people = $state<Participant[]>((start?.participants ?? []).map((p) => ({ ...p })));
  let error = $state("");
  let busy = $state(false);

  const places = $derived(entities.filter((e) => e.kind === "place"));

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (title.trim() === "") return void (error = "A title is required.");
    const chosen = people.filter((p) => p.entity_id !== "");
    if (new Set(chosen.map((p) => p.entity_id)).size !== chosen.length) return void (error = "Each person can take part once.");
    busy = true;
    error =
      (await onsave({
        title: title.trim(),
        description: description.trim(),
        when_text: when.trim() === "" ? null : when.trim(),
        scene_id: sceneId === "" ? null : sceneId,
        place_id: placeId === "" ? null : placeId,
        participants: chosen.map((p) => ({ entity_id: p.entity_id, role: (p.role ?? "").trim() })),
      })) ?? "";
    busy = false;
  }
</script>

<form class="card" onsubmit={submit}>
  <label for="ev-title">What happens</label>
  <input id="ev-title" type="text" bind:value={title} />
  <label for="ev-when">When (as you write dates in the book)</label>
  <input id="ev-when" type="text" bind:value={when} placeholder="e.g. spring 1987" />
  <label for="ev-desc">Details</label>
  <textarea id="ev-desc" rows="3" bind:value={description}></textarea>
  <label for="ev-scene">Scene</label>
  <select id="ev-scene" bind:value={sceneId}>
    <option value="">— none —</option>
    {#each scenes as s (s.scene.id)}<option value={s.scene.id}>{s.label}</option>{/each}
  </select>
  <label for="ev-place">Place</label>
  <select id="ev-place" bind:value={placeId}>
    <option value="">— none —</option>
    {#each places as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
  </select>
  <span class="label">Who takes part</span>
  {#each people as p, i (i)}
    <div class="inline">
      <select bind:value={p.entity_id} aria-label="Person">
        <option value="">— who —</option>
        {#each entities.filter((e) => e.kind !== "place") as e (e.id)}<option value={e.id}>{e.name}</option>{/each}
      </select>
      <input type="text" placeholder="role (optional)" bind:value={p.role} aria-label="Role" />
      <button type="button" class="link" onclick={() => (people = people.filter((_, j) => j !== i))}>remove</button>
    </div>
  {/each}
  <button type="button" class="link" onclick={() => (people = [...people, { entity_id: "", role: "" }])}>+ person</button>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="row">
    <button type="submit" disabled={busy}>{busy ? "Saving…" : "Save"}</button>
    <button type="button" class="secondary" onclick={oncancel} disabled={busy}>Cancel</button>
  </div>
</form>
