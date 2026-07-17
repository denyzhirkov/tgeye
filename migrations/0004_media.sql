-- Local media download state (spec §18). Files are content-addressed by sha256
-- and deduplicated: identical content downloaded from several messages is one file.

CREATE TABLE attachment_files (
    sha256        TEXT    PRIMARY KEY,
    byte_size     INTEGER NOT NULL,
    extension     TEXT    NOT NULL, -- safe, whitelisted; never from the Telegram filename
    category      TEXT    NOT NULL, -- storage subdir: photos | video | audio | voice | documents | other
    mime_type     TEXT,
    downloaded_at TEXT    NOT NULL
);

-- Links an attachment to its downloaded content; NULL until downloaded.
ALTER TABLE attachments ADD COLUMN sha256 TEXT REFERENCES attachment_files (sha256);
ALTER TABLE attachments ADD COLUMN downloaded_at TEXT;

CREATE INDEX idx_attachments_sha256 ON attachments (sha256) WHERE sha256 IS NOT NULL;
CREATE INDEX idx_attachments_unique_file ON attachments (telegram_file_unique_id);
