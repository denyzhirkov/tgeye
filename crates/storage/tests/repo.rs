#![allow(clippy::unwrap_used)] // integration tests; clippy.toml only covers #[cfg(test)] modules

use chrono::{TimeZone, Utc};
use tgeye_domain::{
    AttachmentMeta, ChatInfo, ChatKind, IncomingMessage, MessageContentKind, UserInfo,
};
use tgeye_storage::repo::{self, EditOutcome};

fn chat() -> ChatInfo {
    ChatInfo {
        id: -100200300,
        kind: ChatKind::Supergroup,
        title: Some("Team".into()),
        username: None,
        is_forum: false,
    }
}

fn message(telegram_message_id: i64, text: &str) -> IncomingMessage {
    IncomingMessage {
        chat: chat(),
        sender: Some(UserInfo {
            id: 42,
            username: Some("dz".into()),
            first_name: "Denis".into(),
            last_name: None,
            is_bot: false,
        }),
        sender_chat_id: None,
        telegram_message_id,
        thread_id: None,
        reply_to_message_id: None,
        media_group_id: None,
        kind: MessageContentKind::Text,
        text: Some(text.into()),
        sent_at: Utc.with_ymd_and_hms(2026, 7, 17, 12, 0, 0).unwrap(),
        edited_at: None,
        is_service: false,
        has_protected_content: false,
        attachments: vec![],
        referenced: vec![],
    }
}

async fn setup() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().unwrap();
    // Keep the file alive for the test; SQLite pool holds it open after this returns.
    let path = dir.keep().join("repo-test.sqlite3");
    let pool = tgeye_storage::connect(&path).await.unwrap();
    tgeye_storage::run_migrations(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn raw_update_replay_is_deduplicated() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();
    assert!(
        repo::record_raw_update(&mut conn, 500, "{}", now)
            .await
            .unwrap()
    );
    assert!(
        !repo::record_raw_update(&mut conn, 500, "{}", now)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn message_flow_upsert_insert_duplicate() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();
    let msg = message(10, "hello");

    repo::upsert_chat(&mut conn, &msg.chat, now).await.unwrap();
    repo::upsert_chat(&mut conn, &msg.chat, now).await.unwrap(); // idempotent
    repo::upsert_user(&mut conn, msg.sender.as_ref().unwrap(), now)
        .await
        .unwrap();

    let inserted = repo::insert_message(&mut conn, &msg, now).await.unwrap();
    assert!(inserted.is_some());
    let duplicate = repo::insert_message(&mut conn, &msg, now).await.unwrap();
    assert!(
        duplicate.is_none(),
        "same (chat, message_id) must not duplicate"
    );
}

#[tokio::test]
async fn edit_creates_version_and_updates_main_row() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();
    let msg = message(11, "original");
    repo::upsert_chat(&mut conn, &msg.chat, now).await.unwrap();
    repo::insert_message(&mut conn, &msg, now).await.unwrap();

    let mut edited = message(11, "fixed");
    edited.edited_at = Some(now);
    let outcome = repo::apply_edit(&mut conn, &edited, now).await.unwrap();
    assert_eq!(outcome, EditOutcome::Versioned);

    let versions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message_versions")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(versions, 1);
    let text: String =
        sqlx::query_scalar("SELECT text FROM messages WHERE telegram_message_id = 11")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(text, "fixed");
}

#[tokio::test]
async fn edit_of_unknown_message_inserts_as_new() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();
    let mut edited = message(12, "late edit");
    edited.edited_at = Some(now);
    repo::upsert_chat(&mut conn, &edited.chat, now)
        .await
        .unwrap();
    let outcome = repo::apply_edit(&mut conn, &edited, now).await.unwrap();
    assert_eq!(outcome, EditOutcome::InsertedAsNew);
}

#[tokio::test]
async fn attachments_stored_for_message() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();
    let mut msg = message(13, "with file");
    msg.kind = MessageContentKind::Document;
    msg.attachments = vec![AttachmentMeta {
        kind: MessageContentKind::Document,
        file_id: "file-abc".into(),
        file_unique_id: "uniq-abc".into(),
        file_name: Some("report.pdf".into()),
        mime_type: Some("application/pdf".into()),
        size_bytes: Some(1024),
        width: None,
        height: None,
        duration_secs: None,
    }];
    repo::upsert_chat(&mut conn, &msg.chat, now).await.unwrap();
    let row_id = repo::insert_message(&mut conn, &msg, now)
        .await
        .unwrap()
        .unwrap();
    repo::insert_attachments(&mut conn, &row_id, &msg.attachments)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM attachments WHERE message_id = ?")
        .bind(&row_id)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn access_rules_and_offsets_roundtrip() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();

    assert_eq!(repo::chat_rule(&mut conn, 1).await.unwrap(), None);
    repo::set_chat_rule(&mut conn, 1, true, now).await.unwrap();
    assert_eq!(repo::chat_rule(&mut conn, 1).await.unwrap(), Some(true));
    repo::set_chat_rule(&mut conn, 1, false, now).await.unwrap();
    assert_eq!(repo::chat_rule(&mut conn, 1).await.unwrap(), Some(false));

    assert_eq!(repo::load_offset(&mut conn).await.unwrap(), None);
    repo::save_offset(&mut conn, 900, now).await.unwrap();
    repo::save_offset(&mut conn, 901, now).await.unwrap();
    assert_eq!(repo::load_offset(&mut conn).await.unwrap(), Some(901));
}

#[tokio::test]
async fn list_chats_reports_counts_and_rules() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    let now = Utc::now();
    let msg = message(20, "hi");
    repo::upsert_chat(&mut conn, &msg.chat, now).await.unwrap();
    repo::insert_message(&mut conn, &msg, now).await.unwrap();
    repo::set_chat_rule(&mut conn, msg.chat.id, true, now)
        .await
        .unwrap();

    let chats = repo::list_chats(&mut conn).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id, msg.chat.id);
    assert_eq!(chats[0].message_count, 1);
    assert_eq!(chats[0].rule, Some(true));
}
