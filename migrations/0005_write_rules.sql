-- Write access is a separate, stricter allowlist than read (spec §10.9): a chat
-- being readable never implies the bot may post to it.

CREATE TABLE chat_write_rules (
    chat_id    INTEGER PRIMARY KEY,
    allowed    INTEGER NOT NULL, -- 1 allow, 0 deny
    updated_at TEXT    NOT NULL
);
