import type { Device } from "../generated/api.ts";
import { ApiError, findServer, isDesktop, resume, setUnauthorizedHandler, signIn, signOut, useAddress } from "./api.ts";

export type ServerState =
  | { status: "searching" }
  | { status: "found"; address: string; version: string }
  | { status: "missing"; error: string };

class Session {
  device = $state<Device | null>(null);
  /** Why the user is signed out, when it was not their own choice. */
  notice = $state("");
  /** Desktop app only: whether the token is kept in the system keychain for the next start. */
  remembered = $state(true);
  /** Desktop app only: the search for the server. The browser page is served by the server itself. */
  server = $state<ServerState | null>(isDesktop() ? { status: "searching" } : null);
  #started = false;

  constructor() {
    setUnauthorizedHandler(() => {
      if (this.device === null) return; // a wrong token at sign-in is reported by signIn itself
      this.notice = "The server no longer accepts this token. It may have been revoked.";
      void this.signOut(false);
    });
  }

  /** Desktop app: finds the server, then signs in again with the token from the keychain. Does nothing in the browser. */
  async start(): Promise<void> {
    if (!isDesktop() || this.#started) return;
    this.#started = true;
    await this.#search(() => findServer());
  }

  async tryAddress(address: string): Promise<void> {
    await this.#search(() => useAddress(address));
  }

  async #search(find: () => Promise<{ address: string; version: string }>): Promise<void> {
    this.server = { status: "searching" };
    try {
      const found = await find();
      this.server = { status: "found", ...found };
    } catch (e) {
      this.server = { status: "missing", error: e instanceof ApiError ? e.message : "The server was not found." };
      return;
    }
    try {
      const session = await resume();
      if (session) {
        this.device = session.device;
        this.remembered = session.remembered;
      }
    } catch {
      // the server is there but did not confirm the saved token (for example it is restarting): sign in by hand
    }
  }

  /** Returns what to tell the user when the sign-in failed, or null on success. */
  async signIn(token: string): Promise<string | null> {
    try {
      const session = await signIn(token);
      this.device = session.device;
      this.remembered = session.remembered;
      this.notice = "";
      return null;
    } catch (e) {
      if (e instanceof ApiError) {
        if (e.status === 401 || e.message === "That token was not accepted.") return "That token was not accepted.";
        if (e.code === "unreachable") return "Cannot reach the server.";
        return e.message;
      }
      return "Something went wrong.";
    }
  }

  async signOut(clearNotice = true): Promise<void> {
    this.device = null;
    if (clearNotice) this.notice = "";
    await signOut();
  }
}

export const session = new Session();
