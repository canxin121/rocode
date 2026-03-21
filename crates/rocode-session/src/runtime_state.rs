//! Per-session aggregated runtime state.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRuntimeState {
    pub session_id: String,
    pub run_status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_reason: Option<PendingReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_message_id: Option<String>,
    pub active_tools: Vec<ActiveToolSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_question: Option<PendingQuestionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_permission: Option<PendingPermissionSummary>,
    pub child_sessions: Vec<ChildSessionSummary>,
}

impl SessionRuntimeState {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            run_status: RunStatus::Idle,
            pending_reason: None,
            error_message: None,
            current_message_id: None,
            active_tools: Vec::new(),
            pending_question: None,
            pending_permission: None,
            child_sessions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Idle,
    Running,
    WaitingOnTool,
    #[serde(alias = "waiting_on_user")]
    Pending,
    Cancelling,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRefStr, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PendingReason {
    Question,
    Permission,
    QuestionAndPermission,
}

impl Default for RunStatus {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveToolSummary {
    pub tool_call_id: String,
    pub tool_name: String,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingQuestionSummary {
    pub request_id: String,
    pub questions: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPermissionSummary {
    pub permission_id: String,
    pub info: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSessionSummary {
    pub child_id: String,
    pub parent_id: String,
}

#[derive(Debug)]
pub struct RuntimeStateStore {
    states: RwLock<HashMap<String, SessionRuntimeState>>,
}

impl RuntimeStateStore {
    fn derive_pending_reason(state: &SessionRuntimeState) -> Option<PendingReason> {
        match (
            state.pending_question.is_some(),
            state.pending_permission.is_some(),
        ) {
            (true, true) => Some(PendingReason::QuestionAndPermission),
            (true, false) => Some(PendingReason::Question),
            (false, true) => Some(PendingReason::Permission),
            (false, false) => None,
        }
    }

    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get(&self, session_id: &str) -> Option<SessionRuntimeState> {
        let guard = self.states.read().await;
        guard.get(session_id).cloned()
    }

    pub async fn update<F>(&self, session_id: &str, f: F)
    where
        F: FnOnce(&mut SessionRuntimeState),
    {
        let mut guard = self.states.write().await;
        let state = guard
            .entry(session_id.to_string())
            .or_insert_with(|| SessionRuntimeState::new(session_id));
        f(state);
    }

    pub async fn remove(&self, session_id: &str) {
        let mut guard = self.states.write().await;
        guard.remove(session_id);
    }

    pub async fn mark_running(&self, session_id: &str, message_id: Option<String>) {
        self.update(session_id, |s| {
            s.run_status = RunStatus::Running;
            s.pending_reason = None;
            s.error_message = None;
            s.current_message_id = message_id;
        })
        .await;
    }

    pub async fn mark_idle(&self, session_id: &str) {
        self.update(session_id, |s| {
            s.run_status = RunStatus::Idle;
            s.pending_reason = None;
            s.error_message = None;
            s.current_message_id = None;
            s.active_tools.clear();
            s.pending_question = None;
            s.pending_permission = None;
        })
        .await;
    }

    pub async fn tool_started(&self, session_id: &str, tool_call_id: &str, tool_name: &str) {
        self.update(session_id, |s| {
            s.run_status = RunStatus::WaitingOnTool;
            s.pending_reason = None;
            s.error_message = None;
            s.active_tools.push(ActiveToolSummary {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                started_at: chrono::Utc::now().timestamp_millis(),
            });
        })
        .await;
    }

    pub async fn tool_ended(&self, session_id: &str, tool_call_id: &str) {
        self.update(session_id, |s| {
            s.active_tools.retain(|t| t.tool_call_id != tool_call_id);
            if s.active_tools.is_empty() && s.run_status == RunStatus::WaitingOnTool {
                s.run_status = RunStatus::Running;
            }
        })
        .await;
    }

    pub async fn question_created(
        &self,
        session_id: &str,
        request_id: &str,
        questions: serde_json::Value,
    ) {
        self.update(session_id, |s| {
            s.pending_question = Some(PendingQuestionSummary {
                request_id: request_id.to_string(),
                questions,
            });
            s.pending_reason = Self::derive_pending_reason(s);
            s.run_status = RunStatus::Pending;
        })
        .await;
    }

    pub async fn question_resolved(&self, session_id: &str) {
        self.update(session_id, |s| {
            s.pending_question = None;
            s.pending_reason = Self::derive_pending_reason(s);
            s.run_status = if s.pending_reason.is_some() {
                RunStatus::Pending
            } else {
                RunStatus::Running
            };
        })
        .await;
    }

    pub async fn permission_requested(
        &self,
        session_id: &str,
        permission_id: &str,
        info: serde_json::Value,
    ) {
        self.update(session_id, |s| {
            s.pending_permission = Some(PendingPermissionSummary {
                permission_id: permission_id.to_string(),
                info,
            });
            s.pending_reason = Self::derive_pending_reason(s);
            s.run_status = RunStatus::Pending;
        })
        .await;
    }

    pub async fn permission_resolved(&self, session_id: &str) {
        self.update(session_id, |s| {
            s.pending_permission = None;
            s.pending_reason = Self::derive_pending_reason(s);
            s.run_status = if s.pending_reason.is_some() {
                RunStatus::Pending
            } else {
                RunStatus::Running
            };
        })
        .await;
    }

    pub async fn mark_pending(&self, session_id: &str, reason: String) {
        self.update(session_id, |s| {
            s.run_status = RunStatus::Pending;
            s.pending_reason = reason.parse().ok();
            s.error_message = None;
        })
        .await;
    }

    pub async fn mark_error(&self, session_id: &str, error_message: String) {
        self.update(session_id, |s| {
            s.run_status = RunStatus::Error;
            s.error_message = Some(error_message);
            s.pending_reason = None;
            s.current_message_id = None;
            s.active_tools.clear();
            s.pending_question = None;
            s.pending_permission = None;
        })
        .await;
    }

    pub async fn child_attached(&self, parent_id: &str, child_id: &str) {
        self.update(parent_id, |s| {
            if !s.child_sessions.iter().any(|c| c.child_id == child_id) {
                s.child_sessions.push(ChildSessionSummary {
                    child_id: child_id.to_string(),
                    parent_id: parent_id.to_string(),
                });
            }
        })
        .await;
    }

    pub async fn child_detached(&self, parent_id: &str, child_id: &str) {
        self.update(parent_id, |s| {
            s.child_sessions.retain(|c| c.child_id != child_id);
        })
        .await;
    }
}

impl Default for RuntimeStateStore {
    fn default() -> Self {
        Self::new()
    }
}
