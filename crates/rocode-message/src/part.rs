use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::{AsRefStr, Display, EnumString};

use crate::id::new_part_id;
use crate::status::ToolCallStatus;

fn default_json_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn default_pending_status() -> String {
    "pending".to_string()
}

/// Canonical part kind string used for indexing/filtering.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Display, AsRefStr, EnumString,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum PartKind {
    #[serde(rename = "text")]
    Text,
    #[strum(serialize = "tool_call", serialize = "toolCall")]
    #[serde(rename = "tool_call", alias = "toolCall")]
    ToolCall,
    #[strum(serialize = "tool_result", serialize = "toolResult")]
    #[serde(rename = "tool_result", alias = "toolResult")]
    ToolResult,
    #[serde(rename = "reasoning")]
    Reasoning,
    #[serde(rename = "file")]
    File,
    #[strum(serialize = "step_start", serialize = "stepStart")]
    #[serde(rename = "step_start", alias = "stepStart")]
    StepStart,
    #[strum(serialize = "step_finish", serialize = "stepFinish")]
    #[serde(rename = "step_finish", alias = "stepFinish")]
    StepFinish,
    #[serde(rename = "snapshot")]
    Snapshot,
    #[serde(rename = "patch")]
    Patch,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "subtask")]
    Subtask,
    #[serde(rename = "retry")]
    Retry,
    #[serde(rename = "compaction")]
    Compaction,
}

impl PartKind {
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.trim().parse().ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RunningTime {
    pub start: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CompletedTime {
    pub start: i64,
    pub end: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compacted: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ErrorTime {
    pub start: i64,
    pub end: i64,
}

/// Rich tool state used by some runtime flows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ToolState {
    Pending {
        #[serde(default = "default_json_object")]
        input: serde_json::Value,
        #[serde(default)]
        raw: String,
    },
    Running {
        #[serde(default = "default_json_object")]
        input: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
        #[serde(default)]
        time: RunningTime,
    },
    Completed {
        #[serde(default = "default_json_object")]
        input: serde_json::Value,
        #[serde(default)]
        output: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
        #[serde(default)]
        time: CompletedTime,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<serde_json::Value>>,
    },
    Error {
        #[serde(default = "default_json_object")]
        input: serde_json::Value,
        #[serde(default)]
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
        #[serde(default)]
        time: ErrorTime,
    },
}

impl ToolState {
    pub const fn status(&self) -> ToolCallStatus {
        match self {
            Self::Pending { .. } => ToolCallStatus::Pending,
            Self::Running { .. } => ToolCallStatus::Running,
            Self::Completed { .. } => ToolCallStatus::Completed,
            Self::Error { .. } => ToolCallStatus::Error,
        }
    }

    pub const fn input(&self) -> &serde_json::Value {
        match self {
            Self::Pending { input, .. }
            | Self::Running { input, .. }
            | Self::Completed { input, .. }
            | Self::Error { input, .. } => input,
        }
    }

    pub fn raw(&self) -> Option<&str> {
        match self {
            Self::Pending { raw, .. } => Some(raw.as_str()),
            _ => None,
        }
    }
}

/// A single part inside a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessagePart {
    pub id: String,
    pub part_type: PartType,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

impl MessagePart {
    pub fn new(part_type: PartType) -> Self {
        Self {
            id: new_part_id(),
            part_type,
            created_at: Utc::now(),
            message_id: None,
        }
    }

    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    pub const fn kind(&self) -> PartKind {
        self.part_type.kind()
    }
}

/// Canonical part payload.
///
/// - Writes as `snake_case` tags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PartType {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        synthetic: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ignored: Option<bool>,
    },
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        #[serde(default = "default_json_object")]
        input: serde_json::Value,
        #[serde(default)]
        status: ToolCallStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<ToolState>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename = "reasoning")]
    Reasoning { text: String },
    #[serde(rename = "file")]
    File {
        url: String,
        filename: String,
        mime: String,
    },
    #[serde(rename = "step_start")]
    StepStart {
        id: String,
        #[serde(default)]
        name: String,
    },
    #[serde(rename = "step_finish")]
    StepFinish {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    #[serde(rename = "snapshot")]
    Snapshot { content: String },
    #[serde(rename = "patch")]
    Patch {
        #[serde(default)]
        old_string: String,
        #[serde(default)]
        new_string: String,
        #[serde(default)]
        filepath: String,
    },
    #[serde(rename = "agent")]
    Agent {
        name: String,
        #[serde(default = "default_pending_status")]
        status: String,
    },
    #[serde(rename = "subtask")]
    Subtask {
        id: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_pending_status")]
        status: String,
    },
    #[serde(rename = "retry")]
    Retry {
        #[serde(default)]
        count: u32,
        #[serde(default)]
        reason: String,
    },
    #[serde(rename = "compaction")]
    Compaction {
        #[serde(default)]
        summary: String,
    },
}

