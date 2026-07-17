mod chats;
mod collector;
mod config_cmd;
mod doctor;
mod init;
mod mcp;
mod migrate;
mod token_cmd;

use std::io::{BufRead, Write};

use crate::cli::{Cli, Command};

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let data_dir = tgeye_config::resolve_data_dir(cli.data_dir, env, &std::env::current_dir()?);
    match cli.command {
        Command::Init { token } => init::run(&data_dir, token).await,
        Command::Doctor => doctor::run(&data_dir).await,
        Command::Run => collector::run(&data_dir).await,
        Command::RunMcp => mcp::run(&data_dir).await,
        Command::Chats(cmd) => chats::run(&data_dir, cmd).await,
        Command::Migrate => migrate::run(&data_dir).await,
        Command::Config(cmd) => config_cmd::run(&data_dir, cmd),
        Command::Token(cmd) => token_cmd::run(&data_dir, cmd).await,
    }
}

pub(crate) fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Interactive token prompt; `None` when the user just presses Enter.
pub(crate) fn prompt_token() -> anyhow::Result<Option<String>> {
    eprint!("Enter Telegram bot token (Enter to skip): ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let token = line.trim().to_owned();
    Ok((!token.is_empty()).then_some(token))
}

pub(crate) fn privacy_hint(identity: &tgeye_telegram::BotIdentity) -> &'static str {
    if identity.can_read_all_group_messages {
        "Privacy Mode is OFF — the bot receives all group messages."
    } else {
        "Privacy Mode is ON — the bot only sees commands, replies and mentions in groups. Disable it in BotFather to collect full history."
    }
}
