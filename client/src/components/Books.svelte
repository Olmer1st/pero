<script lang="ts">
  import type { Book } from "../generated/api.ts";
  import { ApiError, listBooks } from "../lib/api.ts";
  import { href } from "../lib/route.svelte.ts";
  import ImportBook from "./ImportBook.svelte";

  let books = $state<Book[] | null>(null);
  let error = $state("");
  let importing = $state(false);

  async function load(): Promise<void> {
    try {
      books = await listBooks();
      error = "";
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return; // the session signs out
      error = e instanceof ApiError ? e.message : "Could not load the books.";
    }
  }

  $effect(() => {
    void load();
  });

  const languageName = (code: string): string =>
    ({ ru: "Russian", en: "English" })[code] ?? code;

  const details = (book: Book): string =>
    [languageName(book.language), book.head_commit && `version ${book.head_commit.slice(0, 7)}`]
      .filter(Boolean)
      .join(" · ");

  function imported(): void {
    importing = false;
    void load();
  }
</script>

<section>
  <div class="row">
    <h1>Books</h1>
    {#if !importing}
      <button type="button" class="secondary" onclick={() => (importing = true)}>
        Import chapters
      </button>
    {/if}
  </div>

  {#if importing}
    <ImportBook ondone={imported} oncancel={() => (importing = false)} />
  {/if}

  {#if error}
    <p class="error" role="alert">{error}</p>
  {:else if books === null}
    <p class="muted">Loading…</p>
  {:else if books.length === 0 && importing}
    <!-- the import form above is the next step -->
  {:else if books.length === 0}
    <p class="muted">
      There are no books yet. Use “Import chapters” to bring in Markdown files, one file per
      chapter.
    </p>
  {:else}
    <ul class="books">
      {#each books as book (book.id)}
        <li class="card">
          <a class="title" href={href({ name: "book", id: book.id })}>{book.title}</a>
          <span class="muted">{details(book)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>
