use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoInfo {
    pub content: String,
    pub status: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub session_id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
    pub position: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, EnumString, AsRefStr)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum TodoStatus {
    Pending,
    #[serde(alias = "in-progress", alias = "in progress")]
    #[strum(serialize = "in-progress", serialize = "in progress")]
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, EnumString, AsRefStr)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

pub fn parse_status(status: &str) -> TodoStatus {
    status.parse::<TodoStatus>().unwrap_or(TodoStatus::Pending)
}

pub fn parse_priority(priority: &str) -> TodoPriority {
    priority
        .parse::<TodoPriority>()
        .unwrap_or(TodoPriority::Medium)
}
