import type { Fact } from "../generated/api.ts";

export const KIND_LABELS: Record<string, string> = {
  character: "Character",
  place: "Place",
  organization: "Organization",
};
export const kindLabel = (kind: string): string => KIND_LABELS[kind] ?? kind;

export const STATUS_LABELS: Record<string, string> = {
  explicit: "Stated in the text",
  author_approved: "Canon (approved)",
  ai_hypothesis: "Hypothesis (needs your approval)",
  rejected: "Rejected",
};
export const statusLabel = (status: string): string => STATUS_LABELS[status] ?? status;

export const ORIGIN_LABELS: Record<string, string> = { author: "you", ai: "AI", import: "import" };
export const originLabel = (origin: string): string => ORIGIN_LABELS[origin] ?? origin;

/** What the author may do with a fact in this state (the server enforces the same rules). */
export function actionsFor(fact: Pick<Fact, "status" | "origin">): ("approve" | "reject" | "reopen")[] {
  const actions: ("approve" | "reject" | "reopen")[] = [];
  if (fact.status === "ai_hypothesis" || fact.status === "rejected") actions.push("approve");
  if (fact.status !== "rejected") actions.push("reject");
  if (fact.origin === "ai" && (fact.status === "author_approved" || fact.status === "rejected")) actions.push("reopen");
  return actions;
}

/** A server timestamp (UTC, ISO 8601) in the reader's own time zone and format. */
export function formatTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** Aliases typed in one box: separated by commas or line breaks, empty ones dropped. */
export function parseAliases(text: string): string[] {
  return text.split(/[,\n]/).map((a) => a.trim()).filter((a) => a !== "");
}
