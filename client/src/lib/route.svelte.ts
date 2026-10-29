// Screens are addressed by the `#` part of the page address, so the server needs no catch-all route
// and a reload comes back to the same screen (after signing in again).

export type Route =
  | { name: "books" }
  | { name: "book"; id: string }
  | { name: "scene"; id: string };

export function parse(hash: string): Route {
  const parts = hash.replace(/^#\/?/, "").split("/").filter(Boolean).map(decodeURIComponent);
  if (parts[0] === "book" && parts[1]) return { name: "book", id: parts[1] };
  if (parts[0] === "scene" && parts[1]) return { name: "scene", id: parts[1] };
  return { name: "books" };
}

export const href = (route: Route): string =>
  route.name === "books" ? "#/" : `#/${route.name}/${encodeURIComponent(route.id)}`;

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
