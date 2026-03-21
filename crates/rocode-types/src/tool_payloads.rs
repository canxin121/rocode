use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

macro_rules! impl_lossy_from_value {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<&Value> for $ty {
                fn from(value: &Value) -> Self {
                    serde_json::from_value::<Self>(value.clone()).unwrap_or_default()
                }
            }

            impl From<Value> for $ty {
                fn from(value: Value) -> Self {
                    serde_json::from_value::<Self>(value).unwrap_or_default()
                }
            }
        )+
    };
}

macro_rules! impl_lossy_from_map {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<&HashMap<String, Value>> for $ty {
                fn from(metadata: &HashMap<String, Value>) -> Self {
                    serde_json::to_value(metadata)
                        .ok()
                        .and_then(|value| serde_json::from_value::<Self>(value).ok())
                        .unwrap_or_default()
                }
            }
        )+
    };
}

macro_rules! impl_lossy_from_json_str {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<&str> for $ty {
                fn from(text: &str) -> Self {
                    serde_json::from_str::<Self>(text).unwrap_or_default()
                }
            }

            impl From<String> for $ty {
                fn from(text: String) -> Self {
                    serde_json::from_str::<Self>(&text).unwrap_or_default()
                }
            }
        )+
    };
}

/// Deserialize an optional string from a JSON value, accepting string/number/bool.
///
/// - Trims whitespace for string values.
/// - Returns `None` for empty/whitespace-only strings.
pub fn deserialize_opt_string_lossy<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Some(Value::Number(value)) => Some(value.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    })
}

/// Deserialize an optional string from a JSON value, preserving whitespace and empties.
///
/// - Does **not** trim.
/// - Returns `Some("")` for empty strings.
/// - Coerces number/bool to string (for robustness).
pub fn deserialize_opt_string_preserve<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value),
        Some(Value::Number(value)) => Some(value.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    })
}

/// Deserialize an optional boolean from a JSON value, accepting bool/number/string.
///
/// Accepted string values (case-insensitive): true/false, 1/0, yes/no, on/off.
pub fn deserialize_opt_bool_lossy<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::Bool(value)) => Some(value),
        Some(Value::Number(value)) => Some(value.as_u64().unwrap_or(0) != 0),
        Some(Value::String(raw)) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "" => None,
                "true" | "1" | "yes" | "y" | "on" => Some(true),
                "false" | "0" | "no" | "n" | "off" => Some(false),
                _ => None,
            }
        }
        _ => None,
    })
}

/// Deserialize a boolean from a JSON value, accepting bool/number/string.
///
/// Defaults to `false` on missing/null/unrecognized values.
pub fn deserialize_bool_lossy<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => value,
        Some(Value::Number(value)) => value.as_u64().unwrap_or(0) != 0,
        Some(Value::String(raw)) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "" => false,
                "true" | "1" | "yes" | "y" | "on" => true,
                "false" | "0" | "no" | "n" | "off" => false,
                _ => false,
            }
        }
        _ => false,
    })
}

/// Deserialize an optional u64 from a JSON value, accepting number/string/bool.
pub fn deserialize_opt_u64_lossy<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::Bool(value)) => Some(u64::from(value)),
        Some(Value::String(raw)) => raw.trim().parse::<u64>().ok(),
        _ => None,
    })
}

/// Deserialize an optional i64 from a JSON value, accepting number/string/bool.
pub fn deserialize_opt_i64_lossy<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) => value.as_i64().or_else(|| value.as_u64().map(|v| v as i64)),
        Some(Value::Bool(value)) => Some(i64::from(value)),
        Some(Value::String(raw)) => raw.trim().parse::<i64>().ok(),
        _ => None,
    })
}

pub fn deserialize_vec_string_lossy<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(parse_vec_string_value_lossy(value.unwrap_or(Value::Null)))
}

fn parse_vec_string_value_lossy(value: Value) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::Array(values) => values
            .into_iter()
            .filter_map(|value| match value {
                Value::String(value) => {
                    let trimmed = value.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                }
                Value::Number(value) => Some(value.to_string()),
                Value::Bool(value) => Some(value.to_string()),
                _ => None,
            })
            .collect(),
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Vec::new();
            }
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .map(parse_vec_string_value_lossy)
                .unwrap_or_else(|| vec![trimmed.to_string()])
        }
        _ => Vec::new(),
    }
}

pub fn deserialize_opt_vec_string_lossy<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_vec_string_value_lossy(value)),
    })
}

pub fn deserialize_vec_value_lossy<'de, D>(deserializer: D) -> Result<Vec<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(parse_vec_value_lossy(value.unwrap_or(Value::Null)))
}

