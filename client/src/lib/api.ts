// The client is thin: every call goes to the Pero server over HTTPS, nothing is computed here.
// The device token lives only in this module's memory (never local storage, session storage or
// cookies); a page reload signs out, and a password manager can fill the sign-in field.

import type {
  Book,
  Chapter,
  Device,
  Entity,
  EntityPatch,
  ErrorBody,
  Event,
  EventPatch,
  Fact,
  FactPatch,
  HealthReport,
  HistoryResponse,
  ImportRequest,
  ImportSummary,
  NewEntity,
  NewEvent,
  NewFact,
  NewRelationship,
  PassageInfo,
  Relationship,
  SaveScene,
  Saved,
  Scene,
  SceneInfo,
  TransitionBody,
} from "../generated/api.ts";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly body: ErrorBody | null;

  constructor(status: number, code: string, message: string, body: ErrorBody | null) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.body = body;
  }
}

let token: string | null = null;
let onUnauthorized: (() => void) | null = null;

export function setToken(value: string): void {
  token = value.trim();
}

export function clearToken(): void {
  token = null;
}

export function hasToken(): boolean {
  return token !== null;
}

/// Called when the server refuses the token (revoked, wrong); the app signs out.
export function setUnauthorizedHandler(handler: (() => void) | null): void {
  onUnauthorized = handler;
}

function isErrorBody(value: unknown): value is ErrorBody {
  if (typeof value !== "object" || value === null) return false;
  const error = (value as { error?: unknown }).error;
  return typeof error === "object" && error !== null && "code" in error && "message" in error;
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (token !== null) headers["Authorization"] = `Bearer ${token}`;
  const init: RequestInit = { method, headers, credentials: "omit", cache: "no-store" };
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
    init.body = JSON.stringify(body);
  }

  let response: Response;
  try {
    response = await fetch(path, init);
  } catch {
    throw new ApiError(0, "unreachable", "Cannot reach the server.", null);
  }

  const text = await response.text();
  let parsed: unknown = null;
  if (text !== "") {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = null;
    }
  }

  if (!response.ok) {
    if (response.status === 401 && token !== null) onUnauthorized?.();
    if (isErrorBody(parsed)) {
      throw new ApiError(response.status, parsed.error.code, parsed.error.message, parsed);
    }
    throw new ApiError(response.status, "http_error", `The server answered ${response.status}.`, null);
  }
  return parsed as T;
}

/// Open to everyone: no manuscript data.
export function health(): Promise<HealthReport> {
  return request<HealthReport>("GET", "/health");
}

export function me(): Promise<Device> {
  return request<Device>("GET", "/api/v1/me");
}

export function listBooks(): Promise<Book[]> {
  return request<Book[]>("GET", "/api/v1/books");
}

export function importBook(body: ImportRequest): Promise<ImportSummary> {
  return request<ImportSummary>("POST", "/api/v1/books/import", body);
}

export function getBook(id: string): Promise<Book> {
  return request<Book>("GET", `/api/v1/books/${encodeURIComponent(id)}`);
}

export function listChapters(bookId: string): Promise<Chapter[]> {
  return request<Chapter[]>("GET", `/api/v1/books/${encodeURIComponent(bookId)}/chapters`);
}

export function listScenes(chapterId: string): Promise<SceneInfo[]> {
  return request<SceneInfo[]>("GET", `/api/v1/chapters/${encodeURIComponent(chapterId)}/scenes`);
}

export function getScene(id: string): Promise<Scene> {
  return request<Scene>("GET", `/api/v1/scenes/${encodeURIComponent(id)}`);
}

/// A save is based on the revision the editor loaded; if the scene changed since, the server answers
/// 409 `revision_conflict` and the error body carries the scene as it is now (`body.current`).
export function saveScene(id: string, body: SaveScene): Promise<Saved> {
  return request<Saved>("PUT", `/api/v1/scenes/${encodeURIComponent(id)}`, body);
}

