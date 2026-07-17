use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

pub const CONFIG_FILE: &str = "config.toml";

const LOG_LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

fn resolve_under(data_dir: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        data_dir.join(path)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub application: ApplicationConfig,
    pub telegram: TelegramConfig,
    pub mcp: McpConfig,
    pub media: MediaConfig,
    pub storage: StorageConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaConfig {
    /// Download directory; relative paths resolve against the data dir.
    pub dir: String,
    pub max_download_size_mb: u32,
    /// Return the absolute local file path in download results (desktop use).
    pub expose_local_path: bool,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            dir: "media".into(),
            max_download_size_mb: 50,
            expose_local_path: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpConfig {
    pub default_page_size: u32,
    pub max_page_size: u32,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            default_page_size: 100,
            max_page_size: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TelegramConfig {
    pub poll_timeout_secs: u32,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            poll_timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityConfig {
    pub require_chat_allowlist: bool,
    pub allow_media_download: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_chat_allowlist: true,
            allow_media_download: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApplicationConfig {
    pub log_level: String,
    pub timezone: String,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            timezone: "UTC".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    pub database_path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: "database.sqlite3".into(),
        }
    }
}

impl AppConfig {
    /// Effective config: defaults ← config.toml ← `TGEYE_*` env vars. Validated.
    pub fn load(
        data_dir: &Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        let path = data_dir.join(CONFIG_FILE);
        let mut config = if path.exists() {
            let raw = std::fs::read_to_string(&path).map_err(|source| ConfigError::Io {
                path: path.clone(),
                source,
            })?;
            toml::from_str(&raw).map_err(|source| ConfigError::Parse { path, source })?
        } else {
            Self::default()
        };

        if let Some(v) = env("TGEYE_LOG_LEVEL") {
            config.application.log_level = v;
        }
        if let Some(v) = env("TGEYE_TIMEZONE") {
            config.application.timezone = v;
        }
        if let Some(v) = env("TGEYE_DATABASE_PATH") {
            config.storage.database_path = v;
        }

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if !LOG_LEVELS.contains(&self.application.log_level.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "application.log_level must be one of {LOG_LEVELS:?}, got {:?}",
                self.application.log_level
            )));
        }
        if self.application.timezone.parse::<chrono_tz::Tz>().is_err() {
            return Err(ConfigError::Invalid(format!(
                "application.timezone {:?} is not a valid IANA timezone",
                self.application.timezone
            )));
        }
        if self.storage.database_path.trim().is_empty() {
            return Err(ConfigError::Invalid(
                "storage.database_path must not be empty".into(),
            ));
        }
        if !(1..=60).contains(&self.telegram.poll_timeout_secs) {
            return Err(ConfigError::Invalid(format!(
                "telegram.poll_timeout_secs must be between 1 and 60, got {}",
                self.telegram.poll_timeout_secs
            )));
        }
        if self.mcp.default_page_size == 0 || self.mcp.default_page_size > self.mcp.max_page_size {
            return Err(ConfigError::Invalid(format!(
                "mcp.default_page_size must be within 1..=max_page_size ({}), got {}",
                self.mcp.max_page_size, self.mcp.default_page_size
            )));
        }
        if self.media.max_download_size_mb == 0 {
            return Err(ConfigError::Invalid(
                "media.max_download_size_mb must be at least 1".into(),
            ));
        }
        if self.media.dir.trim().is_empty() {
            return Err(ConfigError::Invalid("media.dir must not be empty".into()));
        }
        Ok(())
    }

    /// Relative `database_path` is resolved against the data dir.
    pub fn database_path(&self, data_dir: &Path) -> PathBuf {
        resolve_under(data_dir, &self.storage.database_path)
    }

    /// Relative `media.dir` is resolved against the data dir.
    pub fn media_dir(&self, data_dir: &Path) -> PathBuf {
        resolve_under(data_dir, &self.media.dir)
    }

    /// Commented template written by `tgeye init`; values match `Default`.
    pub fn default_toml() -> &'static str {
        "\
# tgeye configuration. Any value here is overridden by the matching TGEYE_* env var.

[application]
# trace | debug | info | warn | error        (env: TGEYE_LOG_LEVEL)
log_level = \"info\"
# IANA timezone for day boundaries in queries (env: TGEYE_TIMEZONE)
timezone = \"UTC\"

[telegram]
# getUpdates long-poll timeout, seconds (1-60)
poll_timeout_secs = 30

[mcp]
# Pagination limits for MCP read tools
default_page_size = 100
max_page_size = 500

[media]
# Download directory (relative paths resolve against the data dir)
dir = \"media\"
# Refuse to download attachments larger than this
max_download_size_mb = 50
# Return the absolute local file path in download results (handy for desktop agents)
expose_local_path = true

[storage]
# Relative paths resolve against the data dir  (env: TGEYE_DATABASE_PATH)
database_path = \"database.sqlite3\"

[security]
# true: message content is stored only for chats allowed via `tgeye chats allow <id>`
require_chat_allowlist = true
# false: download tools are unpublished entirely
allow_media_download = true
"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn defaults_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig::load(dir.path(), no_env).unwrap();
        assert_eq!(config.application.log_level, "info");
        assert_eq!(config.application.timezone, "UTC");
    }

    #[test]
    fn default_template_parses_to_default() {
        let from_template: AppConfig = toml::from_str(AppConfig::default_toml()).unwrap();
        let default = AppConfig::default();
        assert_eq!(
            from_template.application.log_level,
            default.application.log_level
        );
        assert_eq!(
            from_template.application.timezone,
            default.application.timezone
        );
        assert_eq!(
            from_template.storage.database_path,
            default.storage.database_path
        );
    }

    #[test]
    fn env_overrides_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "[application]\nlog_level = \"debug\"\ntimezone = \"Asia/Yerevan\"\n",
        )
        .unwrap();
        let config = AppConfig::load(dir.path(), |k| {
            (k == "TGEYE_LOG_LEVEL").then(|| "error".to_owned())
        })
        .unwrap();
        assert_eq!(config.application.log_level, "error");
        assert_eq!(config.application.timezone, "Asia/Yerevan");
    }

    #[test]
    fn invalid_timezone_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let result = AppConfig::load(dir.path(), |k| {
            (k == "TGEYE_TIMEZONE").then(|| "Mars/Olympus".to_owned())
        });
        assert!(matches!(result, Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn invalid_log_level_rejected() {
        let mut config = AppConfig::default();
        config.application.log_level = "verbose".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn unknown_keys_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            "[application]\ntypo_key = 1\n",
        )
        .unwrap();
        assert!(matches!(
            AppConfig::load(dir.path(), no_env),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn relative_db_path_joins_data_dir() {
        let config = AppConfig::default();
        assert_eq!(
            config.database_path(Path::new("/data")),
            PathBuf::from("/data/database.sqlite3")
        );
    }
}