impl PartType {
    pub const fn kind(&self) -> PartKind {
        match self {
            Self::Text { .. } => PartKind::Text,
            Self::ToolCall { .. } => PartKind::ToolCall,
            Self::ToolResult { .. } => PartKind::ToolResult,
            Self::Reasoning { .. } => PartKind::Reasoning,
            Self::File { .. } => PartKind::File,
            Self::StepStart { .. } => PartKind::StepStart,
            Self::StepFinish { .. } => PartKind::StepFinish,
            Self::Snapshot { .. } => PartKind::Snapshot,
            Self::Patch { .. } => PartKind::Patch,
            Self::Agent { .. } => PartKind::Agent,
            Self::Subtask { .. } => PartKind::Subtask,
            Self::Retry { .. } => PartKind::Retry,
            Self::Compaction { .. } => PartKind::Compaction,
        }
    }

    pub const fn is_activity(&self) -> bool {
        matches!(
            self,
            Self::StepStart { .. }
                | Self::StepFinish { .. }
                | Self::Snapshot { .. }
                | Self::Patch { .. }
                | Self::Agent { .. }
                | Self::Subtask { .. }
                | Self::Retry { .. }
                | Self::Compaction { .. }
        )
    }

    pub const fn is_tool(&self) -> bool {
        matches!(self, Self::ToolCall { .. } | Self::ToolResult { .. })
    }

    pub const fn is_content(&self) -> bool {
        matches!(
            self,
            Self::Text { .. } | Self::Reasoning { .. } | Self::File { .. }
        )
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text.as_str()),
            _ => None,
        }
    }

    pub fn reasoning_text(&self) -> Option<&str> {
        match self {
            Self::Reasoning { text } => Some(text.as_str()),
            _ => None,
        }
    }

    pub fn tool_call_id(&self) -> Option<&str> {
        match self {
            Self::ToolCall { id, .. } => Some(id.as_str()),
            Self::ToolResult { tool_call_id, .. } => Some(tool_call_id.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_kind_parses_snake_and_camel() {
        assert_eq!(
            "toolCall".parse::<PartKind>().ok(),
            Some(PartKind::ToolCall)
        );
        assert_eq!(
            "stepStart".parse::<PartKind>().ok(),
            Some(PartKind::StepStart)
        );
        assert_eq!(
            "tool_result".parse::<PartKind>().ok(),
            Some(PartKind::ToolResult)
        );
        assert_eq!("step-start".parse::<PartKind>().ok(), None);
    }

    #[test]
    fn part_type_deserializes_snake_case_tag() {
        let value = serde_json::json!({
            "type": "tool_call",
            "id": "call_1",
            "name": "bash",
            "input": {"command": "ls"},
            "status": "running"
        });
        let part: PartType = serde_json::from_value(value).expect("parse tool_call");
        match part {
            PartType::ToolCall {
                id, name, status, ..
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "bash");
                assert_eq!(status, ToolCallStatus::Running);
            }
            _ => panic!("expected tool call"),
        }
    }

    #[test]
    fn part_type_rejects_legacy_camel_case_tag() {
        let value = serde_json::json!({
            "type": "stepStart",
            "id": "step_1"
        });
        let parsed = serde_json::from_value::<PartType>(value);
        assert!(parsed.is_err());
    }

    #[test]
    fn part_type_rejects_kebab_step_tag() {
        let value = serde_json::json!({
            "type": "step-start",
            "id": "step_1"
        });
        let parsed = serde_json::from_value::<PartType>(value);
        assert!(parsed.is_err());
    }

    #[test]
    fn message_part_new_allocates_part_id() {
        let part = MessagePart::new(PartType::Text {
            text: "hi".to_string(),
            synthetic: None,
            ignored: None,
        });
        assert!(part.id.parse::<i64>().is_ok());
    }
}
