use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::bus::{Bus, BusEventDef};
use crate::contracts::wire;

use super::run_status::{PendingStatusReason, SessionRunStatus};

pub static SESSION_STATUS_EVENT: BusEventDef = BusEventDef::new("session.status");

pub static SESSION_IDLE_EVENT: BusEventDef = BusEventDef::new("session.idle");

pub type SessionStatusInfo = SessionRunStatus;

pub struct SessionStatusManager {
    state: Arc<RwLock<HashMap<String, SessionStatusInfo>>>,
    bus: Option<Arc<Bus>>,
}

impl SessionStatusManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            bus: None,
        }
    }

    pub fn with_bus(bus: Arc<Bus>) -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            bus: Some(bus),
        }
    }

    pub async fn get(&self, session_id: &str) -> SessionStatusInfo {
        let state = self.state.read().await;
        state.get(session_id).cloned().unwrap_or_default()
    }

    pub async fn list(&self) -> HashMap<String, SessionStatusInfo> {
        let state = self.state.read().await;
        state.clone()
    }

    pub async fn set(&self, session_id: &str, status: SessionStatusInfo) {
        if let Some(ref bus) = self.bus {
            let mut event_data = serde_json::Map::new();
            event_data.insert(
                wire::keys::SESSION_ID.to_string(),
                serde_json::Value::String(session_id.to_string()),
            );
            event_data.insert(
                "status".to_string(),
                serde_json::to_value(&status).unwrap_or(serde_json::Value::Null),
            );
            bus.publish(&SESSION_STATUS_EVENT, serde_json::Value::Object(event_data))
                .await;
        }

        let mut state = self.state.write().await;
        match &status {
            SessionStatusInfo::Idle => {
                if let Some(ref bus) = self.bus {
                    let mut idle_data = serde_json::Map::new();
                    idle_data.insert(
                        wire::keys::SESSION_ID.to_string(),
                        serde_json::Value::String(session_id.to_string()),
                    );
                    bus.publish(&SESSION_IDLE_EVENT, serde_json::Value::Object(idle_data))
                        .await;
                }
                state.remove(session_id);
            }
            _ => {
                state.insert(session_id.to_string(), status);
            }
        }
    }

    pub async fn set_idle(&self, session_id: &str) {
        self.set(session_id, SessionStatusInfo::Idle).await;
    }

    pub async fn set_busy(&self, session_id: &str) {
        self.set(session_id, SessionStatusInfo::Busy).await;
    }

    pub async fn set_retry(&self, session_id: &str, attempt: u32, message: String, next: u64) {
        self.set(
            session_id,
            SessionStatusInfo::Retry {
                attempt,
                message,
                next: i64::try_from(next).unwrap_or(i64::MAX),
            },
        )
        .await;
    }

    pub async fn set_pending(
        &self,
        session_id: &str,
        reason: Option<String>,
        message: Option<String>,
    ) {
        let reason = reason
            .as_deref()
            .and_then(|value| PendingStatusReason::from_str(value).ok())
            .unwrap_or(PendingStatusReason::Question);
        self.set(session_id, SessionStatusInfo::Pending { reason, message })
            .await;
    }

    pub async fn set_error(&self, session_id: &str, message: String) {
        self.set(session_id, SessionStatusInfo::Error { message })
            .await;
    }

    pub async fn is_busy(&self, session_id: &str) -> bool {
        let state = self.state.read().await;
        matches!(
            state.get(session_id),
            Some(
                SessionStatusInfo::Busy
                    | SessionStatusInfo::Pending { .. }
                    | SessionStatusInfo::Retry { .. }
            )
        )
    }
}

impl Default for SessionStatusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_is_idle() {
        let mgr = SessionStatusManager::new();
        let status = mgr.get("ses_123").await;
        assert!(matches!(status, SessionStatusInfo::Idle));
    }

    #[tokio::test]
    async fn test_set_busy() {
        let mgr = SessionStatusManager::new();
        mgr.set_busy("ses_123").await;
        assert!(mgr.is_busy("ses_123").await);
    }

    #[tokio::test]
    async fn test_set_idle_removes() {
        let mgr = SessionStatusManager::new();
        mgr.set_busy("ses_123").await;
        mgr.set_idle("ses_123").await;
        assert!(!mgr.is_busy("ses_123").await);
        let list = mgr.list().await;
        assert!(!list.contains_key("ses_123"));
    }

    #[tokio::test]
    async fn test_set_retry() {
        let mgr = SessionStatusManager::new();
        mgr.set_retry("ses_123", 2, "Rate limited".to_string(), 1700000000000)
            .await;
        let status = mgr.get("ses_123").await;
        match status {
            SessionStatusInfo::Retry {
                attempt,
                message,
                next,
            } => {
                assert_eq!(attempt, 2);
                assert_eq!(message, "Rate limited");
                assert_eq!(next, 1700000000000);
            }
            _ => panic!("Expected Retry status"),
        }
        assert!(mgr.is_busy("ses_123").await);
    }

    #[tokio::test]
    async fn test_set_pending() {
        let mgr = SessionStatusManager::new();
        mgr.set_pending(
            "ses_123",
            Some("question".to_string()),
            Some("waiting user reply".to_string()),
        )
        .await;

        let status = mgr.get("ses_123").await;
        match status {
            SessionStatusInfo::Pending { reason, message } => {
                assert_eq!(reason, PendingStatusReason::Question);
                assert_eq!(message.as_deref(), Some("waiting user reply"));
            }
            _ => panic!("Expected Pending status"),
        }
        assert!(mgr.is_busy("ses_123").await);
    }

    #[tokio::test]
    async fn test_set_error_not_busy() {
        let mgr = SessionStatusManager::new();
        mgr.set_error("ses_123", "provider timeout".to_string())
            .await;

        let status = mgr.get("ses_123").await;
        match status {
            SessionStatusInfo::Error { message } => {
                assert_eq!(message, "provider timeout");
            }
            _ => panic!("Expected Error status"),
        }
        assert!(!mgr.is_busy("ses_123").await);
    }

    #[tokio::test]
    async fn test_list() {
        let mgr = SessionStatusManager::new();
        mgr.set_busy("ses_1").await;
        mgr.set_busy("ses_2").await;
        let list = mgr.list().await;
        assert_eq!(list.len(), 2);
        assert!(list.contains_key("ses_1"));
        assert!(list.contains_key("ses_2"));
    }

    #[tokio::test]
    async fn test_with_bus() {
        let bus = Arc::new(Bus::new());
        let mgr = SessionStatusManager::with_bus(bus);
        mgr.set_busy("ses_123").await;
        assert!(mgr.is_busy("ses_123").await);
    }
}
