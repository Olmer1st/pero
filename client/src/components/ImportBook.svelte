<script lang="ts">
  import type { ImportIssue, ImportSummary, ParagraphMode } from "../generated/api.ts";
  import { ApiError, importBook } from "../lib/api.ts";

  let { ondone, oncancel }: { ondone: () => void; oncancel: () => void } = $props();

  let title = $state("");
  let language = $state("ru");
  let mode = $state<ParagraphMode>("blank_line");
  let files = $state<File[]>([]);
  let busy = $state(false);
  let error = $state("");
  let issues = $state<ImportIssue[]>([]);
  let summary = $state<ImportSummary | null>(null);

  function chosen(event: Event): void {
    const input = event.currentTarget as HTMLInputElement;
    files = Array.from(input.files ?? []).sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { numeric: true }),
    );
  }

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    error = "";
    issues = [];
    if (title.trim() === "") return void (error = "Give the book a title.");
    if (files.length === 0) return void (error = "Choose the chapter files (.md).");
    busy = true;
    try {
      const contents = await Promise.all(
        files.map(async (f) => ({ name: f.name, content: await f.text() })),
      );
      summary = await importBook({ title: title.trim(), language, mode, files: contents });
    } catch (e) {
      if (e instanceof ApiError) {
        error = e.message;
        issues = e.body?.issues ?? [];
      } else {
        error = "Could not read the files.";
      }
    } finally {
      busy = false;
    }
  }
</script>

<section class="card">
  {#if summary}
    <h2>Imported “{summary.book.title}”</h2>
    <p>
      {summary.chapters} chapters, {summary.scenes} scenes, {summary.passages} paragraphs.
    </p>
    {#if summary.titles_from_filename.length > 0}
      <p class="muted">
        These files had no “# ” heading, so the chapter title was made from the file name:
        {summary.titles_from_filename.join(", ")}.
      </p>
    {/if}
    <button type="button" onclick={ondone}>Done</button>
  {:else}
    <h2>Import chapters</h2>
    <p class="muted">
      One Markdown file per chapter. Nothing is imported unless every file is understood; nothing is
      ever changed in your files.
    </p>
    <form onsubmit={submit}>
      <label for="book-title">Book title</label>
      <input id="book-title" type="text" bind:value={title} />

      <label for="book-language">Language</label>
      <select id="book-language" bind:value={language}>
        <option value="ru">Russian</option>
        <option value="en">English</option>
      </select>

      <label for="book-mode">Paragraphs</label>
      <select id="book-mode" bind:value={mode}>
        <option value="blank_line">Separated by blank lines (usual Markdown)</option>
        <option value="line">Every line is a paragraph</option>
      </select>

      <label for="book-files">Chapter files</label>
      <input id="book-files" type="file" accept=".md,text/markdown" multiple onchange={chosen} />
      {#if files.length > 0}
        <p class="muted">{files.length} files, in this order: {files.map((f) => f.name).join(", ")}</p>
      {/if}

      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
      {#if issues.length > 0}
        <ul class="issues">
          {#each issues as issue}
            <li>{issue.file}{issue.line > 0 ? `:${issue.line}` : ""} — {issue.message}</li>
          {/each}
        </ul>
      {/if}

      <div class="row">
        <button type="submit" disabled={busy}>{busy ? "Importing…" : "Import"}</button>
        <button type="button" class="secondary" onclick={oncancel} disabled={busy}>Cancel</button>
      </div>
    </form>
  {/if}
</section>
