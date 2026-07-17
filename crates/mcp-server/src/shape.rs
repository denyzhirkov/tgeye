use serde::Serialize;
use tgeye_storage::queries::{AttachmentRow, AuthorRow, ChatRow, MessageRow};

#[derive(Debug, Serialize)]
pub struct Page {
    pub limit: i64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub source: &'static str,
    pub timezone: String,
    pub generated_at: String,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct ChatShape {
    pub id: String,
    pub kind: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub is_forum: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorShape {
    pub id: String,
    pub display_name: String,
    pub username: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Serialize)]
pub struct MessageShape {
    pub id: i64,
    pub thread_id: Option<i64>,
    pub kind: String,
    pub text: Option<String>,
    pub sent_at: String,
    pub edited_at: Option<String>,
    pub reply_to_message_id: Option<i64>,
    pub media_group_id: Option<String>,
    pub telegram_url: Option<String>,
    pub resource_uri: String,
}

#[derive(Debug, Serialize)]
pub struct AttachmentShape {
    pub id: String,
    pub kind: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_secs: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MessageItem {
    pub chat: ChatShape,
    pub message: MessageShape,
    pub author: Option<AuthorShape>,
    pub attachments: Vec<AttachmentShape>,
}

pub fn chat_shape(chat: &ChatRow) -> ChatShape {
    ChatShape {
        id: chat.id.to_string(),
        kind: chat.kind.clone(),
        title: chat.title.clone(),
        username: chat.username.clone(),
        is_forum: chat.is_forum,
    }
}

pub fn author_shape(author: &AuthorRow) -> AuthorShape {
    let display_name = match (&author.first_name, &author.last_name) {
        (Some(first), Some(last)) => format!("{first} {last}"),
        (Some(first), None) => first.clone(),
        (None, _) => author
            .username
            .clone()
            .unwrap_or_else(|| author.id.to_string()),
    };
    AuthorShape {
        id: author.id.to_string(),
        display_name,
        username: author.username.clone(),
        is_bot: author.is_bot,
    }
}

pub fn attachment_shape(row: &AttachmentRow) -> AttachmentShape {
    AttachmentShape {
        id: row.id.clone(),
        kind: row.kind.clone(),
        file_name: row.file_name.clone(),
        mime_type: row.mime_type.clone(),
        size_bytes: row.size_bytes,
        width: row.width,
        height: row.height,
        duration_secs: row.duration_secs,
    }
}

pub fn message_shape(msg: &MessageRow, chat: &ChatRow) -> MessageShape {
    // Only public username chats get a t.me link; never fabricate one (spec §8.3).
    let telegram_url = chat
        .username
        .as_ref()
        .map(|username| format!("https://t.me/{username}/{}", msg.telegram_message_id));
    MessageShape {
        id: msg.telegram_message_id,
        thread_id: msg.thread_id,
        kind: msg.kind.clone(),
        text: msg.text.clone(),
        sent_at: msg.sent_at.clone(),
        edited_at: msg.edited_at.clone(),
        reply_to_message_id: msg.reply_to_message_id,
        media_group_id: msg.media_group_id.clone(),
        telegram_url,
        resource_uri: format!(
            "telegram://chat/{}/message/{}",
            msg.chat_id, msg.telegram_message_id
        ),
    }
}
