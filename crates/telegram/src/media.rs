use secrecy::{ExposeSecret, SecretString};
use teloxide::Bot;
use teloxide::net::Download;
use teloxide::prelude::Requester;
use teloxide::types::FileId;
use tgeye_domain::media::{MediaError, MediaSource};

/// teloxide-backed adapter: `getFile` then download. The Bot API download URL
/// carries the token, so it is never exposed — bytes go straight into memory.
pub struct TeloxideMedia {
    bot: Bot,
}

impl TeloxideMedia {
    pub fn new(token: &SecretString) -> Self {
        Self {
            bot: Bot::new(token.expose_secret()),
        }
    }
}

#[async_trait::async_trait]
impl MediaSource for TeloxideMedia {
    async fn download(&self, file_id: &str, max_bytes: u64) -> Result<Vec<u8>, MediaError> {
        let file = self
            .bot
            .get_file(FileId(file_id.to_owned()))
            .await
            .map_err(map_err)?;
        let size = u64::from(file.size);
        if size > max_bytes {
            return Err(MediaError::TooLarge {
                size_bytes: size,
                max_bytes,
            });
        }
        let mut buffer = Vec::with_capacity(size as usize);
        self.bot
            .download_file(&file.path, &mut buffer)
            .await
            .map_err(|e| MediaError::Transport(e.to_string()))?;
        Ok(buffer)
    }
}

fn map_err(err: teloxide::RequestError) -> MediaError {
    match err {
        teloxide::RequestError::Api(_) => MediaError::NotFound,
        other => MediaError::Transport(other.to_string()),
    }
}
