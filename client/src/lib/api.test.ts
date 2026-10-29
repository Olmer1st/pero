// Run with `npm test` (Node's built-in runner; no extra packages).
import assert from "node:assert/strict";
import { afterEach, test } from "node:test";

import {
  ApiError,
  clearToken,
  hasToken,
  health,
  importBook,
  listBooks,
  me,
  setToken,
  setUnauthorizedHandler,
} from "./api.ts";

type Call = { url: string; init: RequestInit };
const calls: Call[] = [];
const realFetch = globalThis.fetch;

function answer(status: number, body: unknown): void {
  globalThis.fetch = (async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    return new Response(body === null ? "" : JSON.stringify(body), { status });
  }) as typeof fetch;
}

afterEach(() => {
  globalThis.fetch = realFetch;
  calls.length = 0;
  clearToken();
  setUnauthorizedHandler(null);
});

const headers = (c: Call | undefined) => (c?.init.headers ?? {}) as Record<string, string>;

test("no token: no Authorization header; token set: Bearer header; cleared: gone again", async () => {
  answer(200, []);
  await listBooks();
  assert.equal(headers(calls[0])["Authorization"], undefined);

  setToken("  pero_abc  ");
  assert.equal(hasToken(), true);
  await listBooks();
  assert.equal(headers(calls[1])["Authorization"], "Bearer pero_abc");

  clearToken();
  assert.equal(hasToken(), false);
  await listBooks();
  assert.equal(headers(calls[2])["Authorization"], undefined);
});

test("requests carry no cookies and are never cached", async () => {
  answer(200, { status: "ok" });
  await health();
  assert.equal(calls[0]?.init.credentials, "omit");
  assert.equal(calls[0]?.init.cache, "no-store");
});

test("the token is not written to any browser storage", async () => {
  const stores: string[] = [];
  const fake = { setItem: () => void stores.push("write"), getItem: () => null };
  Object.assign(globalThis, { localStorage: fake, sessionStorage: fake });
  answer(200, { id: "dv_1", name: "laptop" });
  setToken("pero_secret");
  await me();
  assert.deepEqual(stores, []);
  delete (globalThis as Record<string, unknown>)["localStorage"];
  delete (globalThis as Record<string, unknown>)["sessionStorage"];
});

test("a JSON error body becomes an ApiError with its code and issues", async () => {
  const body = {
    error: { code: "import_rejected", message: "the files were not understood" },
    issues: [{ file: "a.md", line: 3, message: "heading level not supported" }],
  };
  answer(422, body);
  await assert.rejects(importBook({ title: "T", language: "ru", files: [] }), (e: unknown) => {
    assert.ok(e instanceof ApiError);
    assert.equal(e.status, 422);
    assert.equal(e.code, "import_rejected");
    assert.equal(e.body?.issues?.[0]?.line, 3);
    return true;
  });
});

test("a plain-text or empty error still becomes an ApiError", async () => {
  globalThis.fetch = (async () => new Response("nope", { status: 400 })) as typeof fetch;
  await assert.rejects(listBooks(), (e: unknown) => e instanceof ApiError && e.code === "http_error");
});

test("an unreachable server becomes the code unreachable", async () => {
  globalThis.fetch = (async () => {
    throw new TypeError("fetch failed");
  }) as typeof fetch;
  await assert.rejects(health(), (e: unknown) => e instanceof ApiError && e.code === "unreachable");
});

test("a 401 with a token signs out through the handler; without a token it does not", async () => {
  let signedOut = 0;
  setUnauthorizedHandler(() => void signedOut++);
  answer(401, { error: { code: "unauthorized", message: "a valid device token is required" } });

  await assert.rejects(listBooks());
  assert.equal(signedOut, 0);

  setToken("pero_revoked");
  await assert.rejects(listBooks());
  assert.equal(signedOut, 1);
});

test("import sends the body as JSON to the import route", async () => {
  answer(201, { book: {}, chapters: 1, scenes: 1, passages: 2, titles_from_filename: [] });
  setToken("pero_x");
  await importBook({ title: "Книга", language: "ru", files: [{ name: "01.md", content: "# Глава" }] });
  assert.equal(calls[0]?.url, "/api/v1/books/import");
  assert.equal(calls[0]?.init.method, "POST");
  assert.equal(headers(calls[0])["Content-Type"], "application/json");
  assert.match(String(calls[0]?.init.body), /Книга/);
});
