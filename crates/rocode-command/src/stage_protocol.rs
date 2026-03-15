//! Stage Protocol — canonical protocol types for the three-layer stage architecture.
//!
//! Three orthogonal layers, each with a single authority:
//!
//! | Layer              | Struct          | Purpose                                   |
//! |--------------------|-----------------|-------------------------------------------|
//! | Stage Summary      | [`StageSummary`]  | Stable card the user sees (aggregated)    |
//! | Execution Topology | [`ExecutionNode`] | Active tree: stage→agent→tool/question    |
//! | Raw SSE            | [`StageEvent`]    | Real-time event stream & history replay   |
//!
//! All three live in `rocode-command` so CLI, TUI, and Server can consume them
//! at zero extra dependency cost.

use serde::{Deserialize, Serialize};

// ─── Stage Summary Layer ───────────────────────────────────────────────

/// Aggregated stage card shown to the user.
///
/// Projected from the presentation-level [`super::output_blocks::SchedulerStageBlock`]
/// via its `to_summary()` method. Adapters consume this shape instead of
/// reaching into the presentation block directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageSummary {
    pub stage_id: String,
    pub stage_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_total: Option<u64>,
    pub status: StageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event: Option<String>,
    pub active_agent_count: u32,
    pub active_tool_count: u32,
    pub child_session_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_child_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Running,
    Waiting,
    Done,
    Cancelled,
    Cancelling,
    Blocked,
    Retrying,
}

impl StageStatus {
    /// Parse a status string leniently, defaulting to `Running` for unknown values.
    pub fn from_str_lossy(s: Option<&str>) -> Self {
        match s {
            Some("done") => Self::Done,
            Some("cancelled") => Self::Cancelled,
            Some("cancelling") => Self::Cancelling,
            Some("waiting") => Self::Waiting,
            Some("blocked") => Self::Blocked,
            Some("retrying") => Self::Retrying,
            Some("running") => Self::Running,
            _ => Self::Running,
        }
    }
}

// ─── Execution Topology Layer ──────────────────────────────────────────

/// A single node in the execution topology tree.
///
/// Server-internal [`ExecutionRecord`](rocode_server) projects into this
/// via its `to_node()` method so that the protocol shape is decoupled
/// from internal bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub execution_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_id: Option<String>,
    pub kind: ExecutionNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub status: ExecutionNodeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionNodeKind {
    Stage,
    Agent,
    Tool,
    Question,
    Subsession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionNodeStatus {
    Running,
    Waiting,
    Cancelling,
    Retry,
    Done,
}

// ─── Raw SSE Layer ─────────────────────────────────────────────────────

/// Structured SSE event envelope carrying `stage_id` / `execution_id`
/// for filtering and replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageEvent {
    pub event_id: String,
    pub scope: EventScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    pub event_type: String,
    pub ts: i64,
    pub payload: serde_json::Value,
}

impl StageEvent {
    /// Create a new event with auto-generated `event_id` and timestamp.
    pub fn new(
        scope: EventScope,
        stage_id: Option<String>,
        execution_id: Option<String>,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4().simple()),
            scope,
            stage_id,
            execution_id,
            event_type: event_type.into(),
            ts: chrono::Utc::now().timestamp_millis(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventScope {
    Session,
    Stage,
    Agent,
}

// ─── Helpers ───────────────────────────────────────────────────────────

/// Parse a step limit from the raw `loop_budget` string.
///
/// Recognises formats like `"step-limit:3"` → `Some(3)`.
/// Returns `None` for `"unbounded"`, unknown, or absent values.
pub fn parse_step_limit_from_budget(budget: Option<&str>) -> Option<u64> {
    let s = budget?;
    let rest = s.strip_prefix("step-limit:")?;
    rest.trim().parse::<u64>().ok()
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── StageSummary serde roundtrip ──

    #[test]
    fn stage_summary_serde_roundtrip() {
        let summary = StageSummary {
            stage_id: "stage_abc123".into(),
            stage_name: "planning".into(),
            index: Some(1),
            total: Some(3),
            step: Some(2),
            step_total: Some(5),
            status: StageStatus::Running,
            prompt_tokens: Some(100),
            completion_tokens: Some(200),
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            focus: Some("analyzing code".into()),
            last_event: Some("tool_call".into()),
            active_agent_count: 2,
            active_tool_count: 1,
            child_session_count: 0,
            primary_child_session_id: None,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let back: StageSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary, back);
    }

    #[test]
    fn stage_summary_omits_none_fields() {
        let summary = StageSummary {
            stage_id: "s1".into(),
            stage_name: "init".into(),
            index: None,
            total: None,
            step: None,
            step_total: None,
            status: StageStatus::Done,
            prompt_tokens: None,
            completion_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            focus: None,
            last_event: None,
            active_agent_count: 0,
            active_tool_count: 0,
            child_session_count: 0,
            primary_child_session_id: None,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("\"index\""));
        assert!(!json.contains("\"focus\""));
        assert!(!json.contains("\"primary_child_session_id\""));
    }

    // ── ExecutionNode serde roundtrip ──

    #[test]
    fn execution_node_serde_roundtrip() {
        let node = ExecutionNode {
            execution_id: "exec_001".into(),
            parent_execution_id: Some("exec_000".into()),
            stage_id: Some("stage_abc".into()),
            kind: ExecutionNodeKind::Tool,
            label: Some("read_file".into()),
            status: ExecutionNodeStatus::Running,
            waiting_on: None,
            started_at: 1710000000000,
            updated_at: 1710000001000,
            session_id: "sess_xyz".into(),
            child_session_id: None,
        };

        let json = serde_json::to_string(&node).unwrap();
        let back: ExecutionNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node, back);
    }

    // ── StageEvent builder ──

    #[test]
    fn stage_event_new_generates_valid_id_and_ts() {
        let evt = StageEvent::new(
            EventScope::Stage,
            Some("stage_1".into()),
            Some("exec_1".into()),
            "tool_started",
            serde_json::json!({"tool": "bash"}),
        );

        assert!(evt.event_id.starts_with("evt_"));
        assert!(evt.event_id.len() > 10);
        assert!(evt.ts > 0);
        assert_eq!(evt.event_type, "tool_started");
        assert_eq!(evt.scope, EventScope::Stage);
    }

    #[test]
    fn stage_event_serde_roundtrip() {
        let evt = StageEvent::new(
            EventScope::Session,
            None,
            None,
            "session_started",
            serde_json::json!({}),
        );

        let json = serde_json::to_string(&evt).unwrap();
        let back: StageEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, back);
    }

