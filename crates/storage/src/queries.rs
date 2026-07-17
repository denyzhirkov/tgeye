use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};

use crate::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

const MESSAGE_COLUMNS: &str = "id, chat_id, telegram_message_id, thread_id, sender_user_id, \
     sender_chat_id, reply_to_message_id, media_group_id, kind, text, sent_at, edited_at, is_service";

fn message_from_row(r: &SqliteRow) -> MessageRow {
    MessageRow {
        id: r.get("id"),
        chat_id: r.get("chat_id"),
        telegram_message_id: r.get("telegram_message_id"),
        thread_id: r.get("thread_id"),
        sender_user_id: r.get("sender_user_id"),
        sender_chat_id: r.get("sender_chat_id"),
        reply_to_message_id: r.get("reply_to_message_id"),
        media_group_id: r.get("media_group_id"),
        kind: r.get("kind"),
        text: r.get("text"),
        sent_at: r.get("sent_at"),
        edited_at: r.get("edited_at"),
        is_service: r.get("is_service"),
    }
}

#[derive(Debug, Clone)]
pub struct ChatRow {
    pub id: i64,
    pub kind: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub is_forum: bool,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub chat_id: i64,
    pub telegram_message_id: i64,
    pub thread_id: Option<i64>,
    pub sender_user_id: Option<i64>,
    pub sender_chat_id: Option<i64>,
    pub reply_to_message_id: Option<i64>,
    pub media_group_id: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub sent_at: String,
    pub edited_at: Option<String>,
    pub is_service: bool,
}

#[derive(Debug, Clone)]
pub struct AuthorRow {
    pub id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Clone)]
pub struct AttachmentRow {
    pub id: String,
    pub kind: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_secs: Option<i64>,
}

/// Time bounds and cursor are pre-formatted UTC RFC3339 (millis, Z) strings —
/// the same format `repo::fmt` writes, so lexicographic comparison is correct.
#[derive(Debug, Clone, Default)]
pub struct MessageQuery {
    pub chat_id: i64,
    /// Inclusive lower bound on sent_at.
    pub from: Option<String>,
    /// Exclusive upper bound on sent_at.
    pub to: Option<String>,
    /// Keyset cursor: strictly after (asc) / before (desc) this position.
    pub after: Option<(String, i64)>,
    pub ascending: bool,
    pub include_service: bool,
    pub limit: i64,
}

pub async fn get_chat(conn: &mut SqliteConnection, chat_id: i64) -> Result<Option<ChatRow>> {
    let row = sqlx::query("SELECT id, kind, title, username, is_forum FROM chats WHERE id = ?")
        .bind(chat_id)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|r| ChatRow {
        id: r.get("id"),
        kind: r.get("kind"),
        title: r.get("title"),
        username: r.get("username"),
        is_forum: r.get("is_forum"),
    }))
}

pub async fn query_messages(
    conn: &mut SqliteConnection,
    query: &MessageQuery,
) -> Result<Vec<MessageRow>> {
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, chat_id, telegram_message_id, thread_id, sender_user_id, sender_chat_id,
                reply_to_message_id, media_group_id, kind, text, sent_at, edited_at, is_service
         FROM messages WHERE is_deleted = 0 AND chat_id = ",
    );
    builder.push_bind(query.chat_id);
    if !query.include_service {
        builder.push(" AND is_service = 0");
    }
    if let Some(from) = &query.from {
        builder.push(" AND sent_at >= ").push_bind(from.clone());
    }
    if let Some(to) = &query.to {
        builder.push(" AND sent_at < ").push_bind(to.clone());
    }
    if let Some((sent_at, message_id)) = &query.after {
        let (cmp1, cmp2) = if query.ascending {
            (">", ">")
        } else {
            ("<", "<")
        };
        builder
            .push(" AND (sent_at ")
            .push(cmp1)
            .push(" ")
            .push_bind(sent_at.clone())
            .push(" OR (sent_at = ")
            .push_bind(sent_at.clone())
            .push(" AND telegram_message_id ")
            .push(cmp2)
            .push(" ")
            .push_bind(*message_id)
            .push("))");
    }
    let direction = if query.ascending { "ASC" } else { "DESC" };
    builder
        .push(format!(
            " ORDER BY sent_at {direction}, telegram_message_id {direction} LIMIT "
        ))
        .push_bind(query.limit);

    let rows = builder.build().fetch_all(conn).await?;
    Ok(rows.iter().map(message_from_row).collect())
}

pub async fn get_message(
    conn: &mut SqliteConnection,
    chat_id: i64,
    telegram_message_id: i64,
) -> Result<Option<MessageRow>> {
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE is_deleted = 0 AND chat_id = ? AND telegram_message_id = ?"
    );
    let row = sqlx::query(&sql)
        .bind(chat_id)
        .bind(telegram_message_id)
        .fetch_optional(conn)
        .await?;
    Ok(row.as_ref().map(message_from_row))
}

/// `count` messages on one side of the anchor, ordered outward then returned in
/// chronological order. `newer = true` selects messages sent at/after the anchor
/// position; `false` selects strictly before it.
pub async fn context_side(
    conn: &mut SqliteConnection,
    chat_id: i64,
    anchor_sent_at: &str,
    anchor_message_id: i64,
    newer: bool,
    include_anchor: bool,
    count: i64,
) -> Result<Vec<MessageRow>> {
    if count <= 0 {
        return Ok(vec![]);
    }
    let (op, dir) = if newer { (">", "ASC") } else { ("<", "DESC") };
    let eq = if include_anchor { "=" } else { "" };
    let sql = format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages
         WHERE is_deleted = 0 AND chat_id = ?
           AND (sent_at {op} ? OR (sent_at = ? AND telegram_message_id {op}{eq} ?))
         ORDER BY sent_at {dir}, telegram_message_id {dir} LIMIT ?"
    );
    let mut rows: Vec<MessageRow> = sqlx::query(&sql)
        .bind(chat_id)
        .bind(anchor_sent_at)
        .bind(anchor_sent_at)
        .bind(anchor_message_id)
        .bind(count)
        .fetch_all(conn)
        .await?
        .iter()
        .map(message_from_row)
        .collect();
    if !newer {
        rows.reverse(); // DESC fetch → chronological
    }
    Ok(rows)
}

