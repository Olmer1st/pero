// End-to-end check of the client's API module against a REAL server (see run-local.sh).
// Usage: PERO_E2E_URL=https://127.0.0.1:8443 PERO_E2E_TOKEN=pero_... node --experimental-strip-types e2e/e2e.ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const base = process.env["PERO_E2E_URL"]!;
const token = process.env["PERO_E2E_TOKEN"]!;
assert.ok(base && token, "PERO_E2E_URL and PERO_E2E_TOKEN are required");

// The client uses addresses relative to its own origin; here the origin is the test server.
const realFetch = globalThis.fetch;
globalThis.fetch = ((url: string, init?: RequestInit) =>
  realFetch(url.startsWith("/") ? base + url : url, init)) as typeof fetch;

const api = await import("../src/lib/api.ts");

/** Field names of a generated type, read from the committed TypeScript file. */
function fieldsOf(typeName: string): string[] {
  const text = readFileSync(new URL("../src/generated/api.ts", import.meta.url), "utf8");
  const start = text.indexOf(`export type ${typeName} = {`);
  assert.ok(start >= 0, `${typeName} is not in the generated file`);
  let depth = 0;
  let name = "";
  const names: string[] = [];
  let i = text.indexOf("{", start);
  const end = text.indexOf("\n\n", start);
  let expectName = true;
  for (; i < end; i++) {
    const c = text[i]!;
    if (c === "{" || c === "<" || c === "(") depth++;
    else if (c === "}" || c === ">" || c === ")") depth--;
    else if (depth === 1 && c === ",") expectName = true;
    else if (depth === 1 && expectName && /[A-Za-z_]/.test(c)) {
      name = text.slice(i).match(/^[A-Za-z_]+/)![0];
      names.push(name);
      i += name.length - 1;
      expectName = false;
    } else if (depth === 1 && c === "/" && text[i + 1] === "*") {
      i = text.indexOf("*/", i) + 1; // doc comment inside the type
    }
  }
  return names.sort();
}
const keys = (o: object) => Object.keys(o).sort();

// --- the page and its headers ---------------------------------------------------------------
const page = await realFetch(base + "/");
assert.equal(page.status, 200);
assert.match(page.headers.get("content-security-policy") ?? "", /script-src 'self'/);
const html = await page.text();
const script = html.match(/src="(\/assets\/[^"]+\.js)"/)?.[1];
assert.ok(script, "index.html names a script");
const asset = await realFetch(base + script);
assert.equal(asset.status, 200);
assert.match(asset.headers.get("content-type") ?? "", /javascript/);
assert.equal((await realFetch(base + "/assets/..%2f..%2fetc%2fpasswd")).status >= 400, true);

// --- health is open and matches its type ------------------------------------------------------
const h = await api.health();
assert.deepEqual(keys(h), fieldsOf("HealthReport"));
assert.deepEqual(keys(h.services), fieldsOf("Services"));

// --- the API refuses without a token ----------------------------------------------------------
await assert.rejects(api.listBooks(), (e: unknown) => e instanceof api.ApiError && e.status === 401);
let signedOut = 0;
api.setUnauthorizedHandler(() => void signedOut++);
api.setToken("pero_" + "0".repeat(64));
await assert.rejects(api.me(), (e: unknown) => e instanceof api.ApiError && e.status === 401);
assert.equal(signedOut, 1, "a wrong token signs the app out");

// --- sign in, import a book, see it in the list -------------------------------------------------
api.setToken(token);
const device = await api.me();
assert.deepEqual(keys(device), fieldsOf("Device"));

const summary = await api.importBook({
  title: "Тестовая книга",
  language: "ru",
  files: [
    { name: "01-nachalo.md", content: "# Начало\n\nПервый абзац.\n\n— Привет, — сказала она.\n" },
    { name: "02-konec.md", content: "# Конец\n\nПоследний абзац.\n" },
  ],
});
assert.deepEqual(keys(summary), fieldsOf("ImportSummary"));
assert.equal(summary.chapters, 2);
assert.equal(summary.passages, 3); // headings are not passages

const books = await api.listBooks();
assert.equal(books.length, 1);
assert.deepEqual(keys(books[0]!), fieldsOf("Book"));
assert.equal(books[0]!.title, "Тестовая книга");

// --- a refused import explains itself and changes nothing -------------------------------------------
await assert.rejects(
  api.importBook({ title: "X", language: "ru", files: [{ name: "a.md", content: "# Г\n\n### нельзя\n" }] }),
  (e: unknown) => {
    assert.ok(e instanceof api.ApiError);
    assert.equal(e.code, "import_rejected");
    assert.equal(e.body?.issues?.[0]?.file, "a.md");
    assert.deepEqual(keys(e.body!.issues![0]!), fieldsOf("ImportIssue"));
    return true;
  },
);
assert.equal((await api.listBooks()).length, 1);

// --- open a scene, edit it, save it (a Git commit), conflict handling -------------------------------
const { align, toDocument, toParagraphs } = await import("../src/lib/passages.ts");
const book = books[0]!;
const chapters = await api.listChapters(book.id);
assert.equal(chapters.length, 2);
assert.deepEqual(keys(chapters[0]!), fieldsOf("Chapter"));
const scenes = await api.listScenes(chapters[0]!.id);
assert.deepEqual(keys(scenes[0]!), fieldsOf("SceneInfo"));
const scene = await api.getScene(scenes[0]!.id);
assert.equal(scene.passages.length, 2);
assert.ok(scene.passages.every((p) => typeof p.id === "string"));

const edited = toDocument(scene.passages).replace("Первый абзац.", "Первый абзац, теперь переписанный.");
const { passages, merged } = align(scene.passages, toParagraphs(edited));
const fields = { title: scene.title, language: scene.language };
const saved = await api.saveScene(scene.id, { base_revision: scene.revision, fields, passages, merged });
assert.deepEqual(keys(saved), fieldsOf("Saved"));
assert.match(saved.commit ?? "", /^[0-9a-f]{40}$/, "a save makes a commit");
assert.notEqual(saved.scene.revision, scene.revision);
assert.deepEqual(saved.scene.passages.map((p) => p.id), scene.passages.map((p) => p.id), "IDs survive an edit");
assert.equal(saved.scene.passages[0]!.text, "Первый абзац, теперь переписанный.");
const reloaded = await api.getScene(scene.id);
assert.equal(reloaded.passages[0]!.text, "Первый абзац, теперь переписанный.", "a reload shows the change");

// a save based on the old revision is refused with the current scene, and "keep mine" works on top of it
await assert.rejects(
  api.saveScene(scene.id, { base_revision: scene.revision, fields, passages, merged }),
  (e: unknown) => {
    assert.ok(e instanceof api.ApiError);
    assert.equal(e.code, "revision_conflict");
    assert.equal(e.body?.current?.revision, saved.scene.revision);
    return true;
  },
);
const again = align(saved.scene.passages, toParagraphs(edited + "\n\nТретий абзац."));
const second = await api.saveScene(scene.id, { base_revision: saved.scene.revision, fields, passages: again.passages, merged: again.merged });
assert.equal(second.scene.passages.length, 3);
assert.equal(second.scene.passages[2]!.text, "Третий абзац.");
console.log("COMMIT " + saved.commit);
console.log("COMMIT " + second.commit);

console.log("E2E PASSED against a real server");
