pub mod mapping;
pub mod media;
pub mod polling;

use secrecy::{ExposeSecret, SecretString};
use teloxide::prelude::Requester;

pub use mapping::map_update;
pub use media::TeloxideMedia;
pub use polling::{FetchedUpdate, UpdatePoller};

#[derive(Debug, thiserror::Error)]
pub enum TelegramError {
    #[error("Telegram rejected the bot token (getMe failed)")]
    AuthFailed,

    #[error("Telegram rate limit, retry after {0}s")]
    RetryAfter(u64),

    #[error("Telegram API rejected the request: {0}")]
    Api(String),

    #[error("Telegram API unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub struct BotIdentity {
    pub id: u64,
    pub username: String,
    pub can_join_groups: bool,
    /// false = BotFather Privacy Mode is ON: the bot only sees commands,
    /// replies and mentions in groups.
    pub can_read_all_group_messages: bool,
}

/// Live `getMe` call — validates the token and returns the bot identity.
pub async fn validate_token(token: &SecretString) -> Result<BotIdentity, TelegramError> {
    let bot = teloxide::Bot::new(token.expose_secret());
    let me = bot.get_me().await.map_err(|err| match err {
        teloxide::RequestError::Api(_) => TelegramError::AuthFailed,
        other => map_request_error(other),
    })?;
    Ok(BotIdentity {
        id: me.user.id.0,
        username: me.username().to_owned(),
        can_join_groups: me.can_join_groups,
        can_read_all_group_messages: me.can_read_all_group_messages,
    })
}

pub(crate) fn map_request_error(err: teloxide::RequestError) -> TelegramError {
    match err {
        teloxide::RequestError::Api(api) => TelegramError::Api(api.to_string()),
        teloxide::RequestError::RetryAfter(secs) => {
            TelegramError::RetryAfter(u64::from(secs.seconds()))
        }
        other => TelegramError::Unavailable(other.to_string()),
    }
}
