#![allow(clippy::unwrap_used)] // integration test

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tgeye_domain::media::{MediaError, MediaSource, WriteError, WriteSink};
use tgeye_domain::{
    AttachmentMeta, ChatInfo, ChatKind, IncomingMessage, MessageContentKind, UserInfo,
};
use tgeye_mcp_server::{ServerContext, TgeyeServer};

/// Serves fixed bytes for any file_id; records how many times it was hit.
struct FakeMedia {
    bytes: Vec<u8>,
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl MediaSource for FakeMedia {
    async fn download(&self, _file_id: &str, max_bytes: u64) -> Result<Vec<u8>, MediaError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.bytes.len() as u64 > max_bytes {
            return Err(MediaError::TooLarge {
                size_bytes: self.bytes.len() as u64,
                max_bytes,
            });
        }
        Ok(self.bytes.clone())
    }
}

/// Write sink that always fails — media tests never call it.
struct NoWrite;

#[async_trait::async_trait]
impl WriteSink for NoWrite {
    async fn send(&self, _: i64, _: &str, _: Option<i64>) -> Result<i64, WriteError> {
        Err(WriteError::Transport("not used".into()))
    }
}

fn doc_message() -> IncomingMessage {
    IncomingMessage {
        chat: ChatInfo {
            id: -900,
            kind: ChatKind::Group,
            title: Some("Files".into()),
            username: None,
            is_forum: false,
        },
        sender: Some(UserInfo {
            id: 7,
            username: Some("dz".into()),
            first_name: "Denis".into(),
            last_name: None,
            is_bot: false,
        }),
        sender_chat_id: None,
        telegram_message_id: 50,
        thread_id: None,
        reply_to_message_id: None,
        media_group_id: None,
        kind: MessageContentKind::Document,
        text: Some("log".into()),
        sent_at: Utc::now(),
        edited_at: None,
        is_service: false,
        has_protected_content: false,
        attachments: vec![AttachmentMeta {
            kind: MessageContentKind::Document,
            file_id: "tg-file-id".into(),
            file_unique_id: "uniq".into(),
            file_name: Some("app.log".into()),
            mime_type: Some("text/plain".into()),
            size_bytes: Some(5),
            width: None,
            height: None,
            duration_secs: None,
        }],
        referenced: vec![],
    }
}

async fn setup(
    media: Arc<FakeMedia>,
    media_root: PathBuf,
) -> (TgeyeServer, sqlx::SqlitePool, String) {
    let dir = tempfile::tempdir().unwrap().keep();
    let pool = tgeye_storage::connect(&dir.join("db.sqlite3"))
        .await
        .unwrap();
    tgeye_storage::run_migrations(&pool).await.unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let msg = doc_message();
    let now = Utc::now();
    tgeye_storage::repo::upsert_chat(&mut conn, &msg.chat, now)
        .await
        .unwrap();
    tgeye_storage::repo::set_chat_rule(&mut conn, msg.chat.id, true, now)
        .await
        .unwrap();
    let row_id = tgeye_storage::repo::insert_message(&mut conn, &msg, now)
        .await
        .unwrap()
        .unwrap();
    tgeye_storage::repo::insert_attachments(&mut conn, &row_id, &msg.attachments)
        .await
        .unwrap();
    let attachment_id: String = sqlx::query_scalar("SELECT id FROM attachments LIMIT 1")
        .fetch_one(&mut *conn)
        .await
        .unwrap();

    let ctx = ServerContext {
        version: "test".into(),
        bot_id: 1,
        bot_username: "bot".into(),
        timezone: chrono_tz::UTC,
        default_page_size: 100,
        max_page_size: 500,
        require_chat_allowlist: true,
        media_root,
        max_download_bytes: 10 * 1024 * 1024,
        expose_local_path: true,
        allow_media_download: true,
        allow_write_tools: false,
    };
    (
        TgeyeServer::new(pool.clone(), ctx, media, Arc::new(NoWrite)),
        pool,
        attachment_id,
    )
}

fn structured(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    result.structured_content.clone().unwrap()
}

#[tokio::test]
async fn download_writes_file_dedups_and_reports_path() {
    let media_root = tempfile::tempdir().unwrap().keep();
    let media = Arc::new(FakeMedia {
        bytes: b"hello".to_vec(),
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let (server, _pool, attachment_id) = setup(media.clone(), media_root.clone()).await;

    let first = server
        .download_attachment_for_test(&attachment_id, false)
        .await;
    let first = structured(&first);
    assert_eq!(first["downloaded"], true);
    assert_eq!(first["deduplicated"], false);
    // sha256("hello")
    assert_eq!(
        first["sha256"],
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    let path = first["local_path"].as_str().unwrap();
    assert!(std::path::Path::new(path).exists(), "file must be written");
    assert!(
        path.ends_with(".txt"),
        "text/plain → .txt extension, got {path}"
    );

    // Second call: already downloaded → dedup, no new network hit.
    let second = structured(
        &server
            .download_attachment_for_test(&attachment_id, false)
            .await,
    );
    assert_eq!(second["deduplicated"], true);
    assert_eq!(media.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn download_refuses_when_disabled() {
    let media_root = tempfile::tempdir().unwrap().keep();
    let media = Arc::new(FakeMedia {
        bytes: b"hello".to_vec(),
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let (mut server, _pool, attachment_id) = setup(media, media_root).await;
    server.set_allow_media_download_for_test(false);

    let result = server
        .download_attachment_for_test(&attachment_id, false)
        .await;
    assert_eq!(structured(&result)["code"], "MEDIA_DOWNLOAD_DISABLED");
}
