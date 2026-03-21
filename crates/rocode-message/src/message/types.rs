use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::usage::MessageUsage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FilePartSource {
    #[serde(rename = "file")]
    File { path: String, text: FileSourceText },
    #[serde(rename = "symbol")]
    Symbol {
        path: String,
        name: String,
        kind: i32,
        range: LspRange,
        text: FileSourceText,
    },
    #[serde(rename = "resource")]
    Resource {
        client_name: String,
        uri: String,
        text: FileSourceText,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSourceText {
    pub value: String,
    pub start: i32,
    pub end: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: i32,
    pub character: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub mime: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<FilePartSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AgentSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSource {
    pub value: String,
    pub start: i32,
    pub end: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub auto: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub prompt: String,
    pub description: String,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub attempt: i32,
    pub error: ApiError,
    pub time: RetryTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryTime {
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStartPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFinishPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    pub cost: f64,
    pub tokens: StepTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
    pub input: i32,
    pub output: i32,
    pub reasoning: i32,
    pub cache: CacheTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheTokens {
    pub read: i32,
    pub write: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ToolState {
    #[serde(rename = "pending")]
    Pending {
        input: serde_json::Value,
        raw: String,
    },
    #[serde(rename = "running")]
    Running {
        input: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
        time: RunningTime,
    },
    #[serde(rename = "completed")]
    Completed {
        input: serde_json::Value,
        output: String,
        title: String,
        metadata: HashMap<String, serde_json::Value>,
        time: CompletedTime,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachments: Option<Vec<FilePart>>,
    },
    #[serde(rename = "error")]
    Error {
        input: serde_json::Value,
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
        time: ErrorTime,
    },
}

impl ToolState {
    pub const fn status(&self) -> crate::status::ToolCallStatus {
        match self {
            Self::Pending { .. } => crate::status::ToolCallStatus::Pending,
            Self::Running { .. } => crate::status::ToolCallStatus::Running,
            Self::Completed { .. } => crate::status::ToolCallStatus::Completed,
            Self::Error { .. } => crate::status::ToolCallStatus::Error,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningTime {
    pub start: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedTime {
    pub start: i64,
    pub end: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compacted: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorTime {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPart {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub call_id: String,
    pub tool: String,
    pub state: ToolState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum MessageInfo {
    #[serde(rename = "user")]
    User {
        id: String,
        session_id: String,
        time: UserTime,
        agent: String,
        model: ModelRef,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<OutputFormat>,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<UserSummary>,
        #[serde(skip_serializing_if = "Option::is_none")]
        system: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tools: Option<HashMap<String, bool>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        variant: Option<String>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        id: String,
        session_id: String,
        time: AssistantTime,
        parent_id: String,
        model_id: String,
        provider_id: String,
        mode: String,
        agent: String,
        path: MessagePath,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<bool>,
        cost: f64,
        tokens: AssistantTokens,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<MessageError>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        variant: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        finish: Option<String>,
    },
    #[serde(rename = "system")]
    System {
        id: String,
        session_id: String,
        time: UserTime,
    },
    #[serde(rename = "tool")]
    Tool {
        id: String,
        session_id: String,
        time: UserTime,
    },
}

impl MessageInfo {
    pub fn id(&self) -> &str {
        match self {
            MessageInfo::User { id, .. }
            | MessageInfo::Assistant { id, .. }
            | MessageInfo::System { id, .. }
            | MessageInfo::Tool { id, .. } => id,
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            MessageInfo::User { session_id, .. }
            | MessageInfo::Assistant { session_id, .. }
            | MessageInfo::System { session_id, .. }
            | MessageInfo::Tool { session_id, .. } => session_id,
        }
    }

    pub fn role(&self) -> rocode_types::Role {
        match self {
            MessageInfo::User { .. } => rocode_types::Role::User,
            MessageInfo::Assistant { .. } => rocode_types::Role::Assistant,
            MessageInfo::System { .. } => rocode_types::Role::System,
            MessageInfo::Tool { .. } => rocode_types::Role::Tool,
        }
    }

    pub fn created_at_millis(&self) -> i64 {
        match self {
            MessageInfo::User { time, .. }
            | MessageInfo::System { time, .. }
            | MessageInfo::Tool { time, .. } => time.created,
            MessageInfo::Assistant { time, .. } => time.created,
        }
    }

    pub fn finish_reason(&self) -> Option<&str> {
        match self {
            MessageInfo::Assistant { finish, .. } => finish.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTime {
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub diffs: Vec<FileDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_content: Option<String>,
    pub new_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTime {
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePath {
    pub cwd: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
    pub input: i32,
    pub output: i32,
    pub reasoning: i32,
    pub cache: CacheTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "name")]
pub enum MessageError {
    #[serde(rename = "OutputLengthError")]
    OutputLengthError { message: String },
    #[serde(rename = "AbortedError")]
    AbortedError { message: String },
    #[serde(rename = "StructuredOutputError")]
    StructuredOutputError { message: String, retries: i32 },
    #[serde(rename = "AuthError")]
    AuthError {
        provider_id: String,
        message: String,
    },
    #[serde(rename = "APIError")]
    ApiError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<i32>,
        is_retryable: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        response_headers: Option<HashMap<String, String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        response_body: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, String>>,
    },
    #[serde(rename = "ContextOverflowError")]
    ContextOverflowError {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        response_body: Option<String>,
    },
    #[serde(rename = "UnknownError")]
    Unknown { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputFormat {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "json_schema")]
    JsonSchema {
        schema: serde_json::Value,
        #[serde(default = "default_retry_count")]
        retry_count: i32,
    },
}

fn default_retry_count() -> i32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageWithParts {
    pub info: MessageInfo,
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<MessageUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Part {
    #[serde(rename = "text")]
    Text {
        id: String,
        session_id: String,
        message_id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        synthetic: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ignored: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        time: Option<TextTime>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    #[serde(rename = "subtask")]
    Subtask(SubtaskPart),
    #[serde(rename = "reasoning")]
    Reasoning {
        id: String,
        session_id: String,
        message_id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
        time: ReasoningTime,
    },
    #[serde(rename = "file")]
    File(FilePart),
    #[serde(rename = "tool")]
    Tool(ToolPart),
    #[serde(rename = "step-start")]
    StepStart(StepStartPart),
    #[serde(rename = "step-finish")]
    StepFinish(StepFinishPart),
    #[serde(rename = "snapshot")]
    Snapshot {
        id: String,
        session_id: String,
        message_id: String,
        snapshot: String,
    },
    #[serde(rename = "patch")]
    Patch {
        id: String,
        session_id: String,
        message_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old: Option<String>,
        hash: String,
        files: Vec<String>,
    },
    #[serde(rename = "agent")]
    Agent(AgentPart),
    #[serde(rename = "retry")]
    Retry(RetryPart),
    #[serde(rename = "compaction")]
    Compaction(CompactionPart),
}

impl Part {
    /// Get the ID of this part, regardless of variant.
    pub fn id(&self) -> Option<&str> {
        match self {
            Part::Text { id, .. } => Some(id),
            Part::Subtask(p) => Some(&p.id),
            Part::Reasoning { id, .. } => Some(id),
            Part::File(p) => Some(&p.id),
            Part::Tool(p) => Some(&p.id),
            Part::StepStart(p) => Some(&p.id),
            Part::StepFinish(p) => Some(&p.id),
            Part::Snapshot { id, .. } => Some(id),
            Part::Patch { id, .. } => Some(id),
            Part::Agent(p) => Some(&p.id),
            Part::Retry(p) => Some(&p.id),
            Part::Compaction(p) => Some(&p.id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextTime {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTime {
    pub start: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<i32>,
    pub is_retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartDelta {
    pub session_id: String,
    pub message_id: String,
    pub part_id: String,
    pub field: String,
    pub delta: String,
}

/// Events emitted by the message system, mirroring the TS `MessageV2.Event` namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageEvent {
    #[serde(rename = "message.updated")]
    Updated { info: MessageInfo },
    #[serde(rename = "message.removed")]
    Removed {
        session_id: String,
        message_id: String,
    },
    #[serde(rename = "message.part.updated")]
    PartUpdated { part: Part },
    #[serde(rename = "message.part.delta")]
    PartDelta {
        session_id: String,
        message_id: String,
        part_id: String,
        field: String,
        delta: String,
    },
    #[serde(rename = "message.part.removed")]
    PartRemoved {
        session_id: String,
        message_id: String,
        part_id: String,
    },
}
