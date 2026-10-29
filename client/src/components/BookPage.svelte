<script lang="ts">
  import type { Book, Chapter, SceneInfo } from "../generated/api.ts";
  import { ApiError, getBook, listChapters, listScenes } from "../lib/api.ts";
  import { href } from "../lib/route.svelte.ts";

  let { id }: { id: string } = $props();

  type Entry = { chapter: Chapter; scenes: SceneInfo[] };
  let book = $state<Book | null>(null);
  let entries = $state<Entry[] | null>(null);
  let error = $state("");

  $effect(() => {
    const wanted = id;
    book = null;
    entries = null;
    error = "";
    (async () => {
      try {
        const [b, chapters] = await Promise.all([getBook(wanted), listChapters(wanted)]);
        const scenes = await Promise.all(chapters.map((c) => listScenes(c.id)));
        if (wanted !== id) return;
        book = b;
        entries = chapters.map((chapter, i) => ({ chapter, scenes: scenes[i] ?? [] }));
      } catch (e) {
        if (e instanceof ApiError && e.status === 401) return; // the session signs out
        error = e instanceof ApiError ? e.message : "Could not load the book.";
      }
    })();
  });
</script>

<nav class="crumbs" aria-label="Where you are">
  <a href={href({ name: "books" })}>Books</a> › <span>{book?.title ?? "…"}</span>
</nav>

{#if error}
  <p class="error" role="alert">{error}</p>
{:else if entries === null}
  <p class="muted">Loading…</p>
{:else if entries.length === 0}
  <p class="muted">This book has no chapters yet.</p>
{:else}
  <h1>{book?.title}</h1>
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
