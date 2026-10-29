<script lang="ts">
  import type { Chapter, SceneInfo } from "../generated/api.ts";
  import { describe, listChapters, listScenes } from "../lib/api.ts";
  import { href } from "../lib/route.svelte.ts";

  let { bookId }: { bookId: string } = $props();

  type Entry = { chapter: Chapter; scenes: SceneInfo[] };
  let entries = $state<Entry[] | null>(null);
  let error = $state("");

  $effect(() => {
    const wanted = bookId;
    entries = null;
    error = "";
    (async () => {
      try {
        const chapters = await listChapters(wanted);
        const scenes = await Promise.all(chapters.map((c) => listScenes(c.id)));
        if (wanted === bookId) entries = chapters.map((chapter, i) => ({ chapter, scenes: scenes[i] ?? [] }));
      } catch (e) {
        error = describe(e, "Could not load the chapters.");
      }
    })();
  });
</script>

{#if error}
  <p class="error" role="alert">{error}</p>
{:else if entries === null}
  <p class="muted">Loading…</p>
{:else if entries.length === 0}
  <p class="muted">This book has no chapters yet.</p>
{:else}
  <ol class="chapters">
    {#each entries as { chapter, scenes } (chapter.id)}
      <li class="card">
        <h2>{chapter.title}</h2>
        <ul class="scenes">
          {#each scenes as scene, i (scene.id)}
            <li>
              <a href={href({ name: "scene", id: scene.id })}>{scene.title || `Scene ${i + 1}`}</a>
              {#if scene.pov}<span class="muted"> · {scene.pov}</span>{/if}
            </li>
          {/each}
        </ul>
      </li>
    {/each}
  </ol>
{/if}
