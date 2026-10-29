<script lang="ts">
  import type { Scene } from "../generated/api.ts";
  import { ApiError, getScene, saveScene } from "../lib/api.ts";
  import { align, toDocument, toParagraphs } from "../lib/passages.ts";
  import { href, router } from "../lib/route.svelte.ts";
  import Editor from "./Editor.svelte";

  let { id }: { id: string } = $props();

  type Meta = { title: string; pov: string; date: string; location: string };
  const metaOf = (s: Scene): Meta => ({
    title: s.title,
    pov: s.pov ?? "",
    date: s.date ?? "",
    location: s.location ?? "",
  });

  let scene = $state<Scene | null>(null);
  let loadError = $state("");
  let doc = $state("");
  let savedDoc = $state("");
  let meta = $state<Meta>({ title: "", pov: "", date: "", location: "" });
  let savedMeta = $state<Meta>({ title: "", pov: "", date: "", location: "" });
  let saving = $state(false);
  let lastCommit = $state<string | null>(null);
  let saveError = $state("");
  let conflict = $state<Scene | null>(null);
  let editor = $state<Editor>();

  const dirty = $derived(
    scene !== null &&
      (doc !== savedDoc ||
        meta.title !== savedMeta.title ||
        meta.pov !== savedMeta.pov ||
        meta.date !== savedMeta.date ||
        meta.location !== savedMeta.location),
  );

  function adopt(s: Scene): void {
    scene = s;
    doc = savedDoc = toDocument(s.passages);
    meta = metaOf(s);
    savedMeta = metaOf(s);
  }

  $effect(() => {
    const wanted = id;
    scene = null;
    loadError = "";
    conflict = null;
    getScene(wanted)
      .then((s) => {
        if (wanted === id) adopt(s);
      })
      .catch((e: unknown) => {
        if (e instanceof ApiError && e.status === 401) return; // the session signs out
        loadError = e instanceof ApiError ? e.message : "Could not load the scene.";
      });
  });

  // Leaving with unsaved changes asks first (links, back button, closing the tab, signing out).
  $effect(() => {
    router.guard = () => !dirty || window.confirm("You have unsaved changes. Leave without saving?");
    const warn = (e: BeforeUnloadEvent): void => {
      if (dirty) e.preventDefault();
    };
    window.addEventListener("beforeunload", warn);
    return () => {
      router.guard = null;
      window.removeEventListener("beforeunload", warn);
    };
  });

  $effect(() => {
    document.title = `${dirty ? "● " : ""}${meta.title || "Scene"} — Pero`;
    return () => {
      document.title = "Pero";
    };
  });

  const optional = (text: string): string | undefined => (text.trim() === "" ? undefined : text.trim());

  async function save(base: Scene | null = scene): Promise<void> {
    if (base === null || saving) return;
    saving = true;
    saveError = "";
    const sentDoc = doc;
    const sentMeta = { ...meta };
    const { passages, merged } = align(base.passages, toParagraphs(sentDoc));
    try {
      const saved = await saveScene(id, {
        base_revision: base.revision,
        fields: {
          title: sentMeta.title.trim(),
          language: base.language,
          pov: optional(sentMeta.pov),
          date: optional(sentMeta.date),
          location: optional(sentMeta.location),
        },
        passages,
        merged,
      });
      scene = saved.scene;
      savedDoc = sentDoc;
      savedMeta = sentMeta;
      conflict = null;
      if (saved.commit) lastCommit = saved.commit;
    } catch (e) {
      if (e instanceof ApiError && e.code === "revision_conflict" && e.body?.current) {
        conflict = e.body.current;
      } else if (e instanceof ApiError && e.status === 401) {
        // the session signs out
      } else {
        saveError = e instanceof ApiError ? e.message : "The save failed.";
      }
    } finally {
      saving = false;
    }
  }

  function takeTheirs(): void {
    if (conflict === null) return;
    if (!window.confirm("Discard your changes and load the other version?")) return;
    const theirs = conflict;
    adopt(theirs);
    editor?.setText(doc);
    conflict = null;
  }

  async function copyMine(): Promise<void> {
    try {
      await navigator.clipboard.writeText(doc);
    } catch {
      saveError = "Could not copy; select the text in the editor instead.";
    }
  }

  const status = $derived(
    saving ? "Saving…" : dirty ? "Unsaved changes" : lastCommit ? `Saved · ${lastCommit.slice(0, 7)}` : "Saved",
  );
</script>

{#if loadError}
  <p class="error" role="alert">{loadError}</p>
{:else if scene === null}
  <p class="muted">Loading…</p>
{:else}
  <nav class="crumbs" aria-label="Where you are">
    <a href={href({ name: "books" })}>Books</a> ›
    <a href={href({ name: "book", id: scene.book_id })}>Book</a> ›
    <span>{meta.title || "Untitled scene"}</span>
  </nav>

  <div class="toolbar">
    <span class="state" class:dirty role="status" aria-live="polite">{status}</span>
    <span class="spacer"></span>
    <button type="button" onclick={() => save()} disabled={saving || !dirty}>Save</button>
  </div>

  {#if conflict}
    <div class="notice" role="alert">
      <p>
        <strong>This scene was changed somewhere else</strong> (another device, or by hand in the
        repository) after you opened it. Your text has not been saved.
      </p>
      <div class="row">
        <button type="button" onclick={() => save(conflict)} disabled={saving}>Keep mine and overwrite</button>
        <button type="button" class="secondary" onclick={takeTheirs}>Load the other version</button>
        <button type="button" class="secondary" onclick={copyMine}>Copy my text</button>
      </div>
    </div>
  {/if}
  {#if saveError}
    <p class="error" role="alert">{saveError}</p>
  {/if}

  <form class="meta" onsubmit={(e) => e.preventDefault()}>
    <label>Title <input type="text" bind:value={meta.title} /></label>
    <label>Point of view <input type="text" bind:value={meta.pov} /></label>
    <label>Date <input type="text" bind:value={meta.date} /></label>
    <label>Place <input type="text" bind:value={meta.location} /></label>
  </form>

  <Editor
    bind:this={editor}
    value={doc}
    language={scene.language}
    onchange={(text) => (doc = text)}
    onsave={() => save()}
  />
  <p class="muted hint">
    Blank lines separate paragraphs. Each paragraph keeps its identity while you edit it, so facts
    that cite it stay attached. Ctrl+S saves.
  </p>
{/if}