/// Walk reply_to_message_id upward from the anchor (excluded), oldest ancestor last
/// in walk order; bounded by `max_depth`. Cycles are broken by the visited set.
pub async fn reply_ancestors(
    conn: &mut SqliteConnection,
    chat_id: i64,
    start_reply_to: Option<i64>,
    max_depth: usize,
) -> Result<Vec<MessageRow>> {
    let mut ancestors = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut next = start_reply_to;
    while let Some(target) = next {
        if ancestors.len() >= max_depth || !seen.insert(target) {
            break;
        }
        match get_message(conn, chat_id, target).await? {
            Some(msg) => {
                next = msg.reply_to_message_id;
                ancestors.push(msg);
            }
            None => break,
        }
    }
    Ok(ancestors)
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub message: MessageRow,
    pub snippet: String,
    pub rank: f64,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Sanitized FTS5 MATCH expression (see `crate::fts`).
    pub match_expr: String,
    /// Empty = all allowed chats the caller already resolved.
    pub chat_ids: Vec<i64>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: i64,
}

pub async fn search_messages(
    conn: &mut SqliteConnection,
    query: &SearchQuery,
) -> Result<Vec<SearchHit>> {
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT m.id, m.chat_id, m.telegram_message_id, m.thread_id, m.sender_user_id,
                m.sender_chat_id, m.reply_to_message_id, m.media_group_id, m.kind, m.text,
                m.sent_at, m.edited_at, m.is_service,
                snippet(messages_fts, 1, '[', ']', '…', 12) AS snippet,
                messages_fts.rank AS rank
         FROM messages_fts
         JOIN messages m ON m.id = messages_fts.message_id
         WHERE messages_fts MATCH ",
    );
    builder.push_bind(query.match_expr.clone());
    builder.push(" AND m.is_deleted = 0");
    if !query.chat_ids.is_empty() {
        builder.push(" AND m.chat_id IN (");
        let mut separated = builder.separated(", ");
        for chat_id in &query.chat_ids {
            separated.push_bind(*chat_id);
        }
        builder.push(")");
    }
    if let Some(from) = &query.from {
        builder.push(" AND m.sent_at >= ").push_bind(from.clone());
    }
    if let Some(to) = &query.to {
        builder.push(" AND m.sent_at < ").push_bind(to.clone());
    }
    builder.push(" ORDER BY rank LIMIT ").push_bind(query.limit);

    let rows = builder.build().fetch_all(conn).await?;
    Ok(rows
        .iter()
        .map(|r| SearchHit {
            message: message_from_row(r),
            snippet: r.get("snippet"),
            rank: r.get("rank"),
        })
        .collect())
}

pub async fn get_author(conn: &mut SqliteConnection, user_id: i64) -> Result<Option<AuthorRow>> {
    let row =
        sqlx::query("SELECT id, username, first_name, last_name, is_bot FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(conn)
            .await?;
    Ok(row.map(|r| AuthorRow {
        id: r.get("id"),
        username: r.get("username"),
        first_name: r.get("first_name"),
        last_name: r.get("last_name"),
        is_bot: r.get("is_bot"),
    }))
}

pub async fn attachments_for_message(
    conn: &mut SqliteConnection,
    message_row_id: &str,
) -> Result<Vec<AttachmentRow>> {
    let rows = sqlx::query(
        "SELECT id, kind, file_name, mime_type, size_bytes, width, height, duration_secs
         FROM attachments WHERE message_id = ?",
    )
    .bind(message_row_id)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AttachmentRow {
            id: r.get("id"),
            kind: r.get("kind"),
            file_name: r.get("file_name"),
            mime_type: r.get("mime_type"),
            size_bytes: r.get("size_bytes"),
            width: r.get("width"),
            height: r.get("height"),
            duration_secs: r.get("duration_secs"),
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct ChatStats {
    pub message_count: i64,
    pub first_message_at: Option<String>,
    pub last_message_at: Option<String>,
    pub edited_count: i64,
}

pub async fn chat_stats(conn: &mut SqliteConnection, chat_id: i64) -> Result<ChatStats> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS message_count,
                MIN(sent_at) AS first_message_at,
                MAX(sent_at) AS last_message_at,
                COALESCE(SUM(edited_at IS NOT NULL), 0) AS edited_count
         FROM messages WHERE is_deleted = 0 AND chat_id = ?",
    )
    .bind(chat_id)
    .fetch_one(conn)
    .await?;
    Ok(ChatStats {
        message_count: row.get("message_count"),
        first_message_at: row.get("first_message_at"),
        last_message_at: row.get("last_message_at"),
        edited_count: row.get("edited_count"),
    })
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub chat_count: i64,
    pub message_count: i64,
    pub last_update_received_at: Option<String>,
}

pub async fn stats(conn: &mut SqliteConnection) -> Result<StorageStats> {
    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM chats) AS chat_count,
                (SELECT COUNT(*) FROM messages) AS message_count,
                (SELECT MAX(received_at) FROM telegram_updates) AS last_update_received_at",
    )
    .fetch_one(conn)
    .await?;
    Ok(StorageStats {
        chat_count: row.get("chat_count"),
        message_count: row.get("message_count"),
        last_update_received_at: row.get("last_update_received_at"),
    })
}
