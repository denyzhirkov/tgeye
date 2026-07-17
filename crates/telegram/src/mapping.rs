use teloxide::types::{
    Chat, ChatKind as TgChatKind, MediaKind, Message, MessageKind, PublicChatKind, Update,
    UpdateKind, User,
};
use tgeye_domain::{
    AttachmentMeta, ChatInfo, ChatKind, CollectedUpdate, IncomingMessage, MessageContentKind,
    UserInfo,
};

pub fn map_update(update: &Update) -> CollectedUpdate {
    match &update.kind {
        UpdateKind::Message(m) | UpdateKind::ChannelPost(m) => {
            CollectedUpdate::NewMessage(map_message(m))
        }
        UpdateKind::EditedMessage(m) | UpdateKind::EditedChannelPost(m) => {
            CollectedUpdate::EditedMessage(map_message(m))
        }
        _ => CollectedUpdate::Unsupported {
            kind: update_kind_name(&update.kind),
        },
    }
}

fn update_kind_name(kind: &UpdateKind) -> &'static str {
    match kind {
        UpdateKind::MessageReaction(_) => "message_reaction",
        UpdateKind::MessageReactionCount(_) => "message_reaction_count",
        UpdateKind::MyChatMember(_) => "my_chat_member",
        UpdateKind::ChatMember(_) => "chat_member",
        UpdateKind::ChatJoinRequest(_) => "chat_join_request",
        UpdateKind::Poll(_) => "poll",
        UpdateKind::PollAnswer(_) => "poll_answer",
        _ => "other",
    }
}

pub fn map_message(msg: &Message) -> IncomingMessage {
    let (kind, attachments, is_service) = classify(msg);
    IncomingMessage {
        chat: map_chat(&msg.chat),
        sender: msg.from.as_ref().map(map_user),
        sender_chat_id: msg.sender_chat.as_ref().map(|c| c.id.0),
        telegram_message_id: i64::from(msg.id.0),
        thread_id: msg.thread_id.map(|t| i64::from(t.0.0)),
        reply_to_message_id: msg.reply_to_message().map(|r| i64::from(r.id.0)),
        media_group_id: msg.media_group_id().map(|m| m.0.clone()),
        kind,
        text: msg.text().or_else(|| msg.caption()).map(ToOwned::to_owned),
        sent_at: msg.date,
        edited_at: msg.edit_date().copied(),
        is_service,
        has_protected_content: msg.has_protected_content(),
        attachments,
    }
}

fn map_chat(chat: &Chat) -> ChatInfo {
    let (kind, is_forum) = match &chat.kind {
        TgChatKind::Private(_) => (ChatKind::Private, false),
        TgChatKind::Public(public) => match &public.kind {
            PublicChatKind::Group => (ChatKind::Group, false),
            PublicChatKind::Supergroup(s) => (ChatKind::Supergroup, s.is_forum),
            PublicChatKind::Channel(_) => (ChatKind::Channel, false),
        },
    };
    ChatInfo {
        id: chat.id.0,
        kind,
        title: chat.title().map(ToOwned::to_owned),
        username: chat.username().map(ToOwned::to_owned),
        is_forum,
    }
}

fn map_user(user: &User) -> UserInfo {
    UserInfo {
        id: user.id.0 as i64,
        username: user.username.clone(),
        first_name: user.first_name.clone(),
        last_name: user.last_name.clone(),
        is_bot: user.is_bot,
    }
}

