use secrecy::{ExposeSecret, SecretString};
use teloxide::Bot;
use teloxide::payloads::GetUpdatesSetters;
use teloxide::prelude::Requester;
use teloxide::types::AllowedUpdate;
use tgeye_domain::CollectedUpdate;

use crate::{TelegramError, map_request_error, mapping};

/// One polled update, fully translated at the adapter boundary — callers never
/// see teloxide types.
pub struct FetchedUpdate {
    pub update_id: i64,
    /// Original Bot API payload for the raw-updates store.
    pub payload_json: String,
    pub collected: CollectedUpdate,
}

/// Long-polling getUpdates client with explicit offset control (idempotent ingestion
/// needs the raw update_id flow, so the teloxide dispatcher is not used).
pub struct UpdatePoller {
    bot: Bot,
    timeout_secs: u32,
}

impl UpdatePoller {
    pub fn new(token: &SecretString, timeout_secs: u32) -> Self {
        Self {
            bot: Bot::new(token.expose_secret()),
            timeout_secs,
        }
    }

    /// Fetch updates after `last_update_id` (None = server default: unconfirmed backlog).
    pub async fn fetch(
        &self,
        last_update_id: Option<i64>,
    ) -> Result<Vec<FetchedUpdate>, TelegramError> {
        let mut request = self
            .bot
            .get_updates()
            .timeout(self.timeout_secs)
            .allowed_updates(vec![
                AllowedUpdate::Message,
                AllowedUpdate::EditedMessage,
                AllowedUpdate::ChannelPost,
                AllowedUpdate::EditedChannelPost,
            ]);
        if let Some(last) = last_update_id {
            request = request.offset((last + 1) as i32);
        }
        let updates = request.await.map_err(map_request_error)?;
        updates
            .iter()
            .map(|update| {
                let payload_json = serde_json::to_string(update).map_err(|e| {
                    TelegramError::Unavailable(format!("payload serialization: {e}"))
                })?;
                Ok(FetchedUpdate {
                    update_id: i64::from(update.id.0),
                    payload_json,
                    collected: mapping::map_update(update),
                })
            })
            .collect()
    }
}
