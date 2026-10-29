// Screens are addressed by the `#` part of the page address, so the server needs no catch-all route
// and a reload comes back to the same screen (after signing in again).

export type Tab = "chapters" | "entities" | "timeline" | "canon";
const TABS: readonly Tab[] = ["chapters", "entities", "timeline", "canon"];

export type Route =
  | { name: "books" }
  | { name: "book"; id: string; tab: Tab }
  /** `passage`: scroll to this paragraph when the scene opens (a click on evidence in the canon ledger). */
  | { name: "scene"; id: string; passage?: string };

export function parse(hash: string): Route {
  const parts = hash.replace(/^#\/?/, "").split("/").filter(Boolean).map(decodeURIComponent);
  if (parts[0] === "book" && parts[1]) {
    const tab = TABS.find((t) => t === parts[2]) ?? "chapters";
    return { name: "book", id: parts[1], tab };
  }
  if (parts[0] === "scene" && parts[1]) {
    return parts[2] ? { name: "scene", id: parts[1], passage: parts[2] } : { name: "scene", id: parts[1] };
  }
  return { name: "books" };
}

export function href(route: Route): string {
  const e = encodeURIComponent;
  if (route.name === "books") return "#/";
  if (route.name === "book") return route.tab === "chapters" ? `#/book/${e(route.id)}` : `#/book/${e(route.id)}/${route.tab}`;
  return route.passage ? `#/scene/${e(route.id)}/${e(route.passage)}` : `#/scene/${e(route.id)}`;
}

class Router {
  route = $state<Route>(parse(window.location.hash));
  /** Returns false to stop leaving the current screen (unsaved changes, after asking the user). */
  guard: (() => boolean) | null = null;
  #shown = window.location.hash;

  constructor() {
    window.addEventListener("hashchange", () => {
      if (this.guard !== null && !this.guard()) {
        history.replaceState(null, "", this.#shown || "#/"); // stay where we were
        return;
      }
      this.#shown = window.location.hash;
      this.route = parse(this.#shown);
    });
  }
}

export const router = new Router();
