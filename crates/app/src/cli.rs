use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tgeye", version, about = "Local Telegram bot → MCP server")]
pub struct Cli {
    /// Data directory (default: ./.tgeye, env TGEYE_HOME)
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create the data dir with config, secrets and an initialized database
    Init {
        /// Telegram bot token; falls back to TGEYE_TELEGRAM_BOT_TOKEN or an interactive prompt
        #[arg(long)]
        token: Option<String>,
    },
    /// Check data dir, config, token, Telegram API and database health
    Doctor,
    /// Run the Telegram collector (long polling; Ctrl-C to stop)
    Run,
    /// Known chats and their access rules
    #[command(subcommand)]
    Chats(ChatsCommand),
    /// Apply pending database migrations
    Migrate,
    /// Configuration inspection
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Bot token management
    #[command(subcommand)]
    Token(TokenCommand),
}

#[derive(Subcommand)]
pub enum ChatsCommand {
    /// List chats the bot has seen, with message counts and access status
    List,
    /// Allow storing message content for a chat
    Allow {
        /// Telegram chat id (e.g. -1001234567890)
        chat_id: i64,
    },
    /// Deny storing message content for a chat
    Deny {
        /// Telegram chat id (e.g. -1001234567890)
        chat_id: i64,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print the effective configuration (defaults + config.toml + env)
    Show,
    /// Validate config.toml
    Validate,
}

#[derive(Subcommand)]
pub enum TokenCommand {
    /// Store the bot token in secrets.toml (owner-only permissions)
    Set {
        /// Token value; prompted on stdin when omitted
        token: Option<String>,
    },
    /// Check the stored token against Telegram getMe
    Validate,
}
