mod anthropic;
mod bedrock;
mod copilot;
mod gitlab;
mod google;
mod openai;
mod vertex;

use std::sync::Arc;

use crate::ProviderConfig;

pub use anthropic::AnthropicProtocol;
pub use bedrock::BedrockProtocol;
pub use copilot::CopilotProtocol;
pub use gitlab::GitLabProtocol;
pub use google::GoogleProtocol;
pub use openai::OpenAIProtocol;
pub use vertex::VertexProtocol;

use crate::{Protocol, ProtocolImpl};

pub(super) fn runtime_pipeline_enabled(config: &ProviderConfig) -> bool {
    config
        .option_bool(&["runtime_pipeline"])
        .unwrap_or_else(|| parse_runtime_pipeline_env(std::env::var("ROCODE_RUNTIME_PIPELINE").ok().as_deref()))
}

fn parse_runtime_pipeline_env(value: Option<&str>) -> bool {
    value
        .and_then(|v| {
            let lower = v.trim().to_ascii_lowercase();
            if matches!(lower.as_str(), "1" | "true" | "yes" | "on") {
                Some(true)
            } else if matches!(lower.as_str(), "0" | "false" | "no" | "off") {
                Some(false)
            } else {
                None
            }
        })
        .unwrap_or(true)
}

pub fn create_protocol_impl(protocol: Protocol) -> Arc<dyn ProtocolImpl> {
    match protocol {
        Protocol::OpenAI => Arc::new(OpenAIProtocol::new()),
        Protocol::Anthropic => Arc::new(AnthropicProtocol::new()),
        Protocol::Google => Arc::new(GoogleProtocol::new()),
        Protocol::Bedrock => Arc::new(BedrockProtocol::new()),
        Protocol::Vertex => Arc::new(VertexProtocol::new()),
        Protocol::GitHubCopilot => Arc::new(CopilotProtocol::new()),
        Protocol::GitLab => Arc::new(GitLabProtocol::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_pipeline_prefers_config_option() {
        let config = ProviderConfig::new("openai", "", "").with_option("runtime_pipeline", json!(false));
        assert!(!runtime_pipeline_enabled(&config));
    }

    #[test]
    fn parse_runtime_pipeline_env_supports_common_values() {
        assert!(parse_runtime_pipeline_env(Some("1")));
        assert!(parse_runtime_pipeline_env(Some("true")));
        assert!(parse_runtime_pipeline_env(Some("yes")));
        assert!(!parse_runtime_pipeline_env(Some("0")));
        assert!(!parse_runtime_pipeline_env(Some("false")));
        assert!(!parse_runtime_pipeline_env(Some("off")));
        assert!(parse_runtime_pipeline_env(Some("unknown")));
        assert!(parse_runtime_pipeline_env(None));
    }
}
