use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

use rocode_session::message_model::{
    MessageWithParts as UnifiedMessageWithParts, Part as UnifiedPart,
};
use rocode_session::{MessagePart, PartType};
use serde_json::Value;
use tracing::{info, warn};

pub struct Migration;

enum StoredMessageData {
    Legacy(Vec<MessagePart>),
    UnifiedParts(Vec<UnifiedPart>),
    UnifiedMessage(UnifiedMessageWithParts),
}

fn parse_stored_message_data(data: &str) -> Option<StoredMessageData> {
    if let Ok(parts) = serde_json::from_str::<Vec<MessagePart>>(data) {
        return Some(StoredMessageData::Legacy(parts));
    }
    if let Ok(parts) = serde_json::from_str::<Vec<UnifiedPart>>(data) {
        return Some(StoredMessageData::UnifiedParts(parts));
    }
    if let Ok(message) = serde_json::from_str::<UnifiedMessageWithParts>(data) {
        return Some(StoredMessageData::UnifiedMessage(message));
    }
    None
}

fn sanitize_unified_part_input(part: &mut UnifiedPart) -> (bool, bool, bool) {
    let UnifiedPart::Tool(tool) = part else {
        return (false, false, false);
    };

    let input = match &mut tool.state {
        rocode_session::message_model::ToolState::Pending { input, .. }
        | rocode_session::message_model::ToolState::Running { input, .. }
        | rocode_session::message_model::ToolState::Completed { input, .. }
        | rocode_session::message_model::ToolState::Error { input, .. } => input,
    };

    let (sanitized, was_recovered, rerouted_invalid) =
        sanitize_tool_call_input_for_storage(&tool.tool, input);
    if *input == sanitized {
        return (false, was_recovered, rerouted_invalid);
    }

    *input = sanitized;
    (true, was_recovered, rerouted_invalid)
}

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260317_000009_migrate_tool_call_input_data"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let backend = DbBackend::Sqlite;

        // `messages.data` stores a JSON array of parts. Historical clients could store
        // unrecoverable tool args sentinels; this migration sanitizes them into a
        // stable payload so downstream code can rely on the shape.
        let select_stmt = Statement::from_sql_and_values(
            backend,
            "SELECT id, data FROM messages WHERE role = 'assistant' AND data IS NOT NULL"
                .to_string(),
            vec![],
        );
        let rows = conn.query_all(select_stmt).await?;

        let mut updated_rows = 0usize;
        let mut recovered_inputs = 0usize;
        let mut invalid_reroutes = 0usize;

        for row in rows {
            let id: String = row.try_get("", "id")?;
            let data: String = row.try_get("", "data")?;
            let mut changed = false;
            let mut parsed = match parse_stored_message_data(&data) {
                Some(parsed) => parsed,
                None => {
                    warn!(
                        message_id = %id,
                        "skipping tool-call input migration for message with unsupported parts JSON"
                    );
                    continue;
                }
            };

            match &mut parsed {
                StoredMessageData::Legacy(parts) => {
                    for part in parts {
                        if let PartType::ToolCall { name, input, .. } = &mut part.part_type {
                            let (sanitized, was_recovered, rerouted_invalid) =
                                sanitize_tool_call_input_for_storage(name, input);
                            if *input != sanitized {
                                *input = sanitized;
                                changed = true;
                            }
                            if was_recovered {
                                recovered_inputs += 1;
                            }
                            if rerouted_invalid {
                                invalid_reroutes += 1;
                            }
                        }
                    }
                }
                StoredMessageData::UnifiedParts(parts) => {
                    for part in parts {
                        let (was_changed, was_recovered, rerouted_invalid) =
                            sanitize_unified_part_input(part);
                        if was_changed {
                            changed = true;
                        }
                        if was_recovered {
                            recovered_inputs += 1;
                        }
                        if rerouted_invalid {
                            invalid_reroutes += 1;
                        }
                    }
                }
                StoredMessageData::UnifiedMessage(message) => {
                    for part in &mut message.parts {
                        let (was_changed, was_recovered, rerouted_invalid) =
                            sanitize_unified_part_input(part);
                        if was_changed {
                            changed = true;
                        }
                        if was_recovered {
                            recovered_inputs += 1;
                        }
                        if rerouted_invalid {
                            invalid_reroutes += 1;
                        }
                    }
                }
            }

            if !changed {
                continue;
            }

            let next_data = match parsed {
                StoredMessageData::Legacy(parts) => {
                    serde_json::to_string(&parts).map_err(|e| DbErr::Custom(e.to_string()))?
                }
                StoredMessageData::UnifiedParts(parts) => {
                    serde_json::to_string(&parts).map_err(|e| DbErr::Custom(e.to_string()))?
                }
                StoredMessageData::UnifiedMessage(message) => {
                    serde_json::to_string(&message).map_err(|e| DbErr::Custom(e.to_string()))?
                }
            };
            let update_stmt = Statement::from_sql_and_values(
                backend,
                "UPDATE messages SET data = ? WHERE id = ?".to_string(),
                vec![next_data.into(), id.clone().into()],
            );
            conn.execute(update_stmt).await?;
            updated_rows += 1;
        }

        if updated_rows > 0 || recovered_inputs > 0 || invalid_reroutes > 0 {
            info!(
                updated_rows,
                recovered_inputs, invalid_reroutes, "tool call input data migration complete"
            );
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

fn invalid_tool_payload_for_storage(tool_name: &str, error: &str, received_args: Value) -> Value {
    serde_json::json!({
        "tool": tool_name,
        "error": error,
        "receivedArgs": received_args,
        "source": "storage-migration",
    })
}

fn sanitize_tool_call_input_for_storage(tool_name: &str, input: &Value) -> (Value, bool, bool) {
    if let Some(obj) = input.as_object() {
        let is_legacy_unrecoverable = obj
            .get("_rocode_unrecoverable_tool_args")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !is_legacy_unrecoverable {
            return (input.clone(), false, false);
        }

        let received_args = serde_json::json!({
            "type": "object",
            "source": "legacy-unrecoverable-sentinel",
            "raw_len": obj.get("raw_len").and_then(Value::as_u64),
            "preview": obj.get("raw_preview").and_then(Value::as_str),
        });
        return (
            invalid_tool_payload_for_storage(
                tool_name,
                "Stored tool arguments were previously marked unrecoverable.",
                received_args,
            ),
            false,
            true,
        );
    }

    if let Some(raw) = input.as_str() {
        if let Some(parsed) = rocode_util::json::try_parse_json_object_robust(raw) {
            return (parsed, true, false);
        }
        if let Some(recovered) =
            rocode_util::json::recover_tool_arguments_from_jsonish(tool_name, raw)
        {
            return (recovered, true, false);
        }

        return (
            invalid_tool_payload_for_storage(
                tool_name,
                "Stored tool arguments are malformed/truncated and cannot be replayed safely.",
                serde_json::json!({
                    "type": "string",
                    "raw_len": raw.len(),
                    "preview": raw.chars().take(240).collect::<String>(),
                }),
            ),
            false,
            true,
        );
    }

    let input_type = match input {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::String(_) => "string",
    };

    (
        invalid_tool_payload_for_storage(
            tool_name,
            "Stored tool arguments are non-object and cannot be replayed safely.",
            serde_json::json!({
                "type": input_type,
            }),
        ),
        false,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        parse_stored_message_data, sanitize_tool_call_input_for_storage, StoredMessageData,
    };

    #[test]
    fn sanitize_tool_call_input_for_storage_recovers_jsonish() {
        let raw = serde_json::Value::String(
            "{\"file_path\":\"t2.html\",\"content\":\"<!DOCTYPE html>".to_string(),
        );
        let (sanitized, recovered, rerouted_invalid) =
            sanitize_tool_call_input_for_storage("write", &raw);
        assert!(sanitized.is_object());
        assert!(recovered);
        assert!(!rerouted_invalid);
        assert_eq!(sanitized["file_path"], "t2.html");
    }

    #[test]
    fn sanitize_tool_call_input_for_storage_routes_unrecoverable_to_invalid_payload() {
        let raw = serde_json::Value::String("not-json".to_string());
        let (sanitized, recovered, rerouted_invalid) =
            sanitize_tool_call_input_for_storage("write", &raw);
        assert!(sanitized.is_object());
        assert!(!recovered);
        assert!(rerouted_invalid);
        assert_eq!(sanitized["tool"], "write");
        assert_eq!(sanitized["receivedArgs"]["type"], "string");
        assert!(sanitized["error"]
            .as_str()
            .unwrap_or_default()
            .contains("malformed/truncated"));
    }

    #[test]
    fn sanitize_tool_call_input_for_storage_rewrites_legacy_sentinel_object() {
        let raw = serde_json::json!({
            "_rocode_unrecoverable_tool_args": true,
            "raw_len": 42,
            "raw_preview": "{\"content\":\"<html>"
        });
        let (sanitized, recovered, rerouted_invalid) =
            sanitize_tool_call_input_for_storage("write", &raw);
        assert!(sanitized.is_object());
        assert!(!recovered);
        assert!(rerouted_invalid);
        assert_eq!(sanitized["tool"], "write");
        assert_eq!(
            sanitized["receivedArgs"]["source"],
            "legacy-unrecoverable-sentinel"
        );
    }

    #[test]
    fn parse_stored_message_data_supports_unified_parts_array() {
        let raw = serde_json::json!([
            {
                "type": "tool",
                "id": "prt_1",
                "session_id": "1",
                "message_id": "2",
                "call_id": "call_1",
                "tool": "bash",
                "state": {
                    "status": "pending",
                    "input": {"command": "ls"},
                    "raw": "{\"command\":\"ls\"}"
                }
            }
        ])
        .to_string();

        let parsed = parse_stored_message_data(&raw);
        assert!(matches!(parsed, Some(StoredMessageData::UnifiedParts(_))));
    }
}
