use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Opaque keyset cursor over (sent_at, telegram_message_id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageCursor {
    pub sent_at: String,
    pub telegram_message_id: i64,
}

pub fn encode(cursor: &MessageCursor) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}\n{}",
        cursor.sent_at, cursor.telegram_message_id
    ))
}

pub fn decode(raw: &str) -> Option<MessageCursor> {
    let bytes = URL_SAFE_NO_PAD.decode(raw).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let (sent_at, id) = text.split_once('\n')?;
    if sent_at.is_empty() {
        return None;
    }
    Some(MessageCursor {
        sent_at: sent_at.to_owned(),
        telegram_message_id: id.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let cursor = MessageCursor {
            sent_at: "2026-07-17T17:55:11.000Z".into(),
            telegram_message_id: 42,
        };
        assert_eq!(decode(&encode(&cursor)), Some(cursor));
    }

    #[test]
    fn garbage_is_rejected() {
        assert_eq!(decode("not-base64!!"), None);
        assert_eq!(decode(&URL_SAFE_NO_PAD.encode("no-newline")), None);
        assert_eq!(decode(&URL_SAFE_NO_PAD.encode("ts\nnot-a-number")), None);
        assert_eq!(decode(&URL_SAFE_NO_PAD.encode("\n5")), None);
    }
}
