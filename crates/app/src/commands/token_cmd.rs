use std::path::Path;

use secrecy::SecretString;

use crate::cli::TokenCommand;

use super::{env, privacy_hint, prompt_token};

pub async fn run(data_dir: &Path, cmd: TokenCommand) -> anyhow::Result<()> {
    match cmd {
        TokenCommand::Set { token } => {
            anyhow::ensure!(
                data_dir.is_dir(),
                "data directory {} does not exist; run `tgeye init` first",
                data_dir.display()
            );
            let token = match token {
                Some(t) => t.trim().to_owned(),
                None => prompt_token()?.ok_or_else(|| anyhow::anyhow!("no token provided"))?,
            };
            anyhow::ensure!(!token.is_empty(), "token must not be empty");

            let identity =
                tgeye_telegram::validate_token(&SecretString::from(token.clone())).await?;
            let path = tgeye_config::store_bot_token(data_dir, &token)?;
            println!(
                "Token validated (@{}) and stored in {} (owner-only).",
                identity.username,
                path.display()
            );
            println!("{}", privacy_hint(&identity));
        }
        TokenCommand::Validate => {
            let (token, source) = tgeye_config::load_bot_token(data_dir, env)?;
            let identity = tgeye_telegram::validate_token(&token).await?;
            println!(
                "Token OK ({source}) — bot @{} (id {})",
                identity.username, identity.id
            );
            println!("{}", privacy_hint(&identity));
        }
    }
    Ok(())
}
