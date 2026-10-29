<script lang="ts">
  // CodeMirror 6 over the scene text: Markdown, plain wrapping lines, the browser's own spell check
  // (right-click suggestions come from the operating system), undo history, Ctrl+S / Cmd+S to save.
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { markdown } from "@codemirror/lang-markdown";
  import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
  import { EditorState } from "@codemirror/state";
  import { untrack } from "svelte";
  import { EditorView, keymap } from "@codemirror/view";
  import { tags } from "@lezer/highlight";

  let {
    value,
    language,
    onchange,
    onsave,
  }: {
    value: string;
    language: string;
    onchange: (doc: string) => void;
    onsave: () => void;
  } = $props();

  /** The server writes a fresh nonce into the page; the editor's own `<style>` element must carry it. */
  const nonce = ((): string | undefined => {
    const value = document.querySelector<HTMLMetaElement>('meta[name="csp-nonce"]')?.content;
    return value && !value.startsWith("__") ? value : undefined;
  })();

  let host: HTMLDivElement;
  let view: EditorView | undefined;

  const highlight = HighlightStyle.define([
    { tag: tags.heading, fontWeight: "700" },
    { tag: tags.strong, fontWeight: "700" },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.quote, color: "var(--muted)" },
    { tag: tags.link, color: "var(--accent)" },
    { tag: tags.monospace, fontFamily: "ui-monospace, monospace" },
    { tag: tags.processingInstruction, color: "var(--muted)" },
  ]);

  const theme = EditorView.theme({
    "&": { color: "var(--fg)", backgroundColor: "var(--card)", fontSize: "1.05rem" },
    ".cm-content": { fontFamily: "Georgia, 'Noto Serif', serif", lineHeight: "1.7", padding: "1rem 1.25rem", caretColor: "var(--fg)" },
    ".cm-scroller": { minHeight: "50vh" },
    "&.cm-focused": { outline: "2px solid var(--accent)" },
    ".cm-cursor": { borderLeftColor: "var(--fg)" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--accent) 30%, transparent)" },
  });

  $effect(() => {
    // Created once per mount. The text, language and callbacks are read once here on purpose: later
    // text changes go through `setText`, and reading them as dependencies would rebuild the editor
    // (cursor, focus and undo history lost) on every keystroke.
    const initial = untrack(() => ({ doc: value, lang: language }));
    const state = EditorState.create({
      doc: initial.doc,
      extensions: [
        history(),
        keymap.of([
          { key: "Mod-s", preventDefault: true, run: () => (untrack(onsave), true) },
          ...defaultKeymap,
          ...historyKeymap,
        ]),
        markdown(),
        syntaxHighlighting(highlight),
        ...(nonce ? [EditorView.cspNonce.of(nonce)] : []),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: "true", lang: initial.lang, autocapitalize: "sentences" }),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) untrack(() => onchange(u.state.doc.toString()));
        }),
        theme,
      ],
    });
    view = new EditorView({ state, parent: host });
    return () => {
      view?.destroy();
      view = undefined;
    };
  });

  /** Replaces the whole text (loading the other version after a conflict). Not an undo step. */
  export function setText(text: string): void {
    if (!view) return;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
  }

  /** Scrolls to a stretch of the text and selects it (a paragraph opened from the canon ledger). */
  export function reveal(from: number, to: number): void {
    if (!view) return;
    view.dispatch({
      selection: { anchor: from, head: to },
      effects: EditorView.scrollIntoView(from, { y: "center" }),
    });
    view.focus();
  }

  export function focus(): void {
    view?.focus();
  }
</script>

<div class="editor" bind:this={host}></div>
