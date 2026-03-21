use serde::{Deserialize, Serialize};
use strum_macros::Display;

/// Normalized finish reason for assistant messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum FinishReason {
    #[strum(serialize = "stop")]
    Stop,
    #[strum(serialize = "tool-calls")]
    ToolCalls,
    #[strum(serialize = "length")]
    Length,
    #[strum(serialize = "content_filter")]
    ContentFilter,
    #[strum(serialize = "error")]
    Error,
    #[strum(serialize = "unknown")]
    Unknown,
    /// Non-standard provider reason.
    #[strum(to_string = "{0}")]
    Custom(String),
}

impl FinishReason {
    fn normalize_style(value: &str) -> String {
        let trimmed = value.trim();
        let mut out = String::with_capacity(trimmed.len() + 2);
        for (idx, ch) in trimmed.chars().enumerate() {
            if ch.is_ascii_uppercase() {
                if idx > 0 {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
            } else {
                out.push(ch.to_ascii_lowercase());
            }
        }
        out
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Custom(_))
    }
}

impl From<&str> for FinishReason {
    fn from(value: &str) -> Self {
        let raw = value.trim();
        if raw.is_empty() {
            return Self::Unknown;
        }

        if raw.eq_ignore_ascii_case("tool-calls") {
            return Self::ToolCalls;
        }

        let normalized = Self::normalize_style(raw);
        match normalized.as_str() {
            "stop" => Self::Stop,
            "tool_calls" => Self::ToolCalls,
            "length" => Self::Length,
            "content_filter" => Self::ContentFilter,
            "error" => Self::Error,
            "unknown" => Self::Unknown,
            _ => Self::Custom(raw.to_string()),
        }
    }
}

impl From<String> for FinishReason {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<FinishReason> for String {
    fn from(value: FinishReason) -> Self {
        value.to_string()
    }
}

impl std::str::FromStr for FinishReason {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

/// Normalize a free-form finish reason into canonical wire text.
pub fn normalize_finish_reason(value: impl AsRef<str>) -> String {
    FinishReason::from(value.as_ref()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_style_variants() {
        assert_eq!(FinishReason::from("tool-calls"), FinishReason::ToolCalls);
        assert_eq!(FinishReason::from("toolCalls"), FinishReason::ToolCalls);
        assert_eq!(
            FinishReason::from("contentFilter"),
            FinishReason::ContentFilter
        );
        assert_eq!(normalize_finish_reason("toolCalls"), "tool-calls");
    }

    #[test]
    fn keeps_custom_reason() {
        assert_eq!(
            FinishReason::from("provider_specific").to_string(),
            "provider_specific"
        );
        assert_eq!(FinishReason::from("end_turn").to_string(), "end_turn");
    }
}
