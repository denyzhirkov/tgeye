use std::path::Path;
use std::sync::Arc;

use tgeye_config::AppConfig;
use tgeye_mcp_server::{ServerContext, TgeyeServer};
use tgeye_telegram::{TeloxideMedia, TeloxideWrite};

use super::env;

/// stdout is the JSON-RPC channel — no println here, logs go to stderr.
pub async fn run(data_dir: &Path) -> anyhow::Result<()> {
    let config = AppConfig::load(data_dir, env)?;
    let (token, _source) = tgeye_config::load_bot_token(data_dir, env)?;
    let pool = tgeye_storage::connect(&config.database_path(data_dir)).await?;
    let pending = tgeye_storage::pending_migrations(&pool).await?;
    anyhow::ensure!(
        pending == 0,
        "{pending} pending migration(s) — run `tgeye migrate` first"
    );

    let identity = tgeye_telegram::validate_token(&token).await?;
    let timezone: chrono_tz::Tz = config
        .application
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid timezone {}", config.application.timezone))?;

    let ctx = ServerContext {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        bot_id: identity.id,
        bot_username: identity.username.clone(),
        timezone,
        default_page_size: i64::from(config.mcp.default_page_size),
        max_page_size: i64::from(config.mcp.max_page_size),
        require_chat_allowlist: config.security.require_chat_allowlist,
        media_root: config.media_dir(data_dir),
        max_download_bytes: u64::from(config.media.max_download_size_mb) * 1024 * 1024,
        expose_local_path: config.media.expose_local_path,
        allow_media_download: config.security.allow_media_download,
        allow_write_tools: config.security.allow_write_tools,
    };
    let media = Arc::new(TeloxideMedia::new(&token));
    let write = Arc::new(TeloxideWrite::new(&token));
    tracing::info!(bot = identity.username, "MCP stdio server starting");
    tgeye_mcp_server::serve_stdio(TgeyeServer::new(pool, ctx, media, write))
        .await
        .map_err(|e| anyhow::anyhow!("MCP server failed: {e}"))?;
    Ok(())
}
