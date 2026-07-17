use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};

use crate::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

fn fmt(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::Millis, true)
}

const ATTACHMENT_COLUMNS: &str = "a.id, a.message_id, a.kind, a.telegram_file_id, \
     a.telegram_file_unique_id, a.file_name, a.mime_type, a.size_bytes, a.width, a.height, \
     a.duration_secs, a.sha256, a.downloaded_at, m.chat_id, m.telegram_message_id, m.sent_at";

#[derive(Debug, Clone)]
pub struct AttachmentDetail {
    pub id: String,
    pub message_row_id: String,
    pub chat_id: i64,
    pub telegram_message_id: i64,
    pub sent_at: String,
    pub kind: String,
    pub telegram_file_id: String,
    pub telegram_file_unique_id: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_secs: Option<i64>,
    pub sha256: Option<String>,
    pub downloaded_at: Option<String>,
}

fn attachment_from_row(r: &SqliteRow) -> AttachmentDetail {
    AttachmentDetail {
        id: r.get("id"),
        message_row_id: r.get("message_id"),
        chat_id: r.get("chat_id"),
        telegram_message_id: r.get("telegram_message_id"),
        sent_at: r.get("sent_at"),
        kind: r.get("kind"),
        telegram_file_id: r.get("telegram_file_id"),
        telegram_file_unique_id: r.get("telegram_file_unique_id"),
        file_name: r.get("file_name"),
        mime_type: r.get("mime_type"),
        size_bytes: r.get("size_bytes"),
        width: r.get("width"),
        height: r.get("height"),
        duration_secs: r.get("duration_secs"),
        sha256: r.get("sha256"),
        downloaded_at: r.get("downloaded_at"),
    }
}

pub async fn get_attachment(
    conn: &mut SqliteConnection,
    attachment_id: &str,
) -> Result<Option<AttachmentDetail>> {
    let sql = format!(
        "SELECT {ATTACHMENT_COLUMNS} FROM attachments a
         JOIN messages m ON m.id = a.message_id WHERE a.id = ?"
    );
    let row = sqlx::query(&sql)
        .bind(attachment_id)
        .fetch_optional(conn)
        .await?;
    Ok(row.as_ref().map(attachment_from_row))
}

#[derive(Debug, Clone, Default)]
pub struct AttachmentQuery {
    pub chat_id: i64,
    pub from: Option<String>,
    pub to: Option<String>,
    pub kinds: Vec<String>,
    pub downloaded_only: bool,
    pub limit: i64,
}

pub async fn list_attachments(
    conn: &mut SqliteConnection,
    query: &AttachmentQuery,
) -> Result<Vec<AttachmentDetail>> {
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT ");
    builder.push(ATTACHMENT_COLUMNS);
    builder.push(
        " FROM attachments a JOIN messages m ON m.id = a.message_id
          WHERE m.is_deleted = 0 AND m.chat_id = ",
    );
    builder.push_bind(query.chat_id);
    if let Some(from) = &query.from {
        builder.push(" AND m.sent_at >= ").push_bind(from.clone());
    }
    if let Some(to) = &query.to {
        builder.push(" AND m.sent_at < ").push_bind(to.clone());
    }
    if query.downloaded_only {
        builder.push(" AND a.sha256 IS NOT NULL");
    }
    if !query.kinds.is_empty() {
        builder.push(" AND a.kind IN (");
        let mut separated = builder.separated(", ");
        for kind in &query.kinds {
            separated.push_bind(kind.clone());
        }
        builder.push(")");
    }
    builder
        .push(" ORDER BY m.sent_at DESC, a.id LIMIT ")
        .push_bind(query.limit);

    let rows = builder.build().fetch_all(conn).await?;
    Ok(rows.iter().map(attachment_from_row).collect())
}

pub async fn media_group(
    conn: &mut SqliteConnection,
    chat_id: i64,
    media_group_id: &str,
) -> Result<Vec<AttachmentDetail>> {
    let sql = format!(
        "SELECT {ATTACHMENT_COLUMNS} FROM attachments a
         JOIN messages m ON m.id = a.message_id
         WHERE m.chat_id = ? AND m.media_group_id = ? AND m.is_deleted = 0
         ORDER BY m.telegram_message_id, a.id"
    );
    let rows = sqlx::query(&sql)
        .bind(chat_id)
        .bind(media_group_id)
        .fetch_all(conn)
        .await?;
    Ok(rows.iter().map(attachment_from_row).collect())
}

/// Existing stored file for this content, if any (dedup lookup).
pub async fn file_by_sha256(
    conn: &mut SqliteConnection,
    sha256: &str,
) -> Result<Option<(String, String)>> {
    let row = sqlx::query("SELECT category, extension FROM attachment_files WHERE sha256 = ?")
        .bind(sha256)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|r| (r.get("category"), r.get("extension"))))
}

/// Record a downloaded file and link the attachment to it, transactionally.
#[allow(clippy::too_many_arguments)]
pub async fn record_download(
    conn: &mut SqliteConnection,
    attachment_id: &str,
    sha256: &str,
    byte_size: i64,
    extension: &str,
    category: &str,
    mime_type: Option<&str>,
    at: DateTime<Utc>,
) -> Result<()> {
    let at = fmt(at);
    sqlx::query(
        "INSERT INTO attachment_files (sha256, byte_size, extension, category, mime_type, downloaded_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (sha256) DO NOTHING",
    )
    .bind(sha256)
    .bind(byte_size)
    .bind(extension)
    .bind(category)
    .bind(mime_type)
    .bind(&at)
    .execute(&mut *conn)
    .await?;
    sqlx::query("UPDATE attachments SET sha256 = ?, downloaded_at = ? WHERE id = ?")
        .bind(sha256)
        .bind(&at)
        .bind(attachment_id)
        .execute(conn)
        .await?;
    Ok(())
}
