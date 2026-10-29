// The client is thin: every call goes to the Pero server over HTTPS, nothing is computed here.
// The device token lives only in this module's memory (never local storage, session storage or
// cookies); a page reload signs out, and a password manager can fill the sign-in field.

import type {
  Book,
  Device,
  ErrorBody,
  HealthReport,
  ImportRequest,
  ImportSummary,
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
