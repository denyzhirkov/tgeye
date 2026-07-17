use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::{Row, SqliteConnection};
use tgeye_domain::{AttachmentMeta, ChatInfo, IncomingMessage, UserInfo};
use uuid::Uuid;

use crate::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

fn fmt(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// `false` when this update_id was already recorded (duplicate delivery).
pub async fn record_raw_update(
    conn: &mut SqliteConnection,
    update_id: i64,
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<bool> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO telegram_updates (update_id, payload, received_at) VALUES (?, ?, ?)",
    )
    .bind(update_id)
    .bind(payload)
    .bind(fmt(received_at))
    .execute(conn)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn upsert_chat(
    conn: &mut SqliteConnection,
    chat: &ChatInfo,
    seen_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO chats (id, kind, title, username, is_forum, first_seen_at, last_seen_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             kind = excluded.kind,
             title = excluded.title,
             username = excluded.username,
             is_forum = excluded.is_forum,
             last_seen_at = excluded.last_seen_at",
    )
    .bind(chat.id)
    .bind(chat.kind.as_str())
    .bind(&chat.title)
    .bind(&chat.username)
    .bind(chat.is_forum)
    .bind(fmt(seen_at))
    .bind(fmt(seen_at))
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn upsert_user(
    conn: &mut SqliteConnection,
    user: &UserInfo,
    seen_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO users (id, username, first_name, last_name, is_bot, first_seen_at, last_seen_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             username = excluded.username,
             first_name = excluded.first_name,
             last_name = excluded.last_name,
             is_bot = excluded.is_bot,
             last_seen_at = excluded.last_seen_at",
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.first_name)
    .bind(&user.last_name)
    .bind(user.is_bot)
    .bind(fmt(seen_at))
    .bind(fmt(seen_at))
    .execute(conn)
    .await?;
    Ok(())
}

/// Internal row id when inserted; `None` on duplicate `(chat_id, telegram_message_id)`.
pub async fn insert_message(
    conn: &mut SqliteConnection,
    msg: &IncomingMessage,
    received_at: DateTime<Utc>,
) -> Result<Option<String>> {
    let id = Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT OR IGNORE INTO messages
             (id, chat_id, telegram_message_id, thread_id, sender_user_id, sender_chat_id,
              reply_to_message_id, media_group_id, kind, text, sent_at, edited_at, received_at,
              is_service, has_protected_content)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(msg.chat.id)
    .bind(msg.telegram_message_id)
    .bind(msg.thread_id)
    .bind(msg.sender.as_ref().map(|s| s.id))
    .bind(msg.sender_chat_id)
    .bind(msg.reply_to_message_id)
    .bind(&msg.media_group_id)
    .bind(msg.kind.as_str())
    .bind(&msg.text)
    .bind(fmt(msg.sent_at))
    .bind(msg.edited_at.map(fmt))
    .bind(fmt(received_at))
    .bind(msg.is_service)
    .bind(msg.has_protected_content)
    .execute(conn)
    .await?;
    Ok((result.rows_affected() > 0).then_some(id))
}

pub async fn insert_attachments(
    conn: &mut SqliteConnection,
    message_row_id: &str,
    attachments: &[AttachmentMeta],
) -> Result<()> {
    for attachment in attachments {
        sqlx::query(
            "INSERT INTO attachments
                 (id, message_id, kind, telegram_file_id, telegram_file_unique_id,
                  file_name, mime_type, size_bytes, width, height, duration_secs)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(message_row_id)
        .bind(attachment.kind.as_str())
        .bind(&attachment.file_id)
        .bind(&attachment.file_unique_id)
        .bind(&attachment.file_name)
        .bind(&attachment.mime_type)
        .bind(attachment.size_bytes)
        .bind(attachment.width)
        .bind(attachment.height)
        .bind(attachment.duration_secs)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditOutcome {
    /// Previous content moved to message_versions, main row updated.
    Versioned,
    /// Original was never stored (predates the bot) — inserted as a new message.
    InsertedAsNew,
    /// Original absent and insert hit a duplicate — nothing changed.
    Skipped,
}

pub async fn apply_edit(
    conn: &mut SqliteConnection,
    msg: &IncomingMessage,
    received_at: DateTime<Utc>,
) -> Result<EditOutcome> {
    let existing = sqlx::query(
        "SELECT id, text, edited_at FROM messages WHERE chat_id = ? AND telegram_message_id = ?",
    )
    .bind(msg.chat.id)
    .bind(msg.telegram_message_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(row) = existing else {
        return Ok(match insert_message(conn, msg, received_at).await? {
            Some(_) => EditOutcome::InsertedAsNew,
            None => EditOutcome::Skipped,
        });
    };

    let row_id: String = row.get("id");
    let old_text: Option<String> = row.get("text");
    let old_edited_at: Option<String> = row.get("edited_at");

    sqlx::query(
        "INSERT INTO message_versions (id, message_id, text, edited_at, replaced_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&row_id)
    .bind(&old_text)
    .bind(&old_edited_at)
    .bind(fmt(received_at))
    .execute(&mut *conn)
    .await?;

    sqlx::query("UPDATE messages SET text = ?, edited_at = ? WHERE id = ?")
        .bind(&msg.text)
        .bind(msg.edited_at.map(fmt))
        .bind(&row_id)
        .execute(conn)
        .await?;

    Ok(EditOutcome::Versioned)
}

pub async fn chat_rule(conn: &mut SqliteConnection, chat_id: i64) -> Result<Option<bool>> {
    let row = sqlx::query("SELECT allowed FROM chat_access_rules WHERE chat_id = ?")
        .bind(chat_id)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|r| r.get::<bool, _>("allowed")))
}

pub async fn set_chat_rule(
    conn: &mut SqliteConnection,
    chat_id: i64,
    allowed: bool,
    updated_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO chat_access_rules (chat_id, allowed, updated_at) VALUES (?, ?, ?)
         ON CONFLICT (chat_id) DO UPDATE SET allowed = excluded.allowed, updated_at = excluded.updated_at",
    )
    .bind(chat_id)
    .bind(allowed)
    .bind(fmt(updated_at))
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(Debug)]
pub struct ChatSummary {
    pub id: i64,
    pub kind: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub message_count: i64,
    pub rule: Option<bool>,
    pub last_seen_at: String,
}

pub async fn list_chats(conn: &mut SqliteConnection) -> Result<Vec<ChatSummary>> {
    let rows = sqlx::query(
        "SELECT c.id, c.kind, c.title, c.username, c.last_seen_at, r.allowed,
                (SELECT COUNT(*) FROM messages m WHERE m.chat_id = c.id) AS message_count
         FROM chats c
         LEFT JOIN chat_access_rules r ON r.chat_id = c.id
         ORDER BY c.last_seen_at DESC",
    )
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ChatSummary {
            id: r.get("id"),
            kind: r.get("kind"),
            title: r.get("title"),
            username: r.get("username"),
            message_count: r.get("message_count"),
            rule: r.get("allowed"),
            last_seen_at: r.get("last_seen_at"),
        })
        .collect())
}

pub async fn load_offset(conn: &mut SqliteConnection) -> Result<Option<i64>> {
    let row = sqlx::query("SELECT last_update_id FROM ingestion_offsets WHERE id = 1")
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|r| r.get("last_update_id")))
}

pub async fn save_offset(
    conn: &mut SqliteConnection,
    last_update_id: i64,
    updated_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO ingestion_offsets (id, last_update_id, updated_at) VALUES (1, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             last_update_id = excluded.last_update_id,
             updated_at = excluded.updated_at",
    )
    .bind(last_update_id)
    .bind(fmt(updated_at))
    .execute(conn)
    .await?;
    Ok(())
}
