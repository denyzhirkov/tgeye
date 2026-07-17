use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use tgeye_config::AppConfig;
use tgeye_domain::{CollectedUpdate, IncomingMessage, chat_allowed};
use tgeye_storage::repo;
use tgeye_telegram::{FetchedUpdate, TelegramError, UpdatePoller};

use super::{env, privacy_hint};

pub async fn run(data_dir: &Path) -> anyhow::Result<()> {
    let config = AppConfig::load(data_dir, env)?;
    let (token, source) = tgeye_config::load_bot_token(data_dir, env)?;
    let pool = tgeye_storage::connect(&config.database_path(data_dir)).await?;
    let pending = tgeye_storage::pending_migrations(&pool).await?;
    anyhow::ensure!(
        pending == 0,
        "{pending} pending migration(s) — run `tgeye migrate` first"
    );

    let identity = tgeye_telegram::validate_token(&token).await?;
    println!(
        "Collecting as @{} (token from {source}). Ctrl-C to stop.",
        identity.username
    );
    println!("{}", privacy_hint(&identity));

    let poller = UpdatePoller::new(&token, config.telegram.poll_timeout_secs);
    let require_allowlist = config.security.require_chat_allowlist;
    let mut last_update_id = {
        let mut conn = pool.acquire().await?;
        repo::load_offset(&mut conn).await?
    };

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    let mut backoff_secs = 1u64;

    loop {
        let batch = tokio::select! {
            _ = &mut ctrl_c => break,
            result = poller.fetch(last_update_id) => result,
        };
        match batch {
            Ok(updates) => {
                backoff_secs = 1;
                for update in updates {
                    if let Err(error) = process_update(&pool, &update, require_allowlist).await {
                        // Poison updates must not stop ingestion (spec §7.2); the offset
                        // tx rolled back, so the update is refetched on next start.
                        tracing::error!(update.update_id, %error, "failed to process update");
                    }
                    last_update_id = Some(update.update_id);
                }
            }
            Err(TelegramError::RetryAfter(secs)) => {
                tracing::warn!(secs, "telegram rate limit on getUpdates");
                if wait_or_shutdown(&mut ctrl_c, secs).await {
                    break;
                }
            }
            Err(error) => {
                tracing::warn!(%error, backoff_secs, "getUpdates failed; backing off");
                if wait_or_shutdown(&mut ctrl_c, backoff_secs).await {
                    break;
                }
                backoff_secs = (backoff_secs * 2).min(60);
            }
        }
    }

    println!("\nCollector stopped; offset saved.");
    Ok(())
}

/// true = Ctrl-C arrived during the wait.
async fn wait_or_shutdown(
    ctrl_c: &mut std::pin::Pin<&mut impl Future<Output = std::io::Result<()>>>,
    secs: u64,
) -> bool {
    tokio::select! {
        _ = ctrl_c.as_mut() => true,
        _ = tokio::time::sleep(Duration::from_secs(secs)) => false,
    }
}

async fn process_update(
    pool: &SqlitePool,
    update: &FetchedUpdate,
    require_allowlist: bool,
) -> anyhow::Result<()> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;
    let fresh =
        repo::record_raw_update(&mut tx, update.update_id, &update.payload_json, now).await?;
    if fresh {
        match &update.collected {
            CollectedUpdate::NewMessage(msg) => {
                persist(&mut tx, msg, require_allowlist, false).await?;
            }
            CollectedUpdate::EditedMessage(msg) => {
                persist(&mut tx, msg, require_allowlist, true).await?;
            }
            CollectedUpdate::Unsupported { kind } => {
                tracing::debug!(
                    update.update_id,
                    kind,
                    "unsupported update stored as raw payload"
                );
            }
        }
    }
    repo::save_offset(&mut tx, update.update_id, now).await?;
    tx.commit().await?;
    Ok(())
}

async fn persist(
    conn: &mut sqlx::SqliteConnection,
    msg: &IncomingMessage,
    require_allowlist: bool,
    is_edit: bool,
) -> anyhow::Result<()> {
    let now = Utc::now();
    // Chat metadata is always recorded so `chats list` can show what to allow;
    // content stays out until the chat passes the allowlist (spec §15.3).
    repo::upsert_chat(conn, &msg.chat, now).await?;
    let rule = repo::chat_rule(conn, msg.chat.id).await?;
    if !chat_allowed(rule, require_allowlist) {
        tracing::debug!(chat_id = msg.chat.id, "chat not allowed; content skipped");
        return Ok(());
    }

    if let Some(sender) = &msg.sender {
        repo::upsert_user(conn, sender, now).await?;
    }
    if is_edit {
        repo::apply_edit(conn, msg, now).await?;
    } else if let Some(row_id) = repo::insert_message(conn, msg, now).await?
        && !msg.attachments.is_empty()
    {
        repo::insert_attachments(conn, &row_id, &msg.attachments).await?;
    }
    tracing::info!(
        chat_id = msg.chat.id,
        telegram_message_id = msg.telegram_message_id,
        kind = msg.kind.as_str(),
        is_edit,
        "stored"
    );
    Ok(())
}