// ---- story graph --------------------------------------------------------------------------------

const q = (params: Record<string, string | boolean | undefined>): string => {
  const pairs = Object.entries(params).filter(([, v]) => v !== undefined && v !== "");
  return pairs.length === 0
    ? ""
    : "?" + pairs.map(([k, v]) => `${k}=${encodeURIComponent(String(v))}`).join("&");
};
const enc = encodeURIComponent;

export const listEntities = (bookId: string): Promise<Entity[]> =>
  request("GET", `/api/v1/books/${enc(bookId)}/entities`);
export const createEntity = (bookId: string, body: NewEntity): Promise<Entity> =>
  request("POST", `/api/v1/books/${enc(bookId)}/entities`, body);
export const patchEntity = (id: string, body: EntityPatch): Promise<Entity> =>
  request("PATCH", `/api/v1/entities/${enc(id)}`, body);
/// 409 `in_use` unless `cascade`; canon facts about the entity always block (the message says so).
export const deleteEntity = (id: string, cascade = false): Promise<void> =>
  request("DELETE", `/api/v1/entities/${enc(id)}${q({ cascade: cascade || undefined })}`);

export const listRelationships = (bookId: string): Promise<Relationship[]> =>
  request("GET", `/api/v1/books/${enc(bookId)}/relationships`);
export const createRelationship = (bookId: string, body: NewRelationship): Promise<Relationship> =>
  request("POST", `/api/v1/books/${enc(bookId)}/relationships`, body);
export const deleteRelationship = (id: string): Promise<void> =>
  request("DELETE", `/api/v1/relationships/${enc(id)}`);

export const listEvents = (bookId: string): Promise<Event[]> =>
  request("GET", `/api/v1/books/${enc(bookId)}/events`);
export const createEvent = (bookId: string, body: NewEvent): Promise<Event> =>
  request("POST", `/api/v1/books/${enc(bookId)}/events`, body);
export const patchEvent = (id: string, body: EventPatch): Promise<Event> =>
  request("PATCH", `/api/v1/events/${enc(id)}`, body);
export const deleteEvent = (id: string): Promise<void> =>
  request("DELETE", `/api/v1/events/${enc(id)}`);

// ---- canon ledger -------------------------------------------------------------------------------

export const listFacts = (bookId: string, filter: { status?: string; stale?: boolean } = {}): Promise<Fact[]> =>
  request("GET", `/api/v1/books/${enc(bookId)}/facts${q(filter)}`);
export const createFact = (bookId: string, body: NewFact): Promise<Fact> =>
  request("POST", `/api/v1/books/${enc(bookId)}/facts`, body);
export const patchFact = (id: string, body: FactPatch): Promise<Fact> =>
  request("PATCH", `/api/v1/facts/${enc(id)}`, body);
/// `approve` is the only way a fact that did not start approved becomes canon; the server records who.
export const transitionFact = (id: string, action: "approve" | "reject" | "reopen", body: TransitionBody = {}): Promise<Fact> =>
  request("POST", `/api/v1/facts/${enc(id)}/${action}`, body);
export const factHistory = (id: string): Promise<HistoryResponse> =>
  request("GET", `/api/v1/facts/${enc(id)}/history`);
/// Sending the same passages again says "I checked: the evidence still holds" and clears the stale flag.
export const replaceEvidence = (id: string, passageIds: string[]): Promise<Fact> =>
  request("PUT", `/api/v1/facts/${enc(id)}/evidence`, { evidence: passageIds.map((passage_id) => ({ passage_id })) });
export const getPassage = (id: string): Promise<PassageInfo> =>
  request("GET", `/api/v1/passages/${enc(id)}`);

/// Text to show for a failed call; nothing for a 401, because the session signs out by itself.
export function describe(e: unknown, fallback: string): string {
  if (e instanceof ApiError) return e.status === 401 ? "" : e.message;
  return fallback;
}
