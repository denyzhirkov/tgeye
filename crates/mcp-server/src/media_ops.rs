use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;
use tgeye_storage::attachments::{self, AttachmentDetail};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Metadata view of an attachment — never includes the bot token or a Bot API URL.
/// `local_path` is filled only when the file is downloaded and paths are exposed.
pub async fn attachment_json(
    conn: &mut SqliteConnection,
    detail: &AttachmentDetail,
    media_root: &Path,
    expose_local_path: bool,
) -> serde_json::Value {
    let mut local_path = serde_json::Value::Null;
    if let Some(sha) = &detail.sha256
        && expose_local_path
        && let Ok(Some((category, extension))) = attachments::file_by_sha256(conn, sha).await
    {
        local_path = serde_json::Value::String(
            media_root
                .join(&category)
                .join(format!("{sha}.{extension}"))
                .to_string_lossy()
                .into_owned(),
        );
    }
    serde_json::json!({
        "attachment_id": detail.id,
        "chat_id": detail.chat_id.to_string(),
        "message_id": detail.telegram_message_id,
        "kind": detail.kind,
        "file_name": detail.file_name,
        "mime_type": detail.mime_type,
        "size_bytes": detail.size_bytes,
        "width": detail.width,
        "height": detail.height,
        "duration_secs": detail.duration_secs,
        "telegram_file_unique_id": detail.telegram_file_unique_id,
        "downloaded": detail.sha256.is_some(),
        "sha256": detail.sha256,
        "downloaded_at": detail.downloaded_at,
        "resource_uri": format!("telegram-media://attachment/{}", detail.id),
        "local_path": local_path,
    })
}
