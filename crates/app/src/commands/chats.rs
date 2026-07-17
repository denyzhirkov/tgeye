use std::path::Path;

use chrono::Utc;
use tgeye_config::AppConfig;
use tgeye_storage::repo;

use crate::cli::ChatsCommand;

use super::env;

pub async fn run(data_dir: &Path, cmd: ChatsCommand) -> anyhow::Result<()> {
    anyhow::ensure!(
        data_dir.is_dir(),
        "data directory {} does not exist; run `tgeye init` first",
        data_dir.display()
    );
    let config = AppConfig::load(data_dir, env)?;
    let pool = tgeye_storage::connect(&config.database_path(data_dir)).await?;
    let mut conn = pool.acquire().await?;

    match cmd {
        ChatsCommand::List => {
            let chats = repo::list_chats(&mut conn).await?;
            if chats.is_empty() {
                println!(
                    "No chats yet. Add the bot to a group, send a message, and run `tgeye run`."
                );
                return Ok(());
            }
            let require_allowlist = config.security.require_chat_allowlist;
            println!("        CHAT ID  KIND        ACCESS    MESSAGES  TITLE");
            for chat in chats {
                let access = match chat.rule {
                    Some(true) => "allowed",
                    Some(false) => "denied",
                    None if require_allowlist => "blocked",
                    None => "open",
                };
                let title = chat
                    .title
                    .or(chat.username.map(|u| format!("@{u}")))
                    .unwrap_or_default();
                println!(
                    "{:>15}  {:<10}  {:<8}  {:>8}  {}",
                    chat.id, chat.kind, access, chat.message_count, title
                );
            }
        }
        ChatsCommand::Allow { chat_id } => {
            repo::set_chat_rule(&mut conn, chat_id, true, Utc::now()).await?;
            println!("Chat {chat_id} allowed — content will be stored.");
        }
        ChatsCommand::Deny { chat_id } => {
            repo::set_chat_rule(&mut conn, chat_id, false, Utc::now()).await?;
            println!("Chat {chat_id} denied — content will not be stored.");
        }
        ChatsCommand::AllowWrite { chat_id } => {
            repo::set_chat_write_rule(&mut conn, chat_id, true, Utc::now()).await?;
            println!(
                "Chat {chat_id} write-allowed. Also set `allow_write_tools = true` in config to enable sending."
            );
        }
        ChatsCommand::DenyWrite { chat_id } => {
            repo::set_chat_write_rule(&mut conn, chat_id, false, Utc::now()).await?;
            println!("Chat {chat_id} write access revoked.");
        }
    }
    Ok(())
}
