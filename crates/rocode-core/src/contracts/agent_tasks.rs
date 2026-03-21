use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString, IntoStaticStr};

/// Bus event payload keys for `agent_task.*` events.
pub mod bus_keys {
    pub const TASK_ID: &str = "task_id";
    pub const SESSION_ID: &str = "session_id";
    pub const AGENT_NAME: &str = "agent_name";
    pub const PARENT_TOOL_CALL_ID: &str = "parent_tool_call_id";
}

/// Canonical agent task lifecycle status labels.
///
/// These values are used as:
/// - `task_flow` tool `status_filter` inputs
/// - API/UI status strings for agent task registry projections
///
/// Keep them stable — they are part of the cross-crate wire contract.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Display,
    AsRefStr,
    IntoStaticStr,
    EnumString,
    EnumIter,
)]
#[serde(rename_all = "lowercase")]
#[strum(ascii_case_insensitive)]
pub enum AgentTaskStatusKind {
    #[strum(serialize = "pending")]
    Pending,
    #[strum(
        serialize = "running",
        serialize = "in_progress",
        serialize = "in-progress",
        serialize = "inprogress"
    )]
    Running,
    #[strum(
        serialize = "completed",
        serialize = "done",
        serialize = "complete",
        serialize = "success"
    )]
    Completed,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
    #[strum(serialize = "failed", serialize = "error", serialize = "failure")]
    Failed,
}

impl AgentTaskStatusKind {
    pub fn allowed_values() -> Vec<&'static str> {
        Self::iter().map(|value| value.into()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases() {
        assert_eq!(
            "in-progress".parse::<AgentTaskStatusKind>().ok(),
            Some(AgentTaskStatusKind::Running)
        );
        assert_eq!(
            "done".parse::<AgentTaskStatusKind>().ok(),
            Some(AgentTaskStatusKind::Completed)
        );
        assert_eq!(
            "canceled".parse::<AgentTaskStatusKind>().ok(),
            Some(AgentTaskStatusKind::Cancelled)
        );
        assert_eq!(
            "error".parse::<AgentTaskStatusKind>().ok(),
            Some(AgentTaskStatusKind::Failed)
        );
    }
}
