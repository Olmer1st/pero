-- Canon ledger: facts about the story, each with a status, evidence passages and a status history.
-- An AI hypothesis never becomes canon on its own: 'author_approved' requires an approval record
-- (who and when), which only the explicit approve call writes.

CREATE TABLE facts (
    id          TEXT PRIMARY KEY NOT NULL,
    book_id     TEXT NOT NULL REFERENCES books (id),
    entity_id   TEXT REFERENCES entities (id),          -- the entity the fact is about, if any
    text        TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 2000),
    status      TEXT NOT NULL CHECK (status IN ('explicit', 'author_approved', 'ai_hypothesis', 'rejected')),
    origin      TEXT NOT NULL CHECK (origin IN ('author', 'ai', 'import')),
    source_ref  TEXT,                                   -- e.g. the skill or model that proposed it
    stale       INTEGER NOT NULL DEFAULT 0 CHECK (stale IN (0, 1)),   -- 1 when a cited passage was removed or changed
    stale_reason TEXT,
    approved_by TEXT,                                   -- device name of the author's approval
    approved_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (status <> 'author_approved' OR (approved_by IS NOT NULL AND approved_at IS NOT NULL)),
    CHECK (stale = 1 OR stale_reason IS NULL)
) STRICT;
CREATE INDEX facts_by_book ON facts (book_id, status);
CREATE INDEX facts_by_entity ON facts (entity_id);

-- Supporting passages. The hash is the passage text hash at the time of citing, so an edited passage
-- shows its evidence as out of date. Passages are tombstoned, never deleted, so these links always resolve.
CREATE TABLE fact_evidence (
    fact_id     TEXT NOT NULL REFERENCES facts (id),
    passage_id  TEXT NOT NULL REFERENCES passages (id),
    text_hash   TEXT NOT NULL CHECK (length(text_hash) = 64),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (fact_id, passage_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX fact_evidence_by_passage ON fact_evidence (passage_id);

CREATE TABLE fact_status_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_id     TEXT NOT NULL REFERENCES facts (id),
    from_status TEXT CHECK (from_status IN ('explicit', 'author_approved', 'ai_hypothesis', 'rejected')),
    to_status   TEXT NOT NULL CHECK (to_status IN ('explicit', 'author_approved', 'ai_hypothesis', 'rejected')),
    changed_by  TEXT NOT NULL,                          -- device name, or 'ai:<source>' / 'import'
    reason      TEXT,
    changed_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX fact_status_history_by_fact ON fact_status_history (fact_id, id);
