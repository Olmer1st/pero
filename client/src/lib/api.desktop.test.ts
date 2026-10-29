// The desktop app path of the API module: requests go to the app (invoke), never to fetch, and the token is never held here.
import assert from "node:assert/strict";
import { test } from "node:test";

type Call = { command: string; args: Record<string, unknown> };
const calls: Call[] = [];
let handler: (command: string, args: Record<string, unknown>) => unknown = () => null;

Object.assign(globalThis, {
  window: {
    __TAURI_INTERNALS__: {
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        calls.push({ command, args });
        return handler(command, args);
      },
    },
  },
});
let fetched = 0;
globalThis.fetch = (async () => {
  fetched++;
  return new Response("{}");
}) as typeof fetch;

const api = await import("./api.ts");

test("inside the app, requests are made by the app, carry no token and never use fetch", async () => {
  handler = () => ({ status: 200, body: JSON.stringify([{ id: "bk_1" }]) });
  const books = await api.listBooks();
  assert.equal(books.length, 1);
  assert.equal(fetched, 0);
  assert.equal(calls[0]?.command, "api_request");
  assert.deepEqual(calls[0]?.args, { method: "GET", path: "/api/v1/books", body: null });
  assert.equal(JSON.stringify(calls[0]).includes("Authorization"), false);
});

test("a body is sent as JSON text", async () => {
  handler = () => ({ status: 201, body: "{}" });
  await api.importBook({ title: "Книга", language: "ru", files: [] });
  const last = calls.at(-1)!;
  assert.equal(last.args["method"], "POST");
  assert.match(String(last.args["body"]), /Книга/);
});

test("signing in hands the token to the app once and keeps nothing here", async () => {
  handler = (command) =>
    command === "sign_in" ? { device: { id: "dv_1", name: "laptop" }, remembered: true } : { status: 200, body: "{}" };
  const session = await api.signIn("pero_" + "a".repeat(64));
  assert.deepEqual(session, { device: { id: "dv_1", name: "laptop" }, remembered: true });
  assert.equal(api.hasToken(), false, "the token is not held in the page");
  const signIn = calls.find((c) => c.command === "sign_in")!;
  assert.deepEqual(Object.keys(signIn.args), ["token"]);
});

test("a refused sign-in comes back as an error the screen can show", async () => {
  handler = () => {
    throw "That token was not accepted."; // the app answers with plain text
  };
  await assert.rejects(api.signIn("pero_" + "b".repeat(64)), (e: unknown) => {
    assert.ok(e instanceof api.ApiError);
    assert.equal(e.message, "That token was not accepted.");
    return true;
  });
});

test("a 401 after signing in signs the app out; a failed command reads as unreachable", async () => {
  handler = (command) => (command === "sign_in" ? { device: { id: "dv_1", name: "laptop" }, remembered: true } : { status: 401, body: JSON.stringify({ error: { code: "unauthorized", message: "a valid device token is required" } }) });
  await api.signIn("pero_" + "c".repeat(64));
  let out = 0;
  api.setUnauthorizedHandler(() => void out++);
  await assert.rejects(api.listBooks());
  assert.equal(out, 1);

  handler = () => {
    throw "Cannot connect to the server (is it on, and are you at home?).";
  };
  await assert.rejects(api.health(), (e: unknown) => e instanceof api.ApiError && e.code === "unreachable" && /Cannot connect/.test(e.message));
  api.setUnauthorizedHandler(null);
});

test("finding the server, resuming and signing out are commands of the app", async () => {
  handler = (command) => (command === "discover" ? { address: "writer.local:8443", version: "0.1.0" } : command === "resume" ? null : undefined);
  assert.deepEqual(await api.findServer(), { address: "writer.local:8443", version: "0.1.0" });
  assert.equal(await api.resume(), null);
  await api.signOut();
  assert.ok(calls.some((c) => c.command === "sign_out"));
});
