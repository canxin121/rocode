use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

/// Shared todo payload keys used across tools/runtime/UI.
pub mod keys {
    pub const SESSION_ID: &str = "session_id";
    pub const TODOS: &str = "todos";
    pub const COUNT: &str = "count";
    pub const NO_OP: &str = "no_op";

    pub const ID: &str = "id";
    pub const CONTENT: &str = "content";
    pub const STATUS: &str = "status";
    pub const PRIORITY: &str = "priority";
}

/// Canonical todo status strings used across the tool protocol and UI.
///
/// Wire format: snake_case strings (e.g. `"in_progress"`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, AsRefStr, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(ascii_case_insensitive)]
pub enum TodoStatus {
    #[strum(serialize = "pending")]
    Pending,
    #[strum(
        serialize = "in_progress",
        serialize = "in-progress",
        serialize = "in progress",
        serialize = "doing"
    )]
    InProgress,
    #[strum(serialize = "completed", serialize = "done")]
    Completed,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
}

impl TodoStatus {
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.trim().parse().ok()
    }
}

/// Canonical todo priority strings.
///
/// Wire format: lowercase strings (`"high"`, `"medium"`, `"low"`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, AsRefStr, EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

impl TodoPriority {
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.trim().parse().ok()
    }
}
