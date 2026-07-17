use secrecy::{ExposeSecret, SecretString};
use teloxide::Bot;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::Requester;
use teloxide::types::{ChatId, MessageId, ReplyParameters};
use tgeye_domain::media::{WriteError, WriteSink};

/// teloxide-backed sender. Only ever posts as the bot (there is no user session).
pub struct TeloxideWrite {
    bot: Bot,
}

impl TeloxideWrite {
    pub fn new(token: &SecretString) -> Self {
        Self {
            bot: Bot::new(token.expose_secret()),
        }
    }
}

#[async_trait::async_trait]
impl WriteSink for TeloxideWrite {
    async fn send(
        &self,
        chat_id: i64,
        text: &str,
        reply_to_message_id: Option<i64>,
    ) -> Result<i64, WriteError> {
        let mut request = self.bot.send_message(ChatId(chat_id), text.to_owned());
        if let Some(reply_to) = reply_to_message_id {
            request = request.reply_parameters(ReplyParameters::new(MessageId(reply_to as i32)));
        }
        let sent = request.await.map_err(map_err)?;
        Ok(i64::from(sent.id.0))
    }
}

fn map_err(err: teloxide::RequestError) -> WriteError {
    match err {
        teloxide::RequestError::Api(api) => WriteError::Rejected(api.to_string()),
        other => WriteError::Transport(other.to_string()),
    }
}
