use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

/// Canonical server event type strings.
///
/// These values are used as:
/// - SSE `event:` names (for streaming clients)
/// - The `type` field inside JSON payloads
///
/// Keep them stable — they form a cross-crate wire contract between
/// `rocode-server`, `rocode-cli`, `rocode-tui`, and any future frontends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum ServerEventType {
    #[strum(serialize = "config.updated")]
    ConfigUpdated,
    #[strum(serialize = "session.updated")]
    SessionUpdated,
    #[strum(serialize = "session.status")]
    SessionStatus,
    #[strum(serialize = "question.created")]
    QuestionCreated,
    #[strum(
        serialize = "question.resolved",
        serialize = "question.replied",
        serialize = "question.rejected"
    )]
    QuestionResolved,
    #[strum(serialize = "permission.requested")]
    PermissionRequested,
    #[strum(serialize = "permission.resolved", serialize = "permission.replied")]
    PermissionResolved,
    #[strum(serialize = "tool_call.lifecycle")]
    ToolCallLifecycle,
    // Legacy split events (kept for forward/backward compatibility)
    #[strum(serialize = "tool_call.start")]
    ToolCallStart,
    #[strum(serialize = "tool_call.complete")]
    ToolCallComplete,
    #[strum(serialize = "execution.topology.changed")]
    ExecutionTopologyChanged,
    #[strum(serialize = "child_session.attached")]
    ChildSessionAttached,
    #[strum(serialize = "child_session.detached")]
    ChildSessionDetached,
    #[strum(serialize = "diff.updated", serialize = "session.diff")]
    DiffUpdated,
    #[strum(serialize = "output_block")]
    OutputBlock,
    #[strum(serialize = "usage")]
    Usage,
    #[strum(serialize = "error")]
    Error,
    // Internal server↔TUI request bus (not consumed by most clients)
    #[strum(serialize = "tui.request")]
    TuiRequest,
}

/// Phase of the tool call lifecycle events.
///
/// Wire format: lowercase strings (`"start"`, `"complete"`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, AsRefStr, EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum ToolCallPhase {
    Start,
    Complete,
}

/// Session run status tag used inside `session.status` payloads.
///
/// Wire format: lowercase strings (`"busy"`, `"idle"`, `"retry"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, AsRefStr, EnumString)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum SessionRunStatusType {
    Idle,
    Busy,
    Retry,
}

/// How a question request was resolved.
///
/// Wire format: snake_case strings (`"answered"`, `"rejected"`, `"cancelled"`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, AsRefStr, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum QuestionResolutionKind {
    Answered,
    Rejected,
    Cancelled,
}

/// Canonical internal bus event name strings.
///
/// These values are used with `rocode_core::bus::BusEventDef` and consumed by
/// session/server/runtime components.
///
/// Keep them stable — they form a cross-crate contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum BusEventName {
    #[strum(serialize = "mcp.tools.changed")]
    McpToolsChanged,
    #[strum(serialize = "session.compacted")]
    SessionCompacted,
    #[strum(serialize = "todo.updated")]
    TodoUpdated,
    #[strum(serialize = "agent_task.registered")]
    AgentTaskRegistered,
    #[strum(serialize = "agent_task.completed")]
    AgentTaskCompleted,
    #[strum(serialize = "file.edited")]
    FileEdited,
    #[strum(serialize = "file_watcher.updated")]
    FileWatcherUpdated,
    #[strum(serialize = "session.status")]
    SessionStatus,
    #[strum(serialize = "session.idle")]
    SessionIdle,
    #[strum(serialize = "session.created")]
    SessionCreated,
    #[strum(serialize = "session.updated")]
    SessionUpdated,
    #[strum(serialize = "session.deleted")]
    SessionDeleted,
    #[strum(serialize = "session.diff")]
    SessionDiff,
    #[strum(serialize = "session.error")]
    SessionError,
    #[strum(serialize = "message.updated")]
    MessageUpdated,
    #[strum(serialize = "message.removed")]
    MessageRemoved,
    #[strum(serialize = "message.part.updated")]
    MessagePartUpdated,
    #[strum(serialize = "message.part.removed")]
    MessagePartRemoved,
    #[strum(serialize = "message.part.delta")]
    MessagePartDelta,
    #[strum(serialize = "command.executed")]
    CommandExecuted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_event_type_parses_legacy_aliases() {
        assert_eq!(
            "question.replied".parse::<ServerEventType>().ok(),
            Some(ServerEventType::QuestionResolved)
        );
        assert_eq!(
            "permission.replied".parse::<ServerEventType>().ok(),
            Some(ServerEventType::PermissionResolved)
        );
        assert_eq!(
            "session.diff".parse::<ServerEventType>().ok(),
            Some(ServerEventType::DiffUpdated)
        );
    }

    #[test]
    fn bus_event_names_round_trip() {
        let values: &[BusEventName] = &[
            BusEventName::McpToolsChanged,
            BusEventName::SessionCompacted,
            BusEventName::TodoUpdated,
            BusEventName::AgentTaskRegistered,
            BusEventName::AgentTaskCompleted,
            BusEventName::FileEdited,
            BusEventName::FileWatcherUpdated,
            BusEventName::SessionStatus,
            BusEventName::SessionIdle,
            BusEventName::SessionCreated,
            BusEventName::SessionUpdated,
            BusEventName::SessionDeleted,
            BusEventName::SessionDiff,
            BusEventName::SessionError,
            BusEventName::MessageUpdated,
            BusEventName::MessageRemoved,
            BusEventName::MessagePartUpdated,
            BusEventName::MessagePartRemoved,
            BusEventName::MessagePartDelta,
            BusEventName::CommandExecuted,
        ];
        for value in values {
            assert_eq!(value.to_string().parse::<BusEventName>().ok(), Some(*value));
            assert_eq!(value.to_string(), value.as_ref());
        }
    }
}
