// A scene is edited as one Markdown text (paragraphs separated by blank lines), but the server keeps
// every paragraph under a stable ID that facts and evidence point to. This module turns the edited
// text back into the passage list for a save, so that a paragraph keeps its ID when its words change,
// when something is inserted around it, or when it is moved, and says what happened to the others.
// It is pure (no DOM, no network) so that it can be tested on its own.

import type { Merge, PassageIn, PassageOut } from "../generated/api.ts";

/** The text shown in the editor for the passages of a scene. */
export function toDocument(passages: readonly PassageOut[]): string {
  return passages.map((p) => p.text).join("\n\n");
}

/** Paragraphs of an edited text: blank lines separate them, trailing spaces and empty ones go. */
export function toParagraphs(doc: string): string[] {
  const lines = doc.replace(/\r\n?/g, "\n").split("\n").map((l) => l.replace(/[ \t]+$/, ""));
  const paragraphs: string[] = [];
  let current: string[] = [];
  for (const line of lines) {
    if (line === "") {
      if (current.length > 0) paragraphs.push(current.join("\n"));
      current = [];
    } else {
      current.push(line);
    }
  }
  if (current.length > 0) paragraphs.push(current.join("\n"));
  return paragraphs;
}

const words = (text: string): Set<string> =>
  new Set(text.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? []);

/** Share of words two texts have in common, 0 to 1. */
function similarity(a: string, b: string): number {
  const wa = words(a);
  const wb = words(b);
  if (wa.size === 0 && wb.size === 0) return a === b ? 1 : 0;
  let common = 0;
  for (const w of wa) if (wb.has(w)) common++;
  return common / (wa.size + wb.size - common);
}

/** Share of the words of `part` that `whole` contains. */
function coverage(part: string, whole: string): number {
  const wp = words(part);
  if (wp.size === 0) return 0;
  const ww = words(whole);
  let common = 0;
  for (const w of wp) if (ww.has(w)) common++;
  return common / wp.size;
}

const SIMILAR = 0.34;
const MAX_GAP_CELLS = 250_000;

/** Pairs [old index, new index] of equal paragraphs that keep their order (longest common subsequence). */
function anchors(oldTexts: readonly string[], newTexts: readonly string[]): [number, number][] {
  const n = oldTexts.length;
  const m = newTexts.length;
  const table: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      table[i]![j] =
        oldTexts[i] === newTexts[j]
          ? table[i + 1]![j + 1]! + 1
          : Math.max(table[i + 1]![j]!, table[i]![j + 1]!);
    }
  }
  const pairs: [number, number][] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (oldTexts[i] === newTexts[j]) {
      pairs.push([i, j]);
      i++;
      j++;
    } else if (table[i + 1]![j]! >= table[i]![j + 1]!) {
      i++;
    } else {
      j++;
    }
  }
  return pairs;
}

/** Order-preserving pairing of the old and new paragraphs of one gap by how alike they are. */
function pairGap(
  oldTexts: readonly string[],
  newTexts: readonly string[],
  oldFrom: number,
  oldTo: number,
  newFrom: number,
  newTo: number,
): [number, number][] {
  const n = oldTo - oldFrom;
  const m = newTo - newFrom;
  if (n === 0 || m === 0 || n * m > MAX_GAP_CELLS) return [];
  const sim = (i: number, j: number): number =>
    similarity(oldTexts[oldFrom + i]!, newTexts[newFrom + j]!);
  const best: Float64Array[] = Array.from({ length: n + 1 }, () => new Float64Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      const s = sim(i, j);
      const take = s >= SIMILAR ? s + best[i + 1]![j + 1]! : -1;
      best[i]![j] = Math.max(best[i + 1]![j]!, best[i]![j + 1]!, take);
    }
  }
  const pairs: [number, number][] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    const s = sim(i, j);
    if (s >= SIMILAR && best[i]![j] === s + best[i + 1]![j + 1]!) {
      pairs.push([oldFrom + i, newFrom + j]);
      i++;
      j++;
    } else if (best[i + 1]![j]! >= best[i]![j + 1]!) {
      i++;
    } else {
      j++;
    }
  }
  return pairs;
}

export type Aligned = { passages: PassageIn[]; merged: Merge[] };

/**
 * Decides which old passage each paragraph of the edited text continues. A paragraph that continues
 * one is sent with its ID; the others are new. `merged` names old passages whose words went into
 * another passage (the server then moves their evidence there instead of flagging it).
 */
export function align(old: readonly PassageOut[], paragraphs: readonly string[]): Aligned {
  const oldTexts = old.map((p) => p.text);
  const owner: (number | null)[] = paragraphs.map(() => null); // new index -> old index
  const used = new Set<number>();
  const claim = (o: number, n: number): void => {
    owner[n] = o;
    used.add(o);
  };

  // 1. unchanged paragraphs, in order
  const fixed = anchors(oldTexts, paragraphs);
  for (const [o, n] of fixed) claim(o, n);

  // 2. in the stretches between them, paragraphs that were edited
  const bounds: [number, number][] = [[-1, -1], ...fixed, [old.length, paragraphs.length]];
  for (let k = 0; k + 1 < bounds.length; k++) {
    const [o0, n0] = bounds[k]!;
    const [o1, n1] = bounds[k + 1]!;
    for (const [o, n] of pairGap(oldTexts, paragraphs, o0 + 1, o1, n0 + 1, n1)) claim(o, n);
  }

  // 3. a paragraph moved elsewhere keeps its ID when its text is unchanged
  for (let n = 0; n < paragraphs.length; n++) {
    if (owner[n] !== null) continue;
    const o = oldTexts.findIndex((t, idx) => !used.has(idx) && t === paragraphs[n]);
    if (o >= 0) claim(o, n);
  }

  // 4. a paragraph split in two: the first part keeps the ID
  for (let n = 1; n < paragraphs.length; n++) {
    const o = owner[n] ?? null;
    if (o === null || owner[n - 1] !== null || fixed.some(([, fn]) => fn === n)) continue;
    const whole = oldTexts[o]!;
    const joined = `${paragraphs[n - 1]} ${paragraphs[n]}`;
    if (similarity(joined, whole) > similarity(paragraphs[n]!, whole)) {
      owner[n - 1] = o;
      owner[n] = null;
    }
  }

  const passages: PassageIn[] = paragraphs.map((text, n) => {
    const o = owner[n];
    const id = o === null || o === undefined ? null : old[o]!.id;
    return id === null ? { text } : { id, text };
  });

  // 5. an old passage that vanished but whose words live on in another one was merged into it
  const merged: Merge[] = [];
  old.forEach((p, o) => {
    if (used.has(o) || p.id === null) return;
    let into: string | null = null;
    let top = 0.6;
    paragraphs.forEach((text, n) => {
      const keeper = owner[n];
      if (keeper === null || keeper === undefined || old[keeper]!.id === null) return;
      const c = coverage(p.text, text);
      if (c >= top) {
        top = c;
        into = old[keeper]!.id;
      }
    });
    if (into !== null) merged.push({ from: p.id, into });
  });

  return { passages, merged };
}
