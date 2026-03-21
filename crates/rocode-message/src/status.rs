use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

/// Tool-call lifecycle status used by `PartType::ToolCall`.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash, Display, EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum ToolCallStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Error,
}
