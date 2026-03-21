use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PendingStatusReason {
    Question,
    Permission,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[derive(Default)]
pub enum SessionRunStatus {
    #[default]
    Idle,
    Busy,
    Pending {
        reason: PendingStatusReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Retry {
        attempt: u32,
        message: String,
        next: i64,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusInfo {
    pub status: String,
    pub idle: bool,
    pub busy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<i64>,
}

impl SessionRunStatus {
    pub fn to_info(&self, fallback_busy: bool) -> SessionStatusInfo {
        match self {
            SessionRunStatus::Idle if fallback_busy => SessionStatusInfo {
                status: "busy".to_string(),
                idle: false,
                busy: true,
                reason: None,
                attempt: None,
                message: None,
                next: None,
            },
            SessionRunStatus::Idle => SessionStatusInfo {
                status: "idle".to_string(),
                idle: true,
                busy: false,
                reason: None,
                attempt: None,
                message: None,
                next: None,
            },
            SessionRunStatus::Busy => SessionStatusInfo {
                status: "busy".to_string(),
                idle: false,
                busy: true,
                reason: None,
                attempt: None,
                message: None,
                next: None,
            },
            SessionRunStatus::Pending { reason, message } => SessionStatusInfo {
                status: "pending".to_string(),
                idle: false,
                busy: true,
                reason: Some(reason.as_ref().to_string()),
                attempt: None,
                message: message.clone(),
                next: None,
            },
            SessionRunStatus::Retry {
                attempt,
                message,
                next,
            } => SessionStatusInfo {
                status: "retry".to_string(),
                idle: false,
                busy: true,
                reason: None,
                attempt: Some(*attempt),
                message: Some(message.clone()),
                next: Some(*next),
            },
            SessionRunStatus::Error { message } => SessionStatusInfo {
                status: "error".to_string(),
                idle: false,
                busy: false,
                reason: None,
                attempt: None,
                message: Some(message.clone()),
                next: None,
            },
        }
    }
}
