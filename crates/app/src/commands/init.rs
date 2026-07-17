use std::path::Path;

use tgeye_config::{AppConfig, CONFIG_FILE, ConfigError};

use super::{env, privacy_hint, prompt_token};

pub async fn run(data_dir: &Path, token_arg: Option<String>) -> anyhow::Result<()> {
    if data_dir.exists() {
        println!("Using existing {}", data_dir.display());
    } else {
        std::fs::create_dir_all(data_dir)?;
        println!("Created {}", data_dir.display());
    }

    let config_path = data_dir.join(CONFIG_FILE);
    if !config_path.exists() {
        std::fs::write(&config_path, AppConfig::default_toml())?;
        println!("Created {}", config_path.display());
    }
    let config = AppConfig::load(data_dir, env)?;

    setup_token(data_dir, token_arg)?;

    let db_path = config.database_path(data_dir);
    let pool = tgeye_storage::connect(&db_path).await?;
    tgeye_storage::run_migrations(&pool).await?;
    println!("Database ready at {}", db_path.display());

    match tgeye_config::load_bot_token(data_dir, env) {
        Ok((token, source)) => {
            let me = tgeye_telegram::validate_token(&token).await?;
            println!("Token OK ({source}) — bot @{}", me.username);
            println!("{}", privacy_hint(&me));
        }
        Err(ConfigError::TokenMissing(_)) => {
            println!("No bot token yet — add it later with `tgeye token set`.");
        }
        Err(e) => return Err(e.into()),
    }

    println!("\nNext steps:");
    println!("  1. Add the bot to the Telegram group you want to collect.");
    println!("  2. Disable Privacy Mode in BotFather if it must see all messages.");
    println!("  3. Run `tgeye doctor` to verify the setup.");
    Ok(())
}

fn setup_token(data_dir: &Path, token_arg: Option<String>) -> anyhow::Result<()> {
    let secrets_path = tgeye_config::secrets_path(data_dir);

    if let Some(token) = token_arg {
        let path = tgeye_config::store_bot_token(data_dir, token.trim())?;
        println!("Stored bot token in {} (owner-only)", path.display());
        return Ok(());
    }
    if env(tgeye_config::TOKEN_ENV).is_some_and(|v| !v.trim().is_empty()) {
        println!(
            "Bot token comes from env {} — not writing it to disk.",
            tgeye_config::TOKEN_ENV
        );
        if !secrets_path.exists() {
            tgeye_config::write_secrets_template(data_dir)?;
        }
        return Ok(());
    }
    if secrets_path.exists() && tgeye_config::load_bot_token(data_dir, |_| None).is_ok() {
        println!("Keeping existing token in {}", secrets_path.display());
        return Ok(());
    }
    match prompt_token()? {
        Some(token) => {
            let path = tgeye_config::store_bot_token(data_dir, &token)?;
            println!("Stored bot token in {} (owner-only)", path.display());
        }
        None => {
            if !secrets_path.exists() {
                tgeye_config::write_secrets_template(data_dir)?;
            }
            println!(
                "Skipped token setup — created {} template.",
                secrets_path.display()
            );
        }
    }
    Ok(())
}
