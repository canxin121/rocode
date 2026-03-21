use chrono::{DateTime, Utc};

use rocode_session::message_model::{unified_parts_to_session, MessageWithParts, Part};
use rocode_session::MessagePart;

pub(crate) fn try_parse_compatible_parts(
    raw: &str,
    fallback_created_at: DateTime<Utc>,
    message_id: &str,
) -> Option<Vec<MessagePart>> {
    if let Ok(parts) = serde_json::from_str::<Vec<MessagePart>>(raw) {
        return Some(parts);
    }
    if let Ok(parts) = serde_json::from_str::<Vec<Part>>(raw) {
        return Some(unified_parts_to_session(
            parts,
            fallback_created_at,
            message_id,
        ));
    }
    if let Ok(message) = serde_json::from_str::<MessageWithParts>(raw) {
        return Some(unified_parts_to_session(
            message.parts,
            fallback_created_at,
            message_id,
        ));
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(parts_value) = value.get("parts") {
            if let Ok(parts) = serde_json::from_value::<Vec<MessagePart>>(parts_value.clone()) {
                return Some(parts);
            }
            if let Ok(parts) = serde_json::from_value::<Vec<Part>>(parts_value.clone()) {
                return Some(unified_parts_to_session(
                    parts,
                    fallback_created_at,
                    message_id,
                ));
            }
        }
    }

    None
}
