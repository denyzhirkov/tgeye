use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
}

impl ChatKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ChatKind::Private => "private",
            ChatKind::Group => "group",
            ChatKind::Supergroup => "supergroup",
            ChatKind::Channel => "channel",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatInfo {
    pub id: i64,
    pub kind: ChatKind,
    pub title: Option<String>,
    pub username: Option<String>,
    pub is_forum: bool,
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub id: i64,
    pub username: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageContentKind {
    Text,
    Photo,
    Document,
    Video,
    Audio,
    Voice,
    VideoNote,
    Sticker,
    Animation,
    Poll,
    Location,
    Venue,
    Contact,
    Dice,
    Game,
    Service,
    Other,
}

impl MessageContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MessageContentKind::Text => "text",
            MessageContentKind::Photo => "photo",
            MessageContentKind::Document => "document",
            MessageContentKind::Video => "video",
            MessageContentKind::Audio => "audio",
            MessageContentKind::Voice => "voice",
            MessageContentKind::VideoNote => "video_note",
            MessageContentKind::Sticker => "sticker",
            MessageContentKind::Animation => "animation",
            MessageContentKind::Poll => "poll",
            MessageContentKind::Location => "location",
            MessageContentKind::Venue => "venue",
            MessageContentKind::Contact => "contact",
            MessageContentKind::Dice => "dice",
            MessageContentKind::Game => "game",
            MessageContentKind::Service => "service",
            MessageContentKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AttachmentMeta {
    pub kind: MessageContentKind,
    pub file_id: String,
    pub file_unique_id: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub chat: ChatInfo,
    pub sender: Option<UserInfo>,
    pub sender_chat_id: Option<i64>,
    pub telegram_message_id: i64,
    pub thread_id: Option<i64>,
    pub reply_to_message_id: Option<i64>,
    pub media_group_id: Option<String>,
    pub kind: MessageContentKind,
    pub text: Option<String>,
    pub sent_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub is_service: bool,
    pub has_protected_content: bool,
    pub attachments: Vec<AttachmentMeta>,
}

#[derive(Debug, Clone)]
pub enum CollectedUpdate {
    NewMessage(IncomingMessage),
    EditedMessage(IncomingMessage),
    /// Stored as raw payload only; `kind` is the Bot API update field name.
    Unsupported {
        kind: &'static str,
    },
}

/// Allowlist policy: an explicit rule always wins; with no rule,
/// `require_allowlist = true` blocks content storage.
pub fn chat_allowed(rule: Option<bool>, require_allowlist: bool) -> bool {
    match rule {
        Some(allowed) => allowed,
        None => !require_allowlist,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_rule_wins_over_mode() {
        assert!(chat_allowed(Some(true), true));
        assert!(chat_allowed(Some(true), false));
        assert!(!chat_allowed(Some(false), true));
        assert!(!chat_allowed(Some(false), false));
    }

    #[test]
    fn no_rule_follows_require_allowlist() {
        assert!(!chat_allowed(None, true));
        assert!(chat_allowed(None, false));
    }
}
