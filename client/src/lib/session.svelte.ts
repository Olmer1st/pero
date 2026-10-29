import type { Device } from "../generated/api.ts";
import { ApiError, clearToken, me, setToken, setUnauthorizedHandler } from "./api.ts";

class Session {
  device = $state<Device | null>(null);
  /** Why the user is signed out, when it was not their own choice. */
  notice = $state("");

  constructor() {
    setUnauthorizedHandler(() => {
      if (this.device === null) return; // a wrong token at sign-in is reported by signIn itself
      this.notice = "The server no longer accepts this token. It may have been revoked.";
      this.signOut(false);
    });
  }

  async signIn(token: string): Promise<string | null> {
    setToken(token);
    try {
      this.device = await me();
      this.notice = "";
      return null;
    } catch (e) {
      clearToken();
      if (e instanceof ApiError) {
        if (e.status === 401) return "That token was not accepted.";
        if (e.code === "unreachable") return "Cannot reach the server.";
        return e.message;
      }
      return "Something went wrong.";
    }
  }

  signOut(clearNotice = true): void {
    clearToken();
    this.device = null;
    if (clearNotice) this.notice = "";
  }
}

export const session = new Session();
