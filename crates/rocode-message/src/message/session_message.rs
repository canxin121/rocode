use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use rocode_types::Role;

use crate::finish::FinishReason;
use crate::id::new_message_id;
use crate::part::{MessagePart, PartKind, PartType, RunningTime, ToolState};
use crate::status::ToolCallStatus;
use crate::usage::MessageUsage;

mod keys {
    pub const MODEL_PROVIDER: &str = "model_provider";
    pub const MODEL_ID: &str = "model_id";
    pub const MODE: &str = "mode";
    pub const FINISH_REASON: &str = "finish_reason";
}

/// Canonical session message model shared across runtime/storage/UI layers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMessage {
    pub id: String,
    #[serde(alias = "sessionId")]
    pub session_id: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<MessagePart>,
    #[serde(alias = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<MessageUsage>,
    /// Provider finish reason (normalized string preferred, but kept as text
    /// for compatibility with existing stored payloads).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
}

/// Convenience alias.
pub type Message = SessionMessage;

impl SessionMessage {
    pub fn new(role: Role, session_id: impl Into<String>) -> Self {
        Self {
            id: new_message_id(),
            session_id: session_id.into(),
            role,
            parts: Vec::new(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
            usage: None,
            finish: None,
        }
    }

    pub fn user(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        let mut message = Self::new(Role::User, session_id);
        message.add_text(text);
        message
    }

    pub fn assistant(session_id: impl Into<String>) -> Self {
        Self::new(Role::Assistant, session_id)
    }

    pub fn system(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        let mut message = Self::new(Role::System, session_id);
        message.add_text(text);
        message
    }

    pub fn tool(session_id: impl Into<String>) -> Self {
        Self::new(Role::Tool, session_id)
    }

    pub fn push_part(&mut self, mut part: MessagePart) {
        if part.message_id.is_none() {
            part.message_id = Some(self.id.clone());
        }
        self.parts.push(part);
    }

    pub fn add_part(&mut self, part_type: PartType) {
        let part = MessagePart::new(part_type).with_message_id(self.id.clone());
        self.parts.push(part);
    }

    pub fn add_text(&mut self, text: impl Into<String>) {
        self.add_part(PartType::Text {
            text: text.into(),
            synthetic: None,
            ignored: None,
        });
    }

    pub fn add_reasoning(&mut self, text: impl Into<String>) {
        self.add_part(PartType::Reasoning { text: text.into() });
    }

    pub fn add_file(
        &mut self,
        url: impl Into<String>,
        filename: impl Into<String>,
        mime: impl Into<String>,
    ) {
        self.add_part(PartType::File {
            url: url.into(),
            filename: filename.into(),
            mime: mime.into(),
        });
    }

    pub fn add_tool_call(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) {
        let now = Utc::now().timestamp_millis();
        self.add_part(PartType::ToolCall {
            id: id.into(),
            name: name.into(),
            input: input.clone(),
            status: ToolCallStatus::Running,
            raw: None,
            state: Some(ToolState::Running {
                input,
                title: None,
                metadata: None,
                time: RunningTime { start: now },
            }),
        });
    }

    pub fn add_tool_result(
        &mut self,
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) {
        self.add_part(PartType::ToolResult {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error,
            title: None,
            metadata: None,
            attachments: None,
        });
    }

    pub fn add_step_start(&mut self, id: impl Into<String>, name: impl Into<String>) {
        self.add_part(PartType::StepStart {
            id: id.into(),
            name: name.into(),
        });
    }

    pub fn add_step_finish(&mut self, id: impl Into<String>, output: Option<String>) {
        self.add_part(PartType::StepFinish {
            id: id.into(),
            output,
        });
    }

    pub fn add_agent(&mut self, name: impl Into<String>) {
        self.add_part(PartType::Agent {
            name: name.into(),
            status: "pending".to_string(),
        });
    }

    pub fn add_subtask(&mut self, id: impl Into<String>, description: impl Into<String>) {
        self.add_part(PartType::Subtask {
            id: id.into(),
            description: description.into(),
            status: "pending".to_string(),
        });
    }

    pub fn add_retry(&mut self, count: u32, reason: impl Into<String>) {
        self.add_part(PartType::Retry {
            count,
            reason: reason.into(),
        });
    }

    pub fn add_compaction(&mut self, summary: impl Into<String>) {
        self.add_part(PartType::Compaction {
            summary: summary.into(),
        });
    }

    pub fn mark_text_parts_synthetic(&mut self) {
        for part in &mut self.parts {
            if let PartType::Text { synthetic, .. } = &mut part.part_type {
                *synthetic = Some(true);
            }
        }
    }

    pub fn text_parts(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().filter_map(|part| part.part_type.text())
    }

    pub fn reasoning_parts(&self) -> impl Iterator<Item = &str> {
        self.parts
            .iter()
            .filter_map(|part| part.part_type.reasoning_text())
    }

    pub fn parts_of_kind(&self, kind: PartKind) -> impl Iterator<Item = &MessagePart> {
        self.parts.iter().filter(move |part| part.kind() == kind)
    }

    pub fn get_text(&self) -> String {
        let total_len: usize = self.text_parts().map(str::len).sum();
        let mut text = String::with_capacity(total_len);
        for segment in self.text_parts() {
            text.push_str(segment);
        }
        text
    }

    pub fn get_reasoning(&self) -> String {
        let total_len: usize = self.reasoning_parts().map(str::len).sum();
        let mut text = String::with_capacity(total_len);
        for segment in self.reasoning_parts() {
            text.push_str(segment);
        }
        text
    }

    /// Append text to the last text part or create a new text part.
    pub fn append_text(&mut self, text: &str) {
        for part in self.parts.iter_mut().rev() {
            if let PartType::Text {
                text: ref mut existing,
                ..
            } = part.part_type
            {
                existing.push_str(text);
                return;
            }
        }
        self.add_text(text.to_string());
    }

    /// Replace all text parts with a single text part.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.parts
            .retain(|part| !matches!(part.part_type, PartType::Text { .. }));
        self.add_text(text);
    }

