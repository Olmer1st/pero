-- Story graph: entities, aliases, relationships, timeline events.

CREATE TABLE entities (
    id          TEXT PRIMARY KEY NOT NULL,
    book_id     TEXT NOT NULL REFERENCES books (id),
    kind        TEXT NOT NULL CHECK (kind IN ('character', 'place', 'organization')),
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 300),
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX entities_by_book ON entities (book_id, kind, name);

-- Other names and forms of an entity (nicknames, short forms, inflected forms). The name registry
-- of the deterministic checks builds on this table.
CREATE TABLE entity_aliases (
    entity_id   TEXT NOT NULL REFERENCES entities (id),
    alias       TEXT NOT NULL CHECK (length(alias) BETWEEN 1 AND 300),
    PRIMARY KEY (entity_id, alias)
) STRICT, WITHOUT ROWID;

CREATE TABLE relationships (
    id             TEXT PRIMARY KEY NOT NULL,
    book_id        TEXT NOT NULL REFERENCES books (id),
    from_entity_id TEXT NOT NULL REFERENCES entities (id),
    to_entity_id   TEXT NOT NULL REFERENCES entities (id),
    kind           TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 100),   -- free text, e.g. "sister of"
    directed       INTEGER NOT NULL DEFAULT 1 CHECK (directed IN (0, 1)),
    description    TEXT NOT NULL DEFAULT '',
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (from_entity_id <> to_entity_id)
) STRICT;
CREATE INDEX relationships_from ON relationships (from_entity_id);
CREATE INDEX relationships_to ON relationships (to_entity_id);
CREATE INDEX relationships_by_book ON relationships (book_id);

CREATE TABLE timeline_events (
    id          TEXT PRIMARY KEY NOT NULL,
    book_id     TEXT NOT NULL REFERENCES books (id),
    title       TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 300),
    description TEXT NOT NULL DEFAULT '',
    when_text   TEXT,                                  -- the date as the author writes it (fictional calendars are fine)
    sort_order  INTEGER NOT NULL DEFAULT 0,            -- author-controlled order on the timeline
    scene_id    TEXT REFERENCES scenes (id),           -- scene where the event is shown, if any
    place_id    TEXT REFERENCES entities (id),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX timeline_events_by_book ON timeline_events (book_id, sort_order);

CREATE TABLE timeline_event_participants (
    event_id    TEXT NOT NULL REFERENCES timeline_events (id),
    entity_id   TEXT NOT NULL REFERENCES entities (id),
    role        TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (event_id, entity_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX timeline_event_participants_by_entity ON timeline_event_participants (entity_id);
