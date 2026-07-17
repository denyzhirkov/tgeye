use std::path::Path;

use tgeye_config::AppConfig;

use crate::cli::ConfigCommand;

use super::env;

pub fn run(data_dir: &Path, cmd: ConfigCommand) -> anyhow::Result<()> {
    match cmd {
        ConfigCommand::Show => {
            let config = AppConfig::load(data_dir, env)?;
            println!("# data dir: {}", data_dir.display());
            println!("# token: {}", token_status(data_dir));
            print!("{}", toml::to_string_pretty(&config)?);
        }
        ConfigCommand::Validate => {
            AppConfig::load(data_dir, env)?;
            println!("Config is valid.");
        }
    }
    Ok(())
}

fn token_status(data_dir: &Path) -> String {
    match tgeye_config::load_bot_token(data_dir, env) {
        Ok((_, source)) => format!("present ({source})"),
        Err(_) => "not configured".into(),
    }
}
