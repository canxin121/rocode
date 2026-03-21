use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOptionInfo {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionItemInfo {
    pub question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default)]
    pub options: Vec<QuestionOptionInfo>,
    #[serde(default)]
    pub multiple: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionInfo {
    pub id: String,
    #[serde(alias = "sessionID", alias = "sessionId")]
    pub session_id: String,
    /// Legacy: flat question strings (kept for backward compat).
    #[serde(default)]
    pub questions: Vec<String>,
    /// Legacy: flat option labels per question (kept for backward compat).
    #[serde(default)]
    pub options: Option<Vec<Vec<String>>>,
    /// Full-fidelity question items with descriptions, headers, multi-select.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<QuestionItemInfo>,
}
