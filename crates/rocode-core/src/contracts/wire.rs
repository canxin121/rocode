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
    /// Canonical tool call identifier key used in event payloads.
    pub const TOOL_CALL_ID: &str = "toolCallId";

    /// Execution topology identifier key used in stage/execution events.
    pub const EXECUTION_ID: &str = "executionID";
    /// Scheduler stage identifier key used in stage/execution events.
    pub const STAGE_ID: &str = "stageID";
}

