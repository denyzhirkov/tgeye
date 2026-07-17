//! Port for fetching attachment bytes from the Telegram side. The adapter lives in
//! the `telegram` crate; consumers (the MCP server) depend only on this trait, not
//! on teloxide.

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("attachment exceeds the size limit ({size_bytes} > {max_bytes} bytes)")]
    TooLarge { size_bytes: u64, max_bytes: u64 },

    #[error("attachment file is not available from Telegram")]
    NotFound,

    #[error("telegram transport error: {0}")]
    Transport(String),
}

#[async_trait::async_trait]
pub trait MediaSource: Send + Sync {
    /// Download the file identified by `file_id`, refusing anything larger than
    /// `max_bytes` (checked against Telegram's reported size before transfer).
    async fn download(&self, file_id: &str, max_bytes: u64) -> Result<Vec<u8>, MediaError>;
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("telegram rejected the send: {0}")]
    Rejected(String),

    #[error("telegram transport error: {0}")]
    Transport(String),
}

/// Port for sending messages from the bot. Adapter lives in the `telegram` crate.
#[async_trait::async_trait]
pub trait WriteSink: Send + Sync {
    /// Send `text` to `chat_id`, optionally as a reply. Returns the sent message id.
    async fn send(
        &self,
        chat_id: i64,
        text: &str,
        reply_to_message_id: Option<i64>,
    ) -> Result<i64, WriteError>;
}
