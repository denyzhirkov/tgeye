pub mod attachments;
pub mod fts;
pub mod media;
pub mod queries;
pub mod repo;

use std::path::Path;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database unavailable: {0}")]
    Unavailable(#[from] sqlx::Error),

    #[error("migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("media storage io error: {0}")]
    Io(std::io::Error),
}

/// WAL-mode SQLite pool; creates the database file if missing.
pub async fn connect(db_path: &Path) -> Result<SqlitePool, StorageError> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), StorageError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Open the pool and bring the schema up to date, returning how many migrations
/// were applied. Lets services self-heal after a binary upgrade instead of
/// failing to start on a stale database.
pub async fn connect_and_migrate(db_path: &Path) -> Result<(SqlitePool, usize), StorageError> {
    let pool = connect(db_path).await?;
    let applied = pending_migrations(&pool).await?;
    run_migrations(&pool).await?;
    Ok((pool, applied))
}

/// Known migrations not yet applied; a missing bookkeeping table counts all as pending.
pub async fn pending_migrations(pool: &SqlitePool) -> Result<usize, StorageError> {
    let total = MIGRATOR.migrations.len();
    let applied: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
    {
        Ok(count) => count,
        Err(sqlx::Error::Database(e)) if e.message().contains("no such table") => 0,
        Err(e) => return Err(e.into()),
    };
    Ok(total.saturating_sub(applied as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_and_migrate_applies_then_reports_zero() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cam.sqlite3");
        let (pool, applied) = connect_and_migrate(&path).await.unwrap();
        assert_eq!(applied, MIGRATOR.migrations.len());
        assert_eq!(pending_migrations(&pool).await.unwrap(), 0);
        drop(pool);

        // Second open on an up-to-date DB applies nothing.
        let (_pool, applied) = connect_and_migrate(&path).await.unwrap();
        assert_eq!(applied, 0);
    }

    #[tokio::test]
    async fn migrations_apply_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let pool = connect(&dir.path().join("test.sqlite3")).await.unwrap();

        assert_eq!(
            pending_migrations(&pool).await.unwrap(),
            MIGRATOR.migrations.len()
        );
        run_migrations(&pool).await.unwrap();
        assert_eq!(pending_migrations(&pool).await.unwrap(), 0);

        // Re-running must be a no-op, not an error.
        run_migrations(&pool).await.unwrap();

        // Baseline tables exist and the unique message index holds.
        sqlx::query("INSERT INTO chats (id, kind, first_seen_at, last_seen_at) VALUES (1, 'group', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&pool)
            .await
            .unwrap();
        let insert_msg = "INSERT INTO messages (id, chat_id, telegram_message_id, kind, sent_at, received_at) VALUES (?, 1, 10, 'text', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')";
        sqlx::query(insert_msg)
            .bind("uuid-1")
            .execute(&pool)
            .await
            .unwrap();
        let duplicate = sqlx::query(insert_msg).bind("uuid-2").execute(&pool).await;
        assert!(
            duplicate.is_err(),
            "duplicate (chat_id, telegram_message_id) must be rejected"
        );
    }
}
