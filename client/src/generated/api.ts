// GENERATED from the Rust types in `shared/` by `cargo test -p pero-shared`. Do not edit.
// Regenerate: PERO_UPDATE_TYPES=1 cargo test -p pero-shared generated_types_are_current

export type Book = { id: string, title: string, language: string, head_commit: string | null, };

export type Chapter = { id: string, book_id: string, position: number, title: string, };

export type SceneInfo = { id: string, book_id: string, chapter_id: string, position: number, title: string, language: string, pov: string | null, date: string | null, location: string | null, };

export type PassageOut = { 
/**
 * `None` for a paragraph written outside Pero that has no marker yet.
 */
id: string | null, text: string, };

export type Scene = { passages: Array<PassageOut>, 
/**
 * SHA-256 of the scene file; send it back as `base_revision` when saving.
 */
revision: string, id: string, book_id: string, chapter_id: string, position: number, title: string, language: string, pov: string | null, date: string | null, location: string | null, };

export type SceneFields = { title?: string, language?: string, pov?: string, date?: string, location?: string, };

export type PassageIn = { 
/**
 * Existing passage ID, or absent for a new paragraph.
 */
id?: string, text: string, };

export type Merge = { from: string, into: string, };

export type SaveScene = { base_revision: string, fields: SceneFields, passages: Array<PassageIn>, merged?: Array<Merge>, };

export type Saved = { scene: Scene, 
/**
 * The new commit, or `None` when the save changed nothing.
 */
commit: string | null, };

export type ImportSummary = { book: Book, chapters: number, scenes: number, passages: number, 
/**
 * Files that had no `# ` heading, so the chapter title was made from the file name.
 */
titles_from_filename: Array<string>, };

export type ExportFile = { name: string, content: string, };

export type ExportResponse = { files: Array<ExportFile>, };

export type ReindexResponse = { 
/**
 * Paragraphs written outside Pero that still have no passage marker.
 */
unmarked_paragraphs: number, };

export type ParagraphMode = "blank_line" | "line";

export type ImportFile = { 
/**
 * File name only, ending in `.md`.
 */
name: string, content: string, };

export type ImportRequest = { title: string, language: string, mode?: ParagraphMode, files: Array<ImportFile>, };

export type NewBook = { title: string, language: string, };

export type TitleBody = { title: string, };

export type NewChapter = { title: string, position?: number, };

export type NewScene = { title?: string, language?: string, pov?: string, date?: string, location?: string, position?: number, };

export type OrderBody = { ids: Array<string>, };

export type Entity = { id: string, book_id: string, kind: string, name: string, description: string, 
/**
 * Other names and forms, sorted.
 */
aliases: Array<string>, created_at: string, updated_at: string, };

export type NewEntity = { kind: string, name: string, description?: string, aliases?: Array<string>, };

export type EntityPatch = { name?: string, description?: string, 
/**
 * Replaces all aliases when present.
 */
aliases?: Array<string>, };

export type Relationship = { id: string, book_id: string, from_entity_id: string, to_entity_id: string, kind: string, directed: boolean, description: string, created_at: string, updated_at: string, };

export type NewRelationship = { from_entity_id: string, to_entity_id: string, kind: string, directed?: boolean, description?: string, };

export type RelationshipPatch = { kind?: string, directed?: boolean, description?: string, };

export type Participant = { entity_id: string, role?: string, };

export type Event = { id: string, book_id: string, title: string, description: string, 
/**
 * The date as the author writes it (fictional calendars are fine).
 */
when_text: string | null, 
/**
 * Author-controlled order on the timeline; lower comes first.
 */
sort_order: number, scene_id: string | null, place_id: string | null, participants: Array<Participant>, created_at: string, updated_at: string, };

export type NewEvent = { title: string, description?: string, when_text?: string, 
/**
 * Defaults to 10 after the last event of the book.
 */
sort_order?: number, scene_id?: string, place_id?: string, participants?: Array<Participant>, };

export type EventPatch = { title?: string, description?: string, when_text?: string | null, sort_order?: number, scene_id?: string | null, place_id?: string | null, 
/**
 * Replaces all participants when present.
 */
participants?: Array<Participant>, };

export type EvidenceOut = { passage_id: string, 
/**
 * `active`, or `removed` when the cited passage no longer exists in the text.
 */
passage_status: string, 
/**
 * True when the passage text changed since it was cited.
 */
changed: boolean, };

export type Fact = { id: string, book_id: string, entity_id: string | null, text: string, status: string, origin: string, source_ref: string | null, 
/**
 * True when a cited passage was removed, merged or edited since the fact was recorded.
 */
stale: boolean, stale_reason: string | null, approved_by: string | null, approved_at: string | null, evidence: Array<EvidenceOut>, created_at: string, updated_at: string, };

export type HistoryEntry = { from_status: string | null, to_status: string, changed_by: string, reason: string | null, changed_at: string, };

export type HistoryResponse = { history: Array<HistoryEntry>, };

export type PassageInfo = { id: string, book_id: string, scene_id: string, position: number, status: string, merged_into: string | null, removed_at: string | null, };

export type EvidenceIn = { passage_id: string, };

export type NewFact = { text: string, entity_id?: string, status?: string, 
/**
 * `author` (default), `ai` or `import`.
 */
origin?: string, 
/**
 * For AI facts: the skill or model that proposed it.
 */
source_ref?: string, evidence?: Array<EvidenceIn>, };

export type FactPatch = { text?: string, entity_id?: string | null, source_ref?: string | null, };

export type TransitionBody = { reason?: string, };

export type EvidenceReplace = { evidence: Array<EvidenceIn>, };

export type Device = { id: string, name: string, };

export type DeviceInfo = { id: string, name: string, created_at: string, last_seen_at: string | null, revoked_at: string | null, };

export type ServiceStatus = { status: string, latency_ms: number, };

export type Services = { llm: ServiceStatus, languagetool: ServiceStatus, };

export type HealthReport = { 
/**
 * `ok` when both local services answer, otherwise `degraded`.
 */
status: string, version: string, uptime_seconds: number, services: Services, };

export type ImportIssue = { file: string, 
/**
 * 1-based line, or 0 when the problem concerns the whole file.
 */
line: number, message: string, };

export type ErrorBody = { error: ErrorDetail, 
/**
 * Only for `import_rejected`: what was wrong and where (never the text itself).
 */
issues?: Array<ImportIssue>, 
/**
 * Only for `revision_conflict`: the scene as it is now.
 */
current?: Scene, };

export type ErrorDetail = { 
/**
 * Stable machine-readable code, for example `not_found`, `invalid`, `revision_conflict`.
 */
code: string, message: string, };

export const ENTITY_KINDS = ["character", "place", "organization"] as const;
export type EntityKind = (typeof ENTITY_KINDS)[number];

export const FACT_STATUSES = ["explicit", "author_approved", "ai_hypothesis", "rejected"] as const;
export type FactStatus = (typeof FACT_STATUSES)[number];

export const FACT_ORIGINS = ["author", "ai", "import"] as const;
export type FactOrigin = (typeof FACT_ORIGINS)[number];

export const PASSAGE_STATUSES = ["active", "removed"] as const;
export type PassageStatus = (typeof PASSAGE_STATUSES)[number];
