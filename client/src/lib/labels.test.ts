import assert from "node:assert/strict";
import { test } from "node:test";

import { actionsFor, formatTime, parseAliases } from "./labels.ts";

test("what can be done with a fact follows the server's rules", () => {
  assert.deepEqual(actionsFor({ status: "ai_hypothesis", origin: "ai" }), ["approve", "reject"]);
  assert.deepEqual(actionsFor({ status: "explicit", origin: "import" }), ["reject"]);
  assert.deepEqual(actionsFor({ status: "author_approved", origin: "author" }), ["reject"]);
  assert.deepEqual(actionsFor({ status: "author_approved", origin: "ai" }), ["reject", "reopen"]);
  assert.deepEqual(actionsFor({ status: "rejected", origin: "author" }), ["approve"]);
  assert.deepEqual(actionsFor({ status: "rejected", origin: "ai" }), ["approve", "reopen"]);
});

test("aliases split on commas and line breaks", () => {
  assert.deepEqual(parseAliases("Гриша, Гриня\n Григорий Петрович ,, "), ["Гриша", "Гриня", "Григорий Петрович"]);
  assert.deepEqual(parseAliases("  "), []);
});

test("a timestamp is shown for people; anything unreadable is shown as it came", () => {
  assert.notEqual(formatTime("2026-10-29T20:26:59.275Z"), "2026-10-29T20:26:59.275Z");
  assert.match(formatTime("2026-10-29T20:26:59.275Z"), /2026/);
  assert.equal(formatTime("not a date"), "not a date");
});
