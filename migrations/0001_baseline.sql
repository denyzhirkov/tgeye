-- Baseline schema: raw updates, chats, users, messages, ingestion offsets.
-- Timestamps are ISO-8601 UTC text; local time is applied at the edge.

CREATE TABLE telegram_updates (
    update_id   INTEGER PRIMARY KEY,
    payload     TEXT    NOT NULL,
    received_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE chats (
    id            INTEGER PRIMARY KEY, -- Telegram chat id
    kind          TEXT    NOT NULL,    -- private | group | supergroup | channel
    title         TEXT,
    username      TEXT,
    is_forum      INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT    NOT NULL,
    last_seen_at  TEXT    NOT NULL
);

CREATE TABLE users (
    id         INTEGER PRIMARY KEY, -- Telegram user id
    username   TEXT,
    first_name TEXT,
    last_name  TEXT,
    is_bot     INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_seen_at  TEXT NOT NULL
);

CREATE TABLE messages (
    id                    TEXT    PRIMARY KEY, -- internal UUID
    chat_id               INTEGER NOT NULL REFERENCES chats (id),
    telegram_message_id   INTEGER NOT NULL,
    thread_id             INTEGER,
    sender_user_id        INTEGER,
    sender_chat_id        INTEGER,
    reply_to_message_id   INTEGER,
    media_group_id        TEXT,
    kind                  TEXT    NOT NULL, -- text | photo | document | ...
    text                  TEXT,             -- normalized text or caption
    raw_json              TEXT,
    sent_at               TEXT    NOT NULL,
    edited_at             TEXT,
    received_at           TEXT    NOT NULL,
    is_deleted            INTEGER NOT NULL DEFAULT 0, -- local tombstone
    is_service            INTEGER NOT NULL DEFAULT 0,
    has_protected_content INTEGER NOT NULL DEFAULT 0,
    UNIQUE (chat_id, telegram_message_id)
);

CREATE INDEX idx_messages_chat_sent_at ON messages (chat_id, sent_at);
CREATE INDEX idx_messages_chat_thread ON messages (chat_id, thread_id) WHERE thread_id IS NOT NULL;

CREATE TABLE ingestion_offsets (
    id             INTEGER PRIMARY KEY CHECK (id = 1), -- single row
    last_update_id INTEGER NOT NULL,
    updated_at     TEXT    NOT NULL
);
