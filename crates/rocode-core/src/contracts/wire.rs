/// Shared JSON field-name constants used across wire payloads.
///
/// These keys appear in:
/// - SSE server events (`rocode-server` → CLI/TUI/Web)
/// - Bus event payloads (internal runtime hooks)
/// - Plugin hook I/O shims
///
/// Keep them stable — they are part of the cross-crate contract.
pub mod keys {
    /// Generic payload type discriminant key.
    pub const TYPE: &str = "type";

    /// Canonical session identifier key used in event payloads.
    pub const SESSION_ID: &str = "sessionID";
    /// Canonical message identifier key used in event payloads.
    pub const MESSAGE_ID: &str = "messageID";
    /// Canonical parent session identifier key used in tree-attach events.
    pub const PARENT_ID: &str = "parentID";
    /// Canonical child session identifier key used in tree-attach events.
    pub const CHILD_ID: &str = "childID";
    /// Canonical question request identifier key used in interaction events.
    pub const REQUEST_ID: &str = "requestID";
    /// Canonical permission identifier key used in permission events.
    pub const PERMISSION_ID: &str = "permissionID";
    /// Canonical tool call identifier key used in event payloads.
    pub const TOOL_CALL_ID: &str = "toolCallId";
    /// Canonical wrapped output block key used by `output_block` events.
    pub const BLOCK: &str = "block";
    /// Common error-message field key used by `error` events.
    pub const ERROR: &str = "error";
    /// Common message field key used by `error` events and status payloads.
    pub const MESSAGE: &str = "message";

    /// Execution topology identifier key used in stage/execution events.
    pub const EXECUTION_ID: &str = "executionID";
    /// Scheduler stage identifier key used in stage/execution events.
    pub const STAGE_ID: &str = "stageID";
}

/// HTTP header names used by internal server/plugin transport.
pub mod headers {
    pub const X_ROCODE_PLUGIN_INTERNAL: &str = "x-rocode-plugin-internal";
    pub const X_ROCODE_INTERNAL_TOKEN: &str = "x-rocode-internal-token";
}

/// Common non-identifier payload field names reused across wire contracts.
pub mod fields {
    pub const SOURCE: &str = "source";
    pub const STATUS: &str = "status";
    pub const PHASE: &str = "phase";
    pub const ROLE: &str = "role";
    pub const TOOL_NAME: &str = "toolName";
    pub const RESOLUTION: &str = "resolution";
    pub const QUESTIONS: &str = "questions";
    pub const QUESTION: &str = "question";
    pub const HEADER: &str = "header";
    pub const OPTIONS: &str = "options";
    pub const MULTIPLE: &str = "multiple";
    pub const LABEL: &str = "label";
    pub const VALUE: &str = "value";
    pub const INFO: &str = "info";
    pub const ID: &str = "id";
    pub const DONE: &str = "done";
    pub const PROMPT_TOKENS: &str = "prompt_tokens";
    pub const COMPLETION_TOKENS: &str = "completion_tokens";
    pub const ADDITIONS: &str = "additions";
    pub const DELETIONS: &str = "deletions";
}

/// Small JSON key lookup helpers for wire payload readers.
pub mod selectors {
    use serde_json::Value;

    pub fn first_value<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a Value> {
        keys.iter().find_map(|key| payload.get(*key))
    }

    pub fn first_str<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
        first_value(payload, keys).and_then(Value::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::{keys, selectors};

    #[test]
    fn session_id_selector_uses_canonical_key() {
        let payload = serde_json::json!({
            "sessionID": "session-1",
        });

        assert_eq!(
            selectors::first_str(&payload, &[keys::SESSION_ID]),
            Some("session-1")
        );
    }

    #[test]
    fn permission_id_selector_uses_canonical_key() {
        let payload = serde_json::json!({
            "permissionID": "permission-1",
        });

        assert_eq!(
            selectors::first_str(&payload, &[keys::PERMISSION_ID]),
            Some("permission-1")
        );
    }

    #[test]
    fn tool_call_id_selector_uses_canonical_key() {
        let payload = serde_json::json!({
            "toolCallId": "tool-1",
        });

        assert_eq!(
            selectors::first_str(&payload, &[keys::TOOL_CALL_ID]),
            Some("tool-1")
        );
    }
}