    // ── StageStatus::from_str_lossy ──

    #[test]
    fn stage_status_from_str_lossy_known_variants() {
        assert_eq!(StageStatus::from_str_lossy(Some("done")), StageStatus::Done);
        assert_eq!(
            StageStatus::from_str_lossy(Some("cancelled")),
            StageStatus::Cancelled
        );
        assert_eq!(
            StageStatus::from_str_lossy(Some("cancelling")),
            StageStatus::Cancelling
        );
        assert_eq!(
            StageStatus::from_str_lossy(Some("waiting")),
            StageStatus::Waiting
        );
        assert_eq!(
            StageStatus::from_str_lossy(Some("blocked")),
            StageStatus::Blocked
        );
        assert_eq!(
            StageStatus::from_str_lossy(Some("retrying")),
            StageStatus::Retrying
        );
        assert_eq!(
            StageStatus::from_str_lossy(Some("running")),
            StageStatus::Running
        );
    }

    #[test]
    fn stage_status_from_str_lossy_unknown_defaults_running() {
        assert_eq!(
            StageStatus::from_str_lossy(Some("banana")),
            StageStatus::Running
        );
        assert_eq!(StageStatus::from_str_lossy(None), StageStatus::Running);
    }

    // ── ExecutionNodeKind serde ──

    #[test]
    fn execution_node_kind_serde() {
        let cases = [
            (ExecutionNodeKind::Stage, "\"stage\""),
            (ExecutionNodeKind::Agent, "\"agent\""),
            (ExecutionNodeKind::Tool, "\"tool\""),
            (ExecutionNodeKind::Question, "\"question\""),
            (ExecutionNodeKind::Subsession, "\"subsession\""),
        ];
        for (kind, expected_json) in cases {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, expected_json);
            let back: ExecutionNodeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    // ── ExecutionNodeStatus serde ──

    #[test]
    fn execution_node_status_serde() {
        let cases = [
            (ExecutionNodeStatus::Running, "\"running\""),
            (ExecutionNodeStatus::Waiting, "\"waiting\""),
            (ExecutionNodeStatus::Cancelling, "\"cancelling\""),
            (ExecutionNodeStatus::Retry, "\"retry\""),
            (ExecutionNodeStatus::Done, "\"done\""),
        ];
        for (status, expected_json) in cases {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected_json);
            let back: ExecutionNodeStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    // ── EventScope serde ──

    #[test]
    fn event_scope_serde() {
        let cases = [
            (EventScope::Session, "\"session\""),
            (EventScope::Stage, "\"stage\""),
            (EventScope::Agent, "\"agent\""),
        ];
        for (scope, expected_json) in cases {
            let json = serde_json::to_string(&scope).unwrap();
            assert_eq!(json, expected_json);
            let back: EventScope = serde_json::from_str(&json).unwrap();
            assert_eq!(scope, back);
        }
    }

    // ── parse_step_limit_from_budget ──

    #[test]
    fn parse_step_limit_valid() {
        assert_eq!(parse_step_limit_from_budget(Some("step-limit:3")), Some(3));
        assert_eq!(
            parse_step_limit_from_budget(Some("step-limit:10")),
            Some(10)
        );
    }

    #[test]
    fn parse_step_limit_unbounded_or_missing() {
        assert_eq!(parse_step_limit_from_budget(Some("unbounded")), None);
        assert_eq!(parse_step_limit_from_budget(None), None);
        assert_eq!(parse_step_limit_from_budget(Some("")), None);
    }
}
