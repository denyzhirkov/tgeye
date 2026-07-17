-- Full-text search over message text (spec §17).
-- Standalone FTS5 (not external-content): rows are keyed by the message UUID, so
-- VACUUM/backup renumbering the implicit rowid of `messages` cannot desync it.
-- unicode61 tokenizer folds case and splits Cyrillic + Latin without stemming.

CREATE VIRTUAL TABLE messages_fts USING fts5(
    message_id UNINDEXED,
    text,
    tokenize = 'unicode61'
);

CREATE TRIGGER messages_fts_ai AFTER INSERT ON messages
WHEN new.text IS NOT NULL
BEGIN
    INSERT INTO messages_fts (message_id, text) VALUES (new.id, new.text);
END;

CREATE TRIGGER messages_fts_ad AFTER DELETE ON messages
BEGIN
    DELETE FROM messages_fts WHERE message_id = old.id;
END;

CREATE TRIGGER messages_fts_au AFTER UPDATE ON messages
BEGIN
    DELETE FROM messages_fts WHERE message_id = old.id;
    INSERT INTO messages_fts (message_id, text)
    SELECT new.id, new.text WHERE new.text IS NOT NULL;
END;

INSERT INTO messages_fts (message_id, text)
SELECT id, text FROM messages WHERE text IS NOT NULL;
