<script lang="ts">
  import type { Book } from "../generated/api.ts";
  import { describe, getBook } from "../lib/api.ts";
  import { href, type Tab } from "../lib/route.svelte.ts";
  import CanonTab from "./CanonTab.svelte";
  import ChaptersTab from "./ChaptersTab.svelte";
  import EntitiesTab from "./EntitiesTab.svelte";
  import TimelineTab from "./TimelineTab.svelte";

  let { id, tab }: { id: string; tab: Tab } = $props();

  let book = $state<Book | null>(null);
  let error = $state("");

  const TABS: { tab: Tab; label: string }[] = [
    { tab: "chapters", label: "Chapters" },
    { tab: "entities", label: "Characters & places" },
    { tab: "timeline", label: "Timeline" },
    { tab: "canon", label: "Canon" },
  ];

  $effect(() => {
    const wanted = id;
    book = null;
    error = "";
    getBook(wanted)
      .then((b) => {
        if (wanted === id) book = b;
      })
      .catch((e: unknown) => (error = describe(e, "Could not load the book.")));
  });
</script>

<nav class="crumbs" aria-label="Where you are">
  <a href={href({ name: "books" })}>Books</a> › <span>{book?.title ?? "…"}</span>
</nav>

{#if error}
  <p class="error" role="alert">{error}</p>
{:else}
  <h1>{book?.title ?? ""}</h1>
  <nav class="tabs" aria-label="Sections of the book">
    {#each TABS as t (t.tab)}
      <a href={href({ name: "book", id, tab: t.tab })} class:active={tab === t.tab} aria-current={tab === t.tab ? "page" : undefined}>{t.label}</a>
    {/each}
  </nav>

  {#if tab === "entities"}
    <EntitiesTab bookId={id} />
  {:else if tab === "timeline"}
    <TimelineTab bookId={id} />
  {:else if tab === "canon"}
    <CanonTab bookId={id} />
  {:else}
    <ChaptersTab bookId={id} />
  {/if}
{/if}