fn parse_vec_value_lossy(value: Value) -> Vec<Value> {
    match value {
        Value::Null => Vec::new(),
        Value::Array(values) => values,
        Value::Object(_) => vec![value],
        Value::String(raw) => serde_json::from_str::<Value>(&raw)
            .ok()
            .map(parse_vec_value_lossy)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub fn deserialize_questions_lossy<'de, D>(deserializer: D) -> Result<Vec<QuestionPrompt>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(parse_questions_value_lossy(value.unwrap_or(Value::Null)))
}

fn parse_questions_value_lossy(value: Value) -> Vec<QuestionPrompt> {
    match value {
        Value::Null => Vec::new(),
        Value::Array(values) => values
            .into_iter()
            .filter_map(|entry| serde_json::from_value::<QuestionPrompt>(entry).ok())
            .collect(),
        Value::Object(_) => serde_json::from_value::<QuestionPrompt>(value)
            .ok()
            .into_iter()
            .collect(),
        Value::String(raw) => serde_json::from_str::<Value>(&raw)
            .ok()
            .map(parse_questions_value_lossy)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisplayOverrideField {
    #[serde(default)]
    pub key: String,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub value: Option<String>,
}

fn deserialize_display_fields_lossy<'de, D>(
    deserializer: D,
) -> Result<Vec<DisplayOverrideField>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    let Some(Value::Array(values)) = value else {
        return Ok(Vec::new());
    };
    Ok(values
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<DisplayOverrideField>(entry).ok())
        .filter(|field| !field.key.trim().is_empty())
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisplayOverrideMetadata {
    #[serde(
        default,
        rename = "display.summary",
        deserialize_with = "deserialize_opt_string_lossy"
    )]
    pub summary: Option<String>,
    #[serde(
        default,
        rename = "display.mode",
        deserialize_with = "deserialize_opt_string_lossy"
    )]
    pub mode: Option<String>,
    #[serde(
        default,
        rename = "display.fields",
        deserialize_with = "deserialize_display_fields_lossy"
    )]
    pub fields: Vec<DisplayOverrideField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionToolInput {
    #[serde(default, deserialize_with = "deserialize_questions_lossy")]
    pub questions: Vec<QuestionPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionPrompt {
    #[serde(default)]
    pub question: String,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub header: Option<String>,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
    #[serde(default, deserialize_with = "deserialize_bool_lossy")]
    pub multiple: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionOption {
    #[serde(default)]
    pub label: String,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuestionToolResult {
    #[serde(default)]
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoWriteToolInput {
    #[serde(default)]
    pub todos: Vec<TodoWriteItem>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoWriteItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoListMetadata {
    #[serde(default)]
    pub todos: Vec<TodoListItem>,
    #[serde(default)]
    pub count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TodoListItem {
    #[serde(default)]
    pub content: String,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub prompt: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub subagent_type: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub category: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub task_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub command: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_vec_string_lossy")]
    pub load_skills: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_lossy")]
    pub run_in_background: Option<bool>,
}

impl TaskToolInput {
    pub fn dispatch_label(&self) -> Option<&str> {
        self.subagent_type
            .as_deref()
            .or(self.category.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskFlowToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub operation: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub task_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub agent: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub category: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub prompt: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub command: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_vec_string_lossy")]
    pub load_skills: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_lossy")]
    pub run_in_background: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_lossy")]
    pub sync_todo: Option<bool>,
    #[serde(default)]
    pub todo_item: Option<TaskFlowTodoItemInput>,
    #[serde(default, deserialize_with = "deserialize_opt_vec_string_lossy")]
    pub status_filter: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_opt_u64_lossy")]
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskFlowTodoItemInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilePathToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UrlToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LspToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub operation: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotebookEditToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub notebook_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub edit_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub pattern: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrepToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub pattern: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub glob: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_lossy")]
    pub ignore_case: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_lossy")]
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_preserve")]
    pub file_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_u64_lossy")]
    pub offset: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_opt_u64_lossy")]
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WriteToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub file_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_preserve")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub file_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_preserve")]
    pub old_string: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_preserve")]
    pub new_string: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_bool_lossy")]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BashToolInput {
    #[serde(default, deserialize_with = "deserialize_opt_string_preserve")]
    pub command: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_u64_lossy")]
    pub timeout: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub workdir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_preserve")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchToolInput {
    #[serde(default)]
    pub tool_calls: Vec<BatchToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchToolCall {
    #[serde(default, deserialize_with = "deserialize_opt_string_lossy")]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub parameters: Option<Value>,
}

impl_lossy_from_value!(
    DisplayOverrideMetadata,
    QuestionToolInput,
    TodoWriteToolInput,
    TodoListMetadata,
    TaskToolInput,
    TaskFlowToolInput,
    FilePathToolInput,
    PatternToolInput,
    CommandToolInput,
    UrlToolInput,
    QueryToolInput,
    SkillToolInput,
    LspToolInput,
    NotebookEditToolInput,
    GlobToolInput,
    GrepToolInput,
    ReadToolInput,
    WriteToolInput,
    EditToolInput,
    BashToolInput,
    BatchToolInput,
);

impl_lossy_from_map!(DisplayOverrideMetadata, TodoListMetadata,);

impl_lossy_from_json_str!(QuestionToolInput, QuestionToolResult,);
