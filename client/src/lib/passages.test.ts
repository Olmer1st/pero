import assert from "node:assert/strict";
import { test } from "node:test";

import type { PassageOut } from "../generated/api.ts";
import { align, toDocument, toParagraphs } from "./passages.ts";

const P = (id: string | null, text: string): PassageOut => ({ id, text });
const ids = (a: ReturnType<typeof align>) => a.passages.map((p) => p.id ?? null);

const old = [
  P("aaaaaaaa", "Григорий вошёл в тёмную комнату и закрыл за собой дверь."),
  P("bbbbbbbb", "— Ты опоздал, — сказала она, не оборачиваясь."),
  P("cccccccc", "Он промолчал и сел у окна, глядя на мокрую улицу."),
];

test("the text round-trips through the editor and back", () => {
  const doc = toDocument(old);
  assert.equal(toParagraphs(doc).length, 3);
  assert.deepEqual(toParagraphs(doc), old.map((p) => p.text));
});

test("blank lines separate paragraphs; trailing spaces, extra blank lines and CRLF are normalised", () => {
  assert.deepEqual(toParagraphs("раз  \r\nдва\r\n\r\n\r\n\n  \nтри\n"), ["раз\nдва", "три"]);
  assert.deepEqual(toParagraphs("   \n\n"), []);
});

test("unchanged text keeps every ID and reports nothing else", () => {
  const a = align(old, toParagraphs(toDocument(old)));
  assert.deepEqual(ids(a), ["aaaaaaaa", "bbbbbbbb", "cccccccc"]);
  assert.deepEqual(a.merged, []);
});

test("editing the words of a paragraph keeps its ID", () => {
  const edited = toParagraphs(
    "Григорий вошёл в тёмную комнату и тихо закрыл за собой дверь.\n\n— Ты опоздал, — сказала она, не оборачиваясь.\n\nОн промолчал и сел у окна, глядя на мокрую улицу.",
  );
  assert.deepEqual(ids(align(old, edited)), ["aaaaaaaa", "bbbbbbbb", "cccccccc"]);
});

test("a new paragraph in the middle gets no ID; the others keep theirs", () => {
  const a = align(old, [old[0]!.text, "Совсем новый абзац о другом.", old[1]!.text, old[2]!.text]);
  assert.deepEqual(ids(a), ["aaaaaaaa", null, "bbbbbbbb", "cccccccc"]);
});

test("a deleted paragraph is simply absent from the list", () => {
  const a = align(old, [old[0]!.text, old[2]!.text]);
  assert.deepEqual(ids(a), ["aaaaaaaa", "cccccccc"]);
  assert.deepEqual(a.merged, []);
});

test("a paragraph moved elsewhere keeps its ID", () => {
  const a = align(old, [old[2]!.text, old[0]!.text, old[1]!.text]);
  assert.deepEqual(ids(a), ["cccccccc", "aaaaaaaa", "bbbbbbbb"]);
});

test("a paragraph split in two: the first part keeps the ID, the second is new", () => {
  const a = align(old, [
    "Григорий вошёл в тёмную комнату",
    "и закрыл за собой дверь.",
    old[1]!.text,
    old[2]!.text,
  ]);
  assert.deepEqual(ids(a), ["aaaaaaaa", null, "bbbbbbbb", "cccccccc"]);
});

test("two paragraphs merged into one: the first ID stays and the other is reported as merged", () => {
  const a = align(old, [`${old[0]!.text} ${old[1]!.text}`, old[2]!.text]);
  assert.deepEqual(ids(a), ["aaaaaaaa", "cccccccc"]);
  assert.deepEqual(a.merged, [{ from: "bbbbbbbb", into: "aaaaaaaa" }]);
});

test("passages without an ID (written outside Pero) stay without one", () => {
  const a = align([P(null, "Стояла осень."), P("dddddddd", "Шёл дождь.")], ["Стояла осень.", "Шёл дождь."]);
  assert.deepEqual(ids(a), [null, "dddddddd"]);
});

test("replacing everything gives no IDs and no merges", () => {
  const a = align(old, ["Совершенно другой текст.", "И ещё один, ни о чём."]);
  assert.deepEqual(ids(a), [null, null]);
  assert.deepEqual(a.merged, []);
});

test("an ID is never sent twice, whatever is typed", () => {
  const dup = old[0]!.text;
  const a = align(old, [dup, dup, dup]);
  const sent = a.passages.map((p) => p.id).filter(Boolean);
  assert.equal(new Set(sent).size, sent.length);
});

test("Latin text and mixed Cyrillic-Latin text are handled like any other", () => {
  const mixed = [P("eeeeeeee", "Он купил Fender Stratocaster 1964 года."), P("ffffffff", "Hello, Мир!")];
  const a = align(mixed, ["Он купил Fender Stratocaster 1965 года.", "Hello, Мир!"]);
  assert.deepEqual(ids(a), ["eeeeeeee", "ffffffff"]);
});

test("a split whose second part is the longer one still gives the ID to the first part", () => {
  const a = align(old, [
    "Григорий вошёл.",
    "В тёмную комнату и закрыл за собой дверь.",
    old[1]!.text,
    old[2]!.text,
  ]);
  assert.deepEqual(ids(a), ["aaaaaaaa", null, "bbbbbbbb", "cccccccc"]);
});
