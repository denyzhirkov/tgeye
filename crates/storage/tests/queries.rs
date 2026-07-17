#![allow(clippy::unwrap_used)] // integration tests; clippy.toml only covers #[cfg(test)] modules

use chrono::{DateTime, Duration, TimeZone, Utc};
use tgeye_domain::{ChatInfo, ChatKind, IncomingMessage, MessageContentKind, UserInfo};
use tgeye_storage::queries::{self, SearchQuery};
use tgeye_storage::{fts, repo};

fn chat() -> ChatInfo {
    ChatInfo {
        id: -100200300,
        kind: ChatKind::Supergroup,
        title: Some("Team".into()),
        username: None,
        is_forum: false,
    }
}

fn base_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 17, 12, 0, 0).unwrap()
}

fn message(id: i64, reply_to: Option<i64>, minute: i64, text: &str) -> IncomingMessage {
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
        telegram_message_id: id,
        thread_id: None,
        reply_to_message_id: reply_to,
        media_group_id: None,
        kind: MessageContentKind::Text,
        text: Some(text.into()),
        sent_at: base_time() + Duration::minutes(minute),
        edited_at: None,
        is_service: false,
        has_protected_content: false,
        attachments: vec![],
    }
}

async fn setup() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("queries-test.sqlite3");
    let pool = tgeye_storage::connect(&path).await.unwrap();
    tgeye_storage::run_migrations(&pool).await.unwrap();
    pool
}

/// Thread: 1 ← 2 ← 3 (each replies to the previous), plus 4 and 5 standalone.
async fn seed_thread(conn: &mut sqlx::SqliteConnection) {
    let now = base_time();
    repo::upsert_chat(conn, &chat(), now).await.unwrap();
    for (id, reply_to, minute, text) in [
        (1, None, 0, "корневое сообщение про баг init"),
        (2, Some(1), 1, "ответ первый"),
        (3, Some(2), 2, "ответ второй с деталями"),
        (4, None, 3, "отдельное сообщение про идею"),
        (5, None, 4, "ещё одно про баг поиска"),
    ] {
        let msg = message(id, reply_to, minute, text);
        repo::insert_message(conn, &msg, now).await.unwrap();
    }
}

#[tokio::test]
async fn get_message_hits_and_misses() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;

    let found = queries::get_message(&mut conn, -100200300, 3)
        .await
        .unwrap();
    assert_eq!(found.unwrap().reply_to_message_id, Some(2));
    assert!(
        queries::get_message(&mut conn, -100200300, 999)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn context_side_windows_around_anchor() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;
    let anchor = queries::get_message(&mut conn, -100200300, 3)
        .await
        .unwrap()
        .unwrap();

    let before = queries::context_side(
        &mut conn,
        -100200300,
        &anchor.sent_at,
        anchor.telegram_message_id,
        false,
        false,
        10,
    )
    .await
    .unwrap();
    let after = queries::context_side(
        &mut conn,
        -100200300,
        &anchor.sent_at,
        anchor.telegram_message_id,
        true,
        false,
        10,
    )
    .await
    .unwrap();

    assert_eq!(
        before
            .iter()
            .map(|m| m.telegram_message_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        after
            .iter()
            .map(|m| m.telegram_message_id)
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
}

#[tokio::test]
async fn reply_ancestors_walks_up_the_chain() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;
    let target = queries::get_message(&mut conn, -100200300, 3)
        .await
        .unwrap()
        .unwrap();

    let ancestors = queries::reply_ancestors(&mut conn, -100200300, target.reply_to_message_id, 20)
        .await
        .unwrap();
    assert_eq!(
        ancestors
            .iter()
            .map(|m| m.telegram_message_id)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
}

#[tokio::test]
async fn reply_ancestors_respects_depth_cap() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;
    let target = queries::get_message(&mut conn, -100200300, 3)
        .await
        .unwrap()
        .unwrap();

    let ancestors = queries::reply_ancestors(&mut conn, -100200300, target.reply_to_message_id, 1)
        .await
        .unwrap();
    assert_eq!(ancestors.len(), 1);
    assert_eq!(ancestors[0].telegram_message_id, 2);
}

#[tokio::test]
async fn fts_search_finds_russian_terms_and_backfills() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;

    let expr = fts::to_match_expr("баг").unwrap();
    let hits = queries::search_messages(
        &mut conn,
        &SearchQuery {
            match_expr: expr,
            chat_ids: vec![-100200300],
            limit: 50,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let ids: Vec<i64> = hits.iter().map(|h| h.message.telegram_message_id).collect();
    assert!(
        ids.contains(&1) && ids.contains(&5),
        "both 'баг' messages found, got {ids:?}"
    );
    assert!(!ids.contains(&4), "idea message must not match");
    assert!(hits.iter().all(|h| !h.snippet.is_empty()));
}

#[tokio::test]
async fn fts_reflects_edits_via_trigger() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;

    // Edit message 4 to contain a new searchable term.
    let mut edited = message(4, None, 3, "переписал в задачу деплой");
    edited.edited_at = Some(base_time() + Duration::minutes(10));
    repo::apply_edit(&mut conn, &edited, Utc::now())
        .await
        .unwrap();

    let expr = fts::to_match_expr("деплой").unwrap();
    let hits = queries::search_messages(
        &mut conn,
        &SearchQuery {
            match_expr: expr,
            chat_ids: vec![-100200300],
            limit: 50,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].message.telegram_message_id, 4);
}

#[tokio::test]
async fn chat_stats_counts_messages() {
    let pool = setup().await;
    let mut conn = pool.acquire().await.unwrap();
    seed_thread(&mut conn).await;

    let stats = queries::chat_stats(&mut conn, -100200300).await.unwrap();
    assert_eq!(stats.message_count, 5);
    assert_eq!(stats.edited_count, 0);
    assert!(stats.first_message_at.is_some() && stats.last_message_at.is_some());
}