    pub fn finish_reason(&self) -> Option<FinishReason> {
        if let Some(reason) = self.finish.as_deref() {
            return Some(FinishReason::parse(reason));
        }
        self.metadata
            .get(keys::FINISH_REASON)
            .and_then(serde_json::Value::as_str)
            .map(FinishReason::parse)
    }

    pub fn set_finish_reason(&mut self, reason: FinishReason) {
        self.finish = Some(reason.as_str().to_string());
        self.metadata.insert(
            keys::FINISH_REASON.to_string(),
            serde_json::Value::String(reason.as_str().to_string()),
        );
    }

    pub fn metadata_str(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).and_then(serde_json::Value::as_str)
    }

    pub fn model_provider(&self) -> Option<&str> {
        self.metadata_str(keys::MODEL_PROVIDER)
    }

    pub fn model_id(&self) -> Option<&str> {
        self.metadata_str(keys::MODEL_ID)
    }

    pub fn mode(&self) -> Option<&str> {
        self.metadata_str(keys::MODE)
    }
}

/// Keep messages after the latest compaction boundary.
///
/// If the resulting tail has no user message, keep the latest user message
/// before the boundary as an anchor for prompt-loop invariants.
pub fn filter_compacted_messages(messages: &[SessionMessage]) -> Vec<SessionMessage> {
    let start = messages
        .iter()
        .rposition(|m| {
            m.parts
                .iter()
                .any(|p| matches!(p.part_type, PartType::Compaction { .. }))
        })
        .unwrap_or(0);
    let tail = messages[start..].to_vec();
    if tail.iter().any(|m| matches!(m.role, Role::User)) {
        return tail;
    }

    if let Some(last_user_idx) = messages.iter().rposition(|m| matches!(m.role, Role::User)) {
        if last_user_idx < start {
            let mut anchored = Vec::with_capacity(messages.len() - last_user_idx);
            anchored.push(messages[last_user_idx].clone());
            anchored.extend_from_slice(&messages[start..]);
            return anchored;
        }
    }

    tail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_has_text_part() {
        let message = SessionMessage::user("ses_1", "hello");
        assert_eq!(message.role, Role::User);
        assert_eq!(message.get_text(), "hello");
        assert_eq!(message.parts.len(), 1);
        assert!(message.parts[0].id.parse::<i64>().is_ok());
    }

    #[test]
    fn append_text_merges_last_text_part() {
        let mut message = SessionMessage::assistant("ses_1");
        message.add_text("hello");
        message.append_text(" world");
        assert_eq!(message.get_text(), "hello world");
        assert_eq!(message.parts_of_kind(PartKind::Text).count(), 1);
    }

    #[test]
    fn finish_reason_is_normalized() {
        let mut message = SessionMessage::assistant("ses_1");
        message.finish = Some("toolCalls".to_string());
        assert_eq!(message.finish_reason(), Some(FinishReason::ToolCalls));
    }

    #[test]
    fn filter_compacted_keeps_tail_after_last_compaction() {
        let before = SessionMessage::assistant("ses_1");
        let mut compact = SessionMessage::assistant("ses_1");
        compact.add_compaction("summary");
        let after = SessionMessage::assistant("ses_1");

        let filtered = filter_compacted_messages(&[before, compact, after]);
        assert_eq!(filtered.len(), 2);
        assert!(filtered[0]
            .parts
            .iter()
            .any(|part| matches!(part.part_type, PartType::Compaction { .. })));
    }

    #[test]
    fn filter_compacted_keeps_latest_user_anchor_when_tail_has_no_user() {
        let user = SessionMessage::user("ses_1", "anchor");
        let mut compact = SessionMessage::assistant("ses_1");
        compact.add_compaction("summary");
        let assistant_after = SessionMessage::assistant("ses_1");

        let filtered = filter_compacted_messages(&[user.clone(), compact, assistant_after]);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].id, user.id);
        assert!(matches!(filtered[0].role, Role::User));
    }
}
