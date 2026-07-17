-- Collector phase: edit history, allowlist rules, attachment metadata.

CREATE TABLE message_versions (
    id          TEXT PRIMARY KEY, -- internal UUID
    message_id  TEXT NOT NULL REFERENCES messages (id),
    text        TEXT,             -- content being replaced
    edited_at   TEXT,             -- edit timestamp of the replaced version (NULL = original)
    replaced_at TEXT NOT NULL
);

CREATE INDEX idx_message_versions_message ON message_versions (message_id);

CREATE TABLE chat_access_rules (
    chat_id    INTEGER PRIMARY KEY,
    allowed    INTEGER NOT NULL, -- 1 allow, 0 deny
    updated_at TEXT    NOT NULL
);

CREATE TABLE attachments (
    id                      TEXT    PRIMARY KEY, -- internal UUID
    message_id              TEXT    NOT NULL REFERENCES messages (id),
    kind                    TEXT    NOT NULL,    -- photo | document | voice | ...
    telegram_file_id        TEXT    NOT NULL,
    telegram_file_unique_id TEXT    NOT NULL,
    file_name               TEXT,
    mime_type               TEXT,
    size_bytes              INTEGER,
    width                   INTEGER,
    height                  INTEGER,
    duration_secs           INTEGER
);

CREATE INDEX idx_attachments_message ON attachments (message_id);
