use strum_macros::{AsRefStr, Display, EnumString};

/// Shared auth payload keys used by server/provider/CLI auth flows.
pub mod auth_keys {
    pub const TYPE: &str = "type";
    pub const PROVIDER: &str = "provider";
    pub const SUCCESS: &str = "success";

    pub const API_KEY_SNAKE: &str = "api_key";
    pub const API_KEY_CAMEL: &str = "apiKey";
    pub const TOKEN: &str = "token";
    pub const KEY: &str = "key";
    pub const ACCESS: &str = "access";
    pub const REFRESH: &str = "refresh";
    pub const EXPIRES: &str = "expires";
    pub const ACCOUNT_ID: &str = "accountId";
    pub const ENTERPRISE_URL: &str = "enterpriseUrl";
}

/// Provider option-map keys (config/bootstrap/runtime state).
pub mod option_keys {
    pub const API_KEY: &str = "apiKey";
    pub const API_KEY_SNAKE: &str = "api_key";
    pub const API_KEY_LOWER: &str = "apikey";
    pub const BASE_URL: &str = "baseURL";
    pub const BASE_URL_CAMEL: &str = "baseUrl";
    pub const URL: &str = "url";
    pub const API: &str = "api";
    pub const ACCOUNT_ID: &str = "accountId";
}

/// Reusable tolerant-reader key sets for provider options.
pub mod option_keysets {
    use super::option_keys;

    pub const API_KEY_ANY: &[&str] = &[
        option_keys::API_KEY,
        option_keys::API_KEY_SNAKE,
        option_keys::API_KEY_LOWER,
    ];

    pub const BASE_URL_ANY: &[&str] = &[
        option_keys::BASE_URL,
        option_keys::BASE_URL_CAMEL,
        option_keys::URL,
        option_keys::API,
    ];
}

/// Canonical provider finish reason strings (wire format).
///
/// These are **normalized** values surfaced across session/runtime layers, and
/// are intentionally stable because they are stored in message metadata and
/// used by multiple frontends.
///
/// Canonical values:
/// - `"stop"`
/// - `"tool-calls"`
/// - `"length"`
/// - `"content_filter"`
/// - `"error"`
/// - `"unknown"`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum ProviderFinishReasonWire {
    #[strum(to_string = "stop", serialize = "stop", serialize = "end_turn")]
    Stop,
    #[strum(
        to_string = "tool-calls",
        serialize = "tool-calls",
        serialize = "tool_calls",
        serialize = "tool_use"
    )]
    ToolCalls,
    #[strum(serialize = "length")]
    Length,
    #[strum(
        to_string = "content_filter",
        serialize = "content_filter",
        serialize = "content-filter"
    )]
    ContentFilter,
    #[strum(serialize = "error")]
    Error,
    #[strum(serialize = "unknown")]
    Unknown,
}

/// Canonical "tool_name" strings used for **provider-managed** tool calls.
///
/// These are used for providers that surface internal tool calls as streaming
/// events (e.g. OpenAI Responses API output items like web search / code interpreter).
///
/// Keep these stable — they are part of the runtime wire contract between
/// `rocode-provider`, `rocode-orchestrator`, and UI layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum ProviderToolCallNameWire {
    #[strum(
        to_string = "web_search_call",
        serialize = "web_search_call",
        serialize = "web-search-call"
    )]
    WebSearchCall,
    #[strum(
        to_string = "code_interpreter_call",
        serialize = "code_interpreter_call",
        serialize = "code-interpreter-call"
    )]
    CodeInterpreterCall,
    #[strum(
        to_string = "file_search_call",
        serialize = "file_search_call",
        serialize = "file-search-call"
    )]
    FileSearchCall,
    #[strum(
        to_string = "image_generation_call",
        serialize = "image_generation_call",
        serialize = "image-generation-call"
    )]
    ImageGenerationCall,
    #[strum(
        to_string = "computer_call",
        serialize = "computer_call",
        serialize = "computer-call"
    )]
    ComputerCall,
    #[strum(
        to_string = "local_shell",
        serialize = "local_shell",
        serialize = "local-shell",
        serialize = "local_shell_call",
        serialize = "local-shell-call"
    )]
    LocalShell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_finish_reason_is_canonical() {
        assert_eq!(ProviderFinishReasonWire::Stop.to_string(), "stop");
        assert_eq!(
            ProviderFinishReasonWire::ToolCalls.to_string(),
            "tool-calls"
        );
        assert_eq!(ProviderFinishReasonWire::Unknown.to_string(), "unknown");
    }

    #[test]
    fn provider_finish_reason_parses_aliases() {
        assert_eq!(
            "end_turn".parse::<ProviderFinishReasonWire>().ok(),
            Some(ProviderFinishReasonWire::Stop)
        );
        assert_eq!(
            "tool_calls".parse::<ProviderFinishReasonWire>().ok(),
            Some(ProviderFinishReasonWire::ToolCalls)
        );
        assert_eq!(
            "tool_use".parse::<ProviderFinishReasonWire>().ok(),
            Some(ProviderFinishReasonWire::ToolCalls)
        );
    }

    #[test]
    fn provider_tool_call_names_round_trip() {
        let cases: &[ProviderToolCallNameWire] = &[
            ProviderToolCallNameWire::WebSearchCall,
            ProviderToolCallNameWire::CodeInterpreterCall,
            ProviderToolCallNameWire::FileSearchCall,
            ProviderToolCallNameWire::ImageGenerationCall,
            ProviderToolCallNameWire::ComputerCall,
            ProviderToolCallNameWire::LocalShell,
        ];
        for value in cases {
            assert_eq!(
                value.to_string().parse::<ProviderToolCallNameWire>().ok(),
                Some(*value)
            );
            assert_eq!(value.to_string(), value.as_ref());
        }
    }
}
