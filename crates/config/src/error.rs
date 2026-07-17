use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("data directory {0} does not exist; run `tgeye init` first")]
    DataDirMissing(PathBuf),

    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid TOML in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid config: {0}")]
    Invalid(String),

    #[error("telegram bot token not found: set {env} or add [telegram].bot_token to {path}", env = crate::secrets::TOKEN_ENV, path = .0.display())]
    TokenMissing(PathBuf),
}