fn classify(msg: &Message) -> (MessageContentKind, Vec<AttachmentMeta>, bool) {
    let MessageKind::Common(common) = &msg.kind else {
        return (MessageContentKind::Service, vec![], true);
    };
    let (kind, attachments) = match &common.media_kind {
        MediaKind::Text(_) => (MessageContentKind::Text, vec![]),
        MediaKind::Photo(m) => {
            // Telegram sends sizes ascending; the last one is the original resolution.
            let attachments = m
                .photo
                .last()
                .map(|p| {
                    vec![AttachmentMeta {
                        kind: MessageContentKind::Photo,
                        file_id: p.file.id.to_string(),
                        file_unique_id: p.file.unique_id.to_string(),
                        file_name: None,
                        mime_type: None,
                        size_bytes: Some(i64::from(p.file.size)),
                        width: Some(i64::from(p.width)),
                        height: Some(i64::from(p.height)),
                        duration_secs: None,
                    }]
                })
                .unwrap_or_default();
            (MessageContentKind::Photo, attachments)
        }
        MediaKind::Document(m) => (
            MessageContentKind::Document,
            vec![AttachmentMeta {
                kind: MessageContentKind::Document,
                file_id: m.document.file.id.to_string(),
                file_unique_id: m.document.file.unique_id.to_string(),
                file_name: m.document.file_name.clone(),
                mime_type: m.document.mime_type.as_ref().map(ToString::to_string),
                size_bytes: Some(i64::from(m.document.file.size)),
                width: None,
                height: None,
                duration_secs: None,
            }],
        ),
        MediaKind::Video(m) => (
            MessageContentKind::Video,
            vec![AttachmentMeta {
                kind: MessageContentKind::Video,
                file_id: m.video.file.id.to_string(),
                file_unique_id: m.video.file.unique_id.to_string(),
                file_name: m.video.file_name.clone(),
                mime_type: m.video.mime_type.as_ref().map(ToString::to_string),
                size_bytes: Some(i64::from(m.video.file.size)),
                width: Some(i64::from(m.video.width)),
                height: Some(i64::from(m.video.height)),
                duration_secs: Some(i64::from(m.video.duration.seconds())),
            }],
        ),
        MediaKind::Audio(m) => (
            MessageContentKind::Audio,
            vec![AttachmentMeta {
                kind: MessageContentKind::Audio,
                file_id: m.audio.file.id.to_string(),
                file_unique_id: m.audio.file.unique_id.to_string(),
                file_name: m.audio.file_name.clone(),
                mime_type: m.audio.mime_type.as_ref().map(ToString::to_string),
                size_bytes: Some(i64::from(m.audio.file.size)),
                width: None,
                height: None,
                duration_secs: Some(i64::from(m.audio.duration.seconds())),
            }],
        ),
        MediaKind::Voice(m) => (
            MessageContentKind::Voice,
            vec![AttachmentMeta {
                kind: MessageContentKind::Voice,
                file_id: m.voice.file.id.to_string(),
                file_unique_id: m.voice.file.unique_id.to_string(),
                file_name: None,
                mime_type: m.voice.mime_type.as_ref().map(ToString::to_string),
                size_bytes: Some(i64::from(m.voice.file.size)),
                width: None,
                height: None,
                duration_secs: Some(i64::from(m.voice.duration.seconds())),
            }],
        ),
        MediaKind::VideoNote(m) => (
            MessageContentKind::VideoNote,
            vec![AttachmentMeta {
                kind: MessageContentKind::VideoNote,
                file_id: m.video_note.file.id.to_string(),
                file_unique_id: m.video_note.file.unique_id.to_string(),
                file_name: None,
                mime_type: None,
                size_bytes: Some(i64::from(m.video_note.file.size)),
                width: None,
                height: None,
                duration_secs: Some(i64::from(m.video_note.duration.seconds())),
            }],
        ),
        MediaKind::Sticker(m) => (
            MessageContentKind::Sticker,
            vec![AttachmentMeta {
                kind: MessageContentKind::Sticker,
                file_id: m.sticker.file.id.to_string(),
                file_unique_id: m.sticker.file.unique_id.to_string(),
                file_name: None,
                mime_type: None,
                size_bytes: Some(i64::from(m.sticker.file.size)),
                width: Some(i64::from(m.sticker.width)),
                height: Some(i64::from(m.sticker.height)),
                duration_secs: None,
            }],
        ),
        MediaKind::Animation(m) => (
            MessageContentKind::Animation,
            vec![AttachmentMeta {
                kind: MessageContentKind::Animation,
                file_id: m.animation.file.id.to_string(),
                file_unique_id: m.animation.file.unique_id.to_string(),
                file_name: m.animation.file_name.clone(),
                mime_type: m.animation.mime_type.as_ref().map(ToString::to_string),
                size_bytes: Some(i64::from(m.animation.file.size)),
                width: Some(i64::from(m.animation.width)),
                height: Some(i64::from(m.animation.height)),
                duration_secs: Some(i64::from(m.animation.duration.seconds())),
            }],
        ),
        MediaKind::Poll(_) => (MessageContentKind::Poll, vec![]),
        MediaKind::Location(_) => (MessageContentKind::Location, vec![]),
        MediaKind::Venue(_) => (MessageContentKind::Venue, vec![]),
        MediaKind::Contact(_) => (MessageContentKind::Contact, vec![]),
        MediaKind::Game(_) => (MessageContentKind::Game, vec![]),
        _ => (MessageContentKind::Other, vec![]),
    };
    (kind, attachments, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_update(json: &str) -> Update {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn maps_text_message_in_forum_supergroup() {
        let update = parse_update(
            r#"{
                "update_id": 700001,
                "message": {
                    "message_id": 100,
                    "message_thread_id": 5,
                    "date": 1752750000,
                    "chat": {"id": -1001234567890, "type": "supergroup", "title": "Team", "is_forum": true},
                    "from": {"id": 42, "is_bot": false, "first_name": "Denis", "username": "dz"},
                    "reply_to_message": {
                        "message_id": 90,
                        "date": 1752740000,
                        "chat": {"id": -1001234567890, "type": "supergroup", "title": "Team"},
                        "text": "earlier"
                    },
                    "text": "hello world"
                }
            }"#,
        );
        let CollectedUpdate::NewMessage(msg) = map_update(&update) else {
            panic!("expected NewMessage");
        };
        assert_eq!(msg.chat.id, -1001234567890);
        assert_eq!(msg.chat.kind, ChatKind::Supergroup);
        assert!(msg.chat.is_forum);
        assert_eq!(msg.telegram_message_id, 100);
        assert_eq!(msg.thread_id, Some(5));
        assert_eq!(msg.reply_to_message_id, Some(90));
        assert_eq!(msg.text.as_deref(), Some("hello world"));
        assert_eq!(msg.kind, MessageContentKind::Text);
        let sender = msg.sender.unwrap();
        assert_eq!(sender.id, 42);
        assert_eq!(sender.username.as_deref(), Some("dz"));
        assert!(msg.attachments.is_empty());
        assert!(!msg.is_service);
    }

    #[test]
    fn maps_edited_message() {
        let update = parse_update(
            r#"{
                "update_id": 700002,
                "edited_message": {
                    "message_id": 100,
                    "date": 1752750000,
                    "edit_date": 1752750100,
                    "chat": {"id": 42, "type": "private", "first_name": "Denis"},
                    "from": {"id": 42, "is_bot": false, "first_name": "Denis"},
                    "text": "fixed"
                }
            }"#,
        );
        let CollectedUpdate::EditedMessage(msg) = map_update(&update) else {
            panic!("expected EditedMessage");
        };
        assert_eq!(msg.chat.kind, ChatKind::Private);
        assert_eq!(msg.text.as_deref(), Some("fixed"));
        assert!(msg.edited_at.is_some());
    }

    #[test]
    fn maps_document_with_caption() {
        let update = parse_update(
            r#"{
                "update_id": 700003,
                "message": {
                    "message_id": 101,
                    "date": 1752750000,
                    "chat": {"id": -100987, "type": "group", "title": "Files"},
                    "from": {"id": 42, "is_bot": false, "first_name": "Denis"},
                    "document": {
                        "file_id": "doc-file-id",
                        "file_unique_id": "doc-uniq",
                        "file_name": "report.pdf",
                        "mime_type": "application/pdf",
                        "file_size": 2048
                    },
                    "caption": "the report"
                }
            }"#,
        );
        let CollectedUpdate::NewMessage(msg) = map_update(&update) else {
            panic!("expected NewMessage");
        };
        assert_eq!(msg.kind, MessageContentKind::Document);
        assert_eq!(msg.text.as_deref(), Some("the report"));
        assert_eq!(msg.attachments.len(), 1);
        let attachment = &msg.attachments[0];
        assert_eq!(attachment.file_id, "doc-file-id");
        assert_eq!(attachment.file_name.as_deref(), Some("report.pdf"));
        assert_eq!(attachment.mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(attachment.size_bytes, Some(2048));
    }

    #[test]
    fn unsupported_update_is_labeled() {
        let update = parse_update(
            r#"{
                "update_id": 700004,
                "poll": {
                    "id": "poll-1",
                    "question": "ok?",
                    "options": [{"text": "yes", "voter_count": 1}],
                    "total_voter_count": 1,
                    "is_closed": false,
                    "is_anonymous": true,
                    "type": "regular",
                    "allows_multiple_answers": false
                }
            }"#,
        );
        let CollectedUpdate::Unsupported { kind } = map_update(&update) else {
            panic!("expected Unsupported");
        };
        assert_eq!(kind, "poll");
    }
}
