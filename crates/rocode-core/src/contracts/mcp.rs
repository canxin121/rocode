use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

/// Canonical MCP server connection status strings (wire format).
///
/// These values are produced by the server and consumed by CLI/TUI/Web.
/// Keep them stable.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, AsRefStr, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum McpConnectionStatusWire {
    Connected,
    #[strum(serialize = "failed", serialize = "error")]
    Failed,
    NeedsAuth,
    NeedsClientRegistration,
    Disabled,
    Disconnected,
}

impl McpConnectionStatusWire {
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.trim().parse().ok()
    }

    pub fn from_str_lossy(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Allow richer statuses like: "needs_client_registration: <error>"
        // and tolerate spaces/hyphens.
        let normalized = trimmed
            .to_ascii_lowercase()
            .replace('-', "_")
            .replace(' ', "_");

        if let Some((prefix, _)) = normalized.split_once(':') {
            return prefix.trim().parse().ok();
        }

        normalized.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_connection_status_round_trips() {
        let values: &[McpConnectionStatusWire] = &[
            McpConnectionStatusWire::Connected,
            McpConnectionStatusWire::Failed,
            McpConnectionStatusWire::NeedsAuth,
            McpConnectionStatusWire::NeedsClientRegistration,
            McpConnectionStatusWire::Disabled,
            McpConnectionStatusWire::Disconnected,
        ];
        for value in values {
            assert_eq!(
                value.to_string().parse::<McpConnectionStatusWire>().ok(),
                Some(*value)
            );
            assert_eq!(value.to_string(), value.as_ref());
        }
    }

    #[test]
    fn mcp_connection_status_parses_prefix_error() {
        assert_eq!(
            McpConnectionStatusWire::from_str_lossy("needs_client_registration: boom"),
            Some(McpConnectionStatusWire::NeedsClientRegistration)
        );
    }
}
