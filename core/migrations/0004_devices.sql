-- Devices that may use the API. Each device has one random token; only its SHA-256 hash is stored,
-- so a leaked database cannot be used to log in, and a lost token cannot be shown again.
-- A device is revoked, never deleted: its name stays in the Git history and the canon ledger, so a
-- revoked name is not reused (the name column is unique across revoked devices too).

CREATE TABLE devices (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 60),
    token_hash   TEXT NOT NULL UNIQUE CHECK (length(token_hash) = 64),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_seen_at TEXT,
    revoked_at   TEXT
) STRICT;
