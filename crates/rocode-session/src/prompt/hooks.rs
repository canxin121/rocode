use crate::{MessageRole, SessionMessage};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum HookPayloadEnvelopeWire {
    Body(HookPayloadBodyWire),
    Output { output: HookPayloadBodyWire },
    Data { data: HookPayloadBodyWire },
}

#[derive(Debug, Default, Deserialize)]
struct HookPayloadBodyWire {
    #[serde(default, deserialize_with = "deserialize_opt_messages_lossy")]
    messages: Option<Vec<SessionMessage>>,
    #[serde(default, deserialize_with = "deserialize_opt_message_lossy")]
    message: Option<SessionMessage>,
    #[serde(default, deserialize_with = "deserialize_opt_parts_lossy")]
    parts: Option<Vec<crate::MessagePart>>,
}

fn deserialize_opt_messages_lossy<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<SessionMessage>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(serde_json::from_value::<Vec<SessionMessage>>(value).ok())
}

fn deserialize_opt_message_lossy<'de, D>(
    deserializer: D,
) -> Result<Option<SessionMessage>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(serde_json::from_value::<SessionMessage>(value).ok())
}

fn deserialize_opt_parts_lossy<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<crate::MessagePart>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(serde_json::from_value::<Vec<crate::MessagePart>>(value).ok())
}

fn parse_hook_payload(payload: &serde_json::Value) -> Option<HookPayloadBodyWire> {
    serde_json::from_value::<HookPayloadEnvelopeWire>(payload.clone())
        .ok()
        .map(|envelope| match envelope {
            HookPayloadEnvelopeWire::Body(body) => body,
            HookPayloadEnvelopeWire::Output { output } => output,
            HookPayloadEnvelopeWire::Data { data } => data,
        })
}

pub(crate) fn session_message_hook_payload(message: &SessionMessage) -> serde_json::Value {
    let mut payload = serde_json::to_value(message).unwrap_or_else(|_| serde_json::json!({}));
    let Some(object) = payload.as_object_mut() else {
        return payload;
    };

    object.insert(
        "info".to_string(),
        serde_json::json!({
            "id": message.id,
            "sessionID": message.session_id,
            "role": hook_message_role(&message.role),
            "time": { "created": message.created_at.timestamp_millis() },
        }),
    );

    payload
}

pub(crate) fn hook_message_role(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
        MessageRole::Tool => "tool",
    }
}

pub(crate) fn apply_chat_messages_hook_outputs(
    messages: &mut Vec<SessionMessage>,
    hook_outputs: Vec<rocode_plugin::HookOutput>,
) {
    for output in hook_outputs {
        let Some(payload) = output.payload.as_ref() else {
            continue;
        };
        let Some(body) = parse_hook_payload(payload) else {
            continue;
        };
        let Some(next_messages) = body.messages else {
            continue;
        };
        *messages = next_messages;
    }
}

pub(crate) fn apply_chat_message_hook_outputs(
    message: &mut SessionMessage,
    hook_outputs: Vec<rocode_plugin::HookOutput>,
) {
    for output in hook_outputs {
        let Some(payload) = output.payload.as_ref() else {
            continue;
        };
        let Some(body) = parse_hook_payload(payload) else {
            continue;
        };
        if let Some(next_message) = body.message {
            *message = next_message;
        }
        if let Some(next_parts) = body.parts {
            message.parts = next_parts;
        }
    }
}
