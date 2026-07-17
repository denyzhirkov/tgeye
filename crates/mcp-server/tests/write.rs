#![allow(clippy::unwrap_used)] // integration test

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use tgeye_domain::media::{MediaError, MediaSource, WriteError, WriteSink};
use tgeye_mcp_server::{ServerContext, TgeyeServer};

struct NoMedia;
#[async_trait::async_trait]
impl MediaSource for NoMedia {
    async fn download(&self, _: &str, _: u64) -> Result<Vec<u8>, MediaError> {
        Err(MediaError::NotFound)
    }
}

/// Records send calls and returns a fixed message id.
struct SpyWrite {
    calls: AtomicUsize,
}
#[async_trait::async_trait]
impl WriteSink for SpyWrite {
    async fn send(&self, _: i64, _: &str, _: Option<i64>) -> Result<i64, WriteError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(555)
    }
}

const CHAT: i64 = -4242;

async fn setup(allow_write: bool, write_allowed_chat: bool) -> (TgeyeServer, Arc<SpyWrite>) {
    let dir = tempfile::tempdir().unwrap().keep();
    let pool = tgeye_storage::connect(&dir.join("db.sqlite3"))
        .await
        .unwrap();
    tgeye_storage::run_migrations(&pool).await.unwrap();
    if write_allowed_chat {
        let mut conn = pool.acquire().await.unwrap();
        tgeye_storage::repo::set_chat_write_rule(&mut conn, CHAT, true, Utc::now())
            .await
            .unwrap();
    }
    let ctx = ServerContext {
        version: "test".into(),
        bot_id: 1,
        bot_username: "bot".into(),
        timezone: chrono_tz::UTC,
        default_page_size: 100,
        max_page_size: 500,
        require_chat_allowlist: true,
        media_root: dir,
        max_download_bytes: 1024,
        expose_local_path: false,
        allow_media_download: false,
        allow_write_tools: allow_write,
    };
    let spy = Arc::new(SpyWrite {
        calls: AtomicUsize::new(0),
    });
    let server = TgeyeServer::new(pool, ctx, Arc::new(NoMedia), spy.clone());
    (server, spy)
}

fn sc(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    result.structured_content.clone().unwrap()
}

#[tokio::test]
async fn send_succeeds_when_enabled_and_chat_allowed() {
    let (server, spy) = setup(true, true).await;
    let out = sc(&server
        .send_for_test(&CHAT.to_string(), "daily report: 3 tasks done", None)
        .await);
    assert_eq!(out["sent"], true);
    assert_eq!(out["message_id"], 555);
    assert_eq!(spy.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn send_blocked_when_tools_disabled() {
    let (server, spy) = setup(false, true).await;
    let out = sc(&server.send_for_test(&CHAT.to_string(), "hi", None).await);
    assert_eq!(out["code"], "WRITE_TOOLS_DISABLED");
    assert_eq!(
        spy.calls.load(Ordering::SeqCst),
        0,
        "must not reach the sink"
    );
}

#[tokio::test]
async fn send_blocked_when_chat_not_write_allowed() {
    let (server, spy) = setup(true, false).await;
    let out = sc(&server.send_for_test(&CHAT.to_string(), "hi", None).await);
    assert_eq!(out["code"], "WRITE_NOT_ALLOWED_FOR_CHAT");
    assert_eq!(spy.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn send_rejects_empty_and_oversized() {
    let (server, spy) = setup(true, true).await;
    assert_eq!(
        sc(&server.send_for_test(&CHAT.to_string(), "   ", None).await)["code"],
        "INVALID_ARGUMENT"
    );
    let huge = "a".repeat(5000);
    assert_eq!(
        sc(&server.send_for_test(&CHAT.to_string(), &huge, None).await)["code"],
        "INVALID_ARGUMENT"
    );
    assert_eq!(spy.calls.load(Ordering::SeqCst), 0);
}
