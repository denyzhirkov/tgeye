use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::error::ConfigError;

pub const TOKEN_ENV: &str = "TGEYE_TELEGRAM_BOT_TOKEN";
pub const SECRETS_FILE: &str = "secrets.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Env,
    File,
}

impl std::fmt::Display for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenSource::Env => write!(f, "env {TOKEN_ENV}"),
            TokenSource::File => write!(f, "{SECRETS_FILE}"),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SecretsFile {
    telegram: TelegramSecrets,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TelegramSecrets {
    bot_token: Option<SecretString>,
}

pub fn secrets_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SECRETS_FILE)
}

/// Env var wins over secrets.toml; missing everywhere is a hard error.
pub fn load_bot_token(
    data_dir: &Path,
    env: impl Fn(&str) -> Option<String>,
) -> Result<(SecretString, TokenSource), ConfigError> {
    if let Some(token) = env(TOKEN_ENV)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
    {
        return Ok((SecretString::from(token), TokenSource::Env));
    }

    let path = secrets_path(data_dir);
    if path.exists() {
        let raw = std::fs::read_to_string(&path).map_err(|source| ConfigError::Io {
            path: path.clone(),
            source,
        })?;
        let parsed: SecretsFile = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.clone(),
            source,
        })?;
        if let Some(token) = parsed.telegram.bot_token
            && !token.expose_secret().trim().is_empty()
        {
            return Ok((token, TokenSource::File));
        }
    }

    Err(ConfigError::TokenMissing(path))
}

/// Writes secrets.toml with the token, owner-only permissions (0600 on Unix).
pub fn store_bot_token(data_dir: &Path, token: &str) -> Result<PathBuf, ConfigError> {
    let quoted = toml::Value::String(token.to_owned());
    write_secrets_file(data_dir, &format!("[telegram]\nbot_token = {quoted}\n"))
}

/// Empty commented template for `init` runs without a token; owner-only permissions.
pub fn write_secrets_template(data_dir: &Path) -> Result<PathBuf, ConfigError> {
    write_secrets_file(
        data_dir,
        "[telegram]\n# bot_token = \"123456:...\"   # or set env TGEYE_TELEGRAM_BOT_TOKEN\n",
    )
}

fn write_secrets_file(data_dir: &Path, contents: &str) -> Result<PathBuf, ConfigError> {
    let path = secrets_path(data_dir);
    let io_err = |source| ConfigError::Io {
        path: path.clone(),
        source,
    };

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(&path).map_err(io_err)?;
    file.write_all(contents.as_bytes()).map_err(io_err)?;

    // mode() only applies at creation — clamp perms on pre-existing files too.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(io_err)?;
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn env_wins_over_file() {
        let dir = tempfile::tempdir().unwrap();
        store_bot_token(dir.path(), "111:from-file").unwrap();
        let (token, source) = load_bot_token(dir.path(), |k| {
            (k == TOKEN_ENV).then(|| "222:from-env".to_owned())
        })
        .unwrap();
        assert_eq!(token.expose_secret(), "222:from-env");
        assert_eq!(source, TokenSource::Env);
    }

    #[test]
    fn file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        store_bot_token(dir.path(), "123:abc\"quote").unwrap();
        let (token, source) = load_bot_token(dir.path(), no_env).unwrap();
        assert_eq!(token.expose_secret(), "123:abc\"quote");
        assert_eq!(source, TokenSource::File);
    }

    #[test]
    fn missing_everywhere_is_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            load_bot_token(dir.path(), no_env),
            Err(ConfigError::TokenMissing(_))
        ));
    }

    #[test]
    fn commented_template_is_missing_token() {
        let dir = tempfile::tempdir().unwrap();
        write_secrets_template(dir.path()).unwrap();
        assert!(matches!(
            load_bot_token(dir.path(), no_env),
            Err(ConfigError::TokenMissing(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn secrets_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = store_bot_token(dir.path(), "123:abc").unwrap();
        let mode = std::fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
