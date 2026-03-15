use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub parent_id: Option<String>,
    pub share: Option<ShareInfo>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareInfo {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub finish: Option<String>,
    pub error: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cost: f64,
    pub tokens: TokenUsage,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub parts: Vec<MessagePart>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessagePart {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
    },
    File {
        path: String,
        mime: String,
    },
    Image {
        url: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        id: String,
        result: String,
        is_error: bool,
        title: Option<String>,
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct SessionContext {
    pub sessions: HashMap<String, Session>,
    pub messages: HashMap<String, Vec<Message>>,
    pub message_index: HashMap<String, HashMap<String, usize>>,
    pub current_session_id: Option<String>,
    pub session_status: HashMap<String, SessionStatus>,
    pub session_diff: HashMap<String, Vec<DiffEntry>>,
    pub todos: HashMap<String, Vec<TodoItem>>,
    pub revert: HashMap<String, RevertInfo>,
}

#[derive(Clone, Debug, Default)]
pub enum SessionStatus {
    #[default]
    Idle,
    Running,
    Retrying {
        message: String,
        attempt: u32,
        next: i64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffEntry {
    pub file: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevertInfo {
    pub message_id: String,
    pub part_id: Option<String>,
    pub snapshot: Option<String>,
    pub diff: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct ChildSessionInfo {
    pub session_id: String,
    pub stage_name: String,
    pub stage_title: String,
    pub stage_id: Option<String>,
    pub stage_index: Option<u64>,
    pub stage_total: Option<u64>,
    pub status: String,
}

pub fn collect_child_sessions(messages: &[Message]) -> Vec<ChildSessionInfo> {
    let mut seen = HashMap::new();
    for msg in messages {
        let meta = match msg.metadata.as_ref() {
            Some(m) => m,
            None => continue,
        };
        let child_id = match meta
            .get("scheduler_stage_child_session_id")
            .and_then(|v| v.as_str())
        {
            Some(id) => id.to_string(),
            None => continue,
        };
        let stage_name = meta
            .get("scheduler_stage")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let stage_title = meta
            .get("scheduler_stage_title")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| stage_name.clone());
        let stage_index = meta.get("scheduler_stage_index").and_then(|v| v.as_u64());
        let stage_total = meta.get("scheduler_stage_total").and_then(|v| v.as_u64());
        let status = meta
            .get("scheduler_stage_status")
            .and_then(|v| v.as_str())
            .unwrap_or("running")
            .to_string();
        let stage_id = meta
            .get("stage_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let info = ChildSessionInfo {
            session_id: child_id.clone(),
            stage_name,
            stage_title,
            stage_id,
            stage_index,
            stage_total,
            status,
        };
        seen.insert(child_id, info);
    }

    let mut result: Vec<ChildSessionInfo> = seen.into_values().collect();
    result.sort_by(|a, b| {
        a.stage_index
            .unwrap_or(u64::MAX)
            .cmp(&b.stage_index.unwrap_or(u64::MAX))
    });
    result
}

impl SessionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_session(&self) -> Option<&Session> {
        self.current_session_id
            .as_ref()
            .and_then(|id| self.sessions.get(id))
    }

    pub fn current_messages(&self) -> Vec<&Message> {
        self.current_session_id
            .as_ref()
            .and_then(|id| self.messages.get(id))
            .map(|m| m.iter().collect())
            .unwrap_or_default()
    }

    pub fn create_session(&mut self, title: Option<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let session = Session {
            id: id.clone(),
            title: title.unwrap_or_else(|| "New Session".to_string()),
            created_at: now,
            updated_at: now,
            parent_id: None,
            share: None,
            metadata: None,
        };
        self.sessions.insert(id.clone(), session);
        self.messages.insert(id.clone(), Vec::new());
        self.message_index.insert(id.clone(), HashMap::new());
        self.session_status.insert(id.clone(), SessionStatus::Idle);
        self.current_session_id = Some(id.clone());
        id
    }

    pub fn upsert_session(&mut self, session: Session) {
        let id = session.id.clone();
        self.sessions.insert(id.clone(), session);
        self.messages.entry(id.clone()).or_default();
        self.message_index.entry(id.clone()).or_default();
        self.session_status
            .entry(id.clone())
            .or_insert(SessionStatus::Idle);
        // Only set current_session_id if no session is active yet.
        // Callers that want to switch sessions should use set_current_session_id().
        if self.current_session_id.is_none() {
            self.current_session_id = Some(id);
        }
    }

    /// Explicitly switch the current session to the given id.
    pub fn set_current_session_id(&mut self, id: String) {
        self.current_session_id = Some(id);
    }

    pub fn set_messages(&mut self, session_id: &str, messages: Vec<Message>) {
        let mut index = HashMap::with_capacity(messages.len());
        for (pos, message) in messages.iter().enumerate() {
            index.insert(message.id.clone(), pos);
        }
        self.messages.insert(session_id.to_string(), messages);
        self.message_index.insert(session_id.to_string(), index);
    }

    pub fn add_message(&mut self, session_id: &str, message: Message) {
        self.upsert_message(session_id, message);
    }

    pub fn upsert_messages_incremental(&mut self, session_id: &str, incoming: Vec<Message>) {
        for message in incoming {
            self.upsert_message(session_id, message);
        }
    }

    pub fn upsert_message(&mut self, session_id: &str, message: Message) {
        let messages = self.messages.entry(session_id.to_string()).or_default();
        let index = self
            .message_index
            .entry(session_id.to_string())
            .or_default();
        if let Some(existing_pos) = index.get(&message.id).copied() {
            if existing_pos < messages.len() {
                messages[existing_pos] = message;
                return;
            }
            // Index drift should be rare; rebuild once to recover.
            index.clear();
            for (pos, msg) in messages.iter().enumerate() {
                index.insert(msg.id.clone(), pos);
            }
        }
        let message_id = message.id.clone();
        messages.push(message);
        index.insert(message_id, messages.len().saturating_sub(1));
    }

    pub fn set_status(&mut self, session_id: &str, status: SessionStatus) {
        self.session_status.insert(session_id.to_string(), status);
    }

    pub fn status(&self, session_id: &str) -> &SessionStatus {
        self.session_status
            .get(session_id)
            .unwrap_or(&SessionStatus::Idle)
    }

    /// Incrementally update reasoning content for a message during streaming.
    /// This allows real-time display of thinking content before the message is complete.
    pub fn update_reasoning_incremental(
        &mut self,
        session_id: &str,
        message_id: &str,
        phase: &str,
        text: &str,
    ) {
        if message_id.is_empty() {
            return;
        }
        let messages = self.messages.entry(session_id.to_string()).or_default();
        let index = self
            .message_index
            .entry(session_id.to_string())
            .or_default();

        // If the message doesn't exist yet (streaming hasn't synced it),
        // create a placeholder assistant message so reasoning can accumulate.
        if !index.contains_key(message_id) {
            let pos = messages.len();
            messages.push(Message {
                id: message_id.to_string(),
                role: MessageRole::Assistant,
                content: String::new(),
                created_at: chrono::Utc::now(),
                agent: None,
                model: None,
                mode: None,
                finish: None,
                error: None,
                completed_at: None,
                cost: 0.0,
                tokens: TokenUsage::default(),
                metadata: None,
                parts: Vec::new(),
            });
            index.insert(message_id.to_string(), pos);
        }

        let Some(&pos) = index.get(message_id) else {
            return;
        };
        let Some(message) = messages.get_mut(pos) else {
            return;
        };

        // Find or create a Reasoning part
        match phase {
            "start" => {
                // Initialize or reset reasoning content
                // Check if there's already a Reasoning part
                let has_reasoning = message
                    .parts
                    .iter()
                    .any(|p| matches!(p, MessagePart::Reasoning { .. }));
                if !has_reasoning {
                    message.parts.push(MessagePart::Reasoning {
                        text: String::new(),
                    });
                }
            }
            "delta" => {
                // Append reasoning text
                for part in &mut message.parts {
                    if let MessagePart::Reasoning {
                        text: ref mut existing,
                    } = part
                    {
                        existing.push_str(text);
                        break;
                    }
                }
            }
            "end" => {
                // Reasoning complete - nothing special to do, the text is already there
            }
            _ => {}
        }
    }
}
