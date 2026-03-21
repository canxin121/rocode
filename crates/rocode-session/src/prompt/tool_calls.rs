use std::collections::HashMap;

use serde::Deserialize;

use crate::{PartKind, PartType, Session, SessionMessage};
use rocode_message::message::{
    canonical_tool_state_to_message, session_message_to_unified_message, tool_state_to_canonical,
    unified_parts_to_session, CompletedTime as ModelCompletedTime, ErrorTime as ModelErrorTime,
    Part as ModelPart, RunningTime as ModelRunningTime, ToolPart as ModelToolPart,
    ToolState as ModelToolState,
};

use super::SessionPrompt;

impl SessionPrompt {
    pub(super) fn upsert_tool_call_part(
        message: &mut SessionMessage,
        tool_call_id: &str,
        tool_name: Option<&str>,
        input: Option<serde_json::Value>,
        raw_input: Option<String>,
        status: Option<crate::ToolCallStatus>,
        tool_state: Option<crate::ToolState>,
    ) {
        let mut input = input;
        let mut raw_input = raw_input;
        let mut status = status;
        if let Some(state) = tool_state.as_ref() {
            let (state_input, state_raw, state_status) = Self::state_projection(state);
            input = Some(state_input);
            // Only override raw_input if state_projection returns Some.
            // Running/Completed/Error states return None for raw, but the
            // caller may have explicitly provided a raw value that should
            // be preserved.
            if state_raw.is_some() {
                raw_input = state_raw;
            }
            status = Some(state_status);
        }
        let created_at = chrono::Utc::now();
        let created_ms = created_at.timestamp_millis();
        let explicit_name = tool_name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToString::to_string);
        let has_state_update =
            tool_state.is_some() || input.is_some() || raw_input.is_some() || status.is_some();

        for part in &mut message.parts {
            let PartType::ToolCall {
                id,
                name,
                input: current_input,
                status: current_status,
                raw: current_raw,
                state: current_state,
            } = &mut part.part_type
            else {
                continue;
            };
            if id != tool_call_id {
                continue;
            }

            if let Some(explicit_tool_name) = explicit_name.as_ref() {
                *name = explicit_tool_name.clone();
            }

            if has_state_update {
                let next_state = if let Some(next_state) = tool_state.as_ref() {
                    next_state.clone()
                } else if let Some(existing_state) = current_state.as_ref() {
                    if input.is_some() || raw_input.is_some() || status.is_some() {
                        tracing::debug!(
                            tool_call_id,
                            "ignoring legacy tool-call field update because canonical state exists"
                        );
                    }
                    canonical_tool_state_to_message(existing_state)
                } else {
                    let (baseline_input, baseline_raw, baseline_status) = current_state
                        .as_ref()
                        .map(|state| {
                            (
                                state.input().clone(),
                                state.raw().map(ToString::to_string),
                                state.status(),
                            )
                        })
                        .unwrap_or_else(|| {
                            (current_input.clone(), current_raw.clone(), *current_status)
                        });
                    Self::synthesize_tool_state(
                        status.unwrap_or(baseline_status),
                        input.clone().unwrap_or(baseline_input),
                        raw_input.clone().or(baseline_raw),
                        created_ms,
                        name,
                    )
                };

                let (state_input, state_raw, state_status) = Self::state_projection(&next_state);
                *current_input = state_input;
                *current_raw = state_raw;
                *current_status = state_status;
                *current_state = Some(tool_state_to_canonical(&next_state));
            }
            return;
        }

        let resolved_input = input.unwrap_or_else(|| serde_json::json!({}));
        let resolved_status = status.unwrap_or(crate::ToolCallStatus::Pending);
        let resolved_name = explicit_name.unwrap_or_default();
        let resolved_state = tool_state.unwrap_or_else(|| {
            Self::synthesize_tool_state(
                resolved_status,
                resolved_input.clone(),
                raw_input.clone(),
                created_ms,
                &resolved_name,
            )
        });

        let model_tool_part = ModelToolPart {
            id: rocode_core::id::create(rocode_core::id::Prefix::Part, true, None),
            session_id: message.session_id.clone(),
            message_id: message.id.clone(),
            call_id: tool_call_id.to_string(),
            tool: resolved_name.clone(),
            state: resolved_state.clone(),
            metadata: None,
        };

        if let Some(part) = Self::tool_part_to_session_part(
            model_tool_part,
            created_at,
            &message.id,
            PartKind::ToolCall,
        ) {
            message.parts.push(part);
        } else {
            tracing::warn!(
                tool_call_id = tool_call_id,
                "failed to append converted tool call part"
            );
        }
    }

    pub(super) fn state_projection(
        state: &crate::ToolState,
    ) -> (serde_json::Value, Option<String>, crate::ToolCallStatus) {
        (
            state.input().clone(),
            state.raw().map(ToString::to_string),
            state.status(),
        )
    }

    fn synthesize_tool_state(
        status: crate::ToolCallStatus,
        input: serde_json::Value,
        raw_input: Option<String>,
        created_ms: i64,
        tool_name: &str,
    ) -> ModelToolState {
        match status {
            crate::ToolCallStatus::Pending => ModelToolState::Pending {
                input: input.clone(),
                raw: raw_input.unwrap_or_else(|| serde_json::to_string(&input).unwrap_or_default()),
            },
            crate::ToolCallStatus::Running => ModelToolState::Running {
                input,
                title: None,
                metadata: None,
                time: ModelRunningTime { start: created_ms },
            },
            crate::ToolCallStatus::Completed => ModelToolState::Completed {
                input,
                output: String::new(),
                title: if tool_name.trim().is_empty() {
                    "tool".to_string()
                } else {
                    tool_name.to_string()
                },
                metadata: HashMap::new(),
                time: ModelCompletedTime {
                    start: created_ms,
                    end: created_ms,
                    compacted: None,
                },
                attachments: None,
            },
            crate::ToolCallStatus::Error => ModelToolState::Error {
                input,
                error: "Tool execution failed".to_string(),
                metadata: None,
                time: ModelErrorTime {
                    start: created_ms,
                    end: created_ms,
                },
            },
        }
    }

    fn tool_part_to_session_part(
        tool_part: ModelToolPart,
        created_at: chrono::DateTime<chrono::Utc>,
        message_id: &str,
        expected_kind: PartKind,
    ) -> Option<crate::MessagePart> {
        let mut converted =
            unified_parts_to_session(vec![ModelPart::Tool(tool_part)], created_at, message_id);
        let idx = converted
            .iter()
            .position(|part| part.kind() == expected_kind)?;
        let mut part = converted.swap_remove(idx);
        part.message_id = Some(message_id.to_string());
        Some(part)
    }

    pub(super) fn push_tool_result_part(
        message: &mut SessionMessage,
        tool_call_id: String,
        content: String,
        is_error: bool,
        title: Option<String>,
        metadata: Option<HashMap<String, serde_json::Value>>,
        attachments: Option<Vec<serde_json::Value>>,
    ) {
        let created_at = chrono::Utc::now();
        let created_ms = created_at.timestamp_millis();
        let resolved_title = title.clone().unwrap_or_else(|| {
            if is_error {
                "Tool Error".to_string()
            } else {
                "Tool Result".to_string()
            }
        });
        let parsed_attachments = attachments
            .as_ref()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| {
                        serde_json::from_value::<crate::message_model::FilePart>(value.clone()).ok()
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty());

        let state = if is_error {
            ModelToolState::Error {
                input: serde_json::json!({}),
                error: content.clone(),
                metadata: metadata.clone(),
                time: ModelErrorTime {
                    start: created_ms,
                    end: created_ms,
                },
            }
        } else {
            ModelToolState::Completed {
                input: serde_json::json!({}),
                output: content.clone(),
                title: resolved_title.clone(),
                metadata: metadata.clone().unwrap_or_default(),
                time: ModelCompletedTime {
                    start: created_ms,
                    end: created_ms,
                    compacted: None,
                },
                attachments: parsed_attachments,
            }
        };

        let model_tool_part = ModelToolPart {
            id: rocode_core::id::create(rocode_core::id::Prefix::Part, true, None),
            session_id: message.session_id.clone(),
            message_id: message.id.clone(),
            call_id: tool_call_id.clone(),
            tool: resolved_title,
            state,
            metadata: None,
        };
        if let Some(part) = Self::tool_part_to_session_part(
            model_tool_part,
            created_at,
            &message.id,
            PartKind::ToolResult,
        ) {
            message.parts.push(part);
        } else {
            tracing::warn!(
                tool_call_id = tool_call_id,
                "failed to append converted tool result part"
            );
        }
    }

    pub(super) fn take_attachment_values(
        metadata: &mut HashMap<String, serde_json::Value>,
    ) -> Option<Vec<serde_json::Value>> {
        let mut attachments = Vec::new();

        if let Some(value) = metadata.remove("attachments") {
            match value {
                serde_json::Value::Array(values) => attachments.extend(values),
                serde_json::Value::Null => {}
                other => attachments.push(other),
            }
        }

        if let Some(value) = metadata.remove("attachment") {
            if !value.is_null() {
                attachments.push(value);
            }
        }

        if attachments.is_empty() {
            None
        } else {
            Some(attachments)
        }
    }

    pub(super) fn normalize_tool_attachments(
        raw_attachments: Option<Vec<serde_json::Value>>,
        session_id: &str,
        message_id: &str,
    ) -> (
        Option<Vec<serde_json::Value>>,
        Option<Vec<crate::message_model::FilePart>>,
    ) {
        #[derive(Debug, Deserialize)]
        struct AttachmentWire {
            mime: String,
            url: String,
            #[serde(default)]
            filename: Option<String>,
            #[serde(default)]
            id: Option<String>,
            #[serde(
                default,
                rename = "sessionID",
                alias = "sessionId",
                alias = "session_id"
            )]
            session_id: Option<String>,
            #[serde(
                default,
                rename = "messageID",
                alias = "messageId",
                alias = "message_id"
            )]
            message_id: Option<String>,
        }

        let mut normalized_json = Vec::new();
        let mut normalized_files = Vec::new();

        for value in raw_attachments.unwrap_or_default() {
            let Ok(value) = serde_json::from_value::<AttachmentWire>(value) else {
                continue;
            };

            if value.mime.trim().is_empty() || value.url.trim().is_empty() {
                continue;
            }

            let AttachmentWire {
                mime,
                url,
                filename,
                id,
                session_id: attachment_session_id,
                message_id: attachment_message_id,
            } = value;

            let id = id.unwrap_or_else(|| {
                rocode_core::id::create(rocode_core::id::Prefix::Part, true, None)
            });
            let normalized_session_id = attachment_session_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(session_id)
                .to_string();
            let normalized_message_id = attachment_message_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(message_id)
                .to_string();

            let mut normalized = serde_json::Map::new();
            normalized.insert("type".to_string(), serde_json::json!("file"));
            normalized.insert("id".to_string(), serde_json::json!(id.clone()));
            normalized.insert(
                "sessionID".to_string(),
                serde_json::json!(normalized_session_id.clone()),
            );
            normalized.insert(
                "messageID".to_string(),
                serde_json::json!(normalized_message_id.clone()),
            );
            normalized.insert("mime".to_string(), serde_json::json!(&mime));
            normalized.insert("url".to_string(), serde_json::json!(&url));
            if let Some(name) = filename.clone() {
                normalized.insert("filename".to_string(), serde_json::json!(name));
            }

            normalized_json.push(serde_json::Value::Object(normalized));
            normalized_files.push(crate::message_model::FilePart {
                id,
                session_id: normalized_session_id,
                message_id: normalized_message_id,
                mime,
                url,
                filename,
                source: None,
            });
        }

        (
            (!normalized_json.is_empty()).then_some(normalized_json),
            (!normalized_files.is_empty()).then_some(normalized_files),
        )
    }

    pub(super) fn extract_tool_attachments_from_metadata(
        metadata: &mut HashMap<String, serde_json::Value>,
        session_id: &str,
        message_id: &str,
    ) -> (
        Option<Vec<serde_json::Value>>,
        Option<Vec<crate::message_model::FilePart>>,
    ) {
        let raw_attachments = Self::take_attachment_values(metadata);
        Self::normalize_tool_attachments(raw_attachments, session_id, message_id)
    }

    pub(super) fn has_unresolved_tool_calls(message: &SessionMessage) -> bool {
        session_message_to_unified_message(message)
            .parts
            .into_iter()
            .any(|part| {
                let ModelPart::Tool(tool) = part else {
                    return false;
                };

                if tool.tool.trim().is_empty() {
                    return false;
                }

                tool.state.status() == crate::ToolCallStatus::Running
            })
    }

    pub(super) fn parse_json_or_string(raw: &str) -> serde_json::Value {
        // First try standard JSON parse.
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
            // If the parsed value is a JSON string, it may itself contain a
            // JSON object (double-encoded). Try robust object recovery first.
            if matches!(val, serde_json::Value::String(_)) {
                if let Some(obj) = rocode_util::json::try_parse_json_object_robust(raw) {
                    return obj;
                }
            }
            return val;
        }

        // Recover object-shaped arguments with robust parsing.
        if let Some(obj) = rocode_util::json::try_parse_json_object_robust(raw) {
            return obj;
        }

        // If the raw string looks like it could be a JSON object with issues,
        // wrap it so tools can still access it via the registry normalizer.
        tracing::warn!(
            raw_len = raw.len(),
            raw_preview = %raw.chars().take(200).collect::<String>(),
            "tool call arguments failed JSON parse, wrapping as string"
        );
        serde_json::Value::String(raw.to_string())
    }

    pub(super) fn invalid_tool_payload(tool_name: &str, error: &str) -> serde_json::Value {
        serde_json::json!({
            "tool": tool_name,
            "error": error,
        })
    }

    pub(super) fn prevalidate_tool_arguments(
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        if tool_name != "write" {
            return None;
        }

        if let Some(obj) = input.as_object() {
            #[derive(Debug, Deserialize, Default)]
            struct WriteArgumentsWire {
                #[serde(default, rename = "file_path", alias = "filePath")]
                file_path: Option<String>,
                #[serde(default)]
                content: Option<String>,
            }

            let args =
                serde_json::from_value::<WriteArgumentsWire>(input.clone()).unwrap_or_default();
            let file_path = args
                .file_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let content = args.content.as_deref();
            if file_path.is_some() && content.is_some() {
                return None;
            }

            let keys = obj.keys().cloned().collect::<Vec<_>>();
            let mut payload = if file_path.is_none() {
                Self::invalid_tool_payload(
                    "write",
                    "The write tool was called without file_path/filePath. Provide both file_path and content.",
                )
            } else {
                Self::invalid_tool_payload(
                    "write",
                    "The write tool was called without content. Provide both file_path and content.",
                )
            };
            if let Some(map) = payload.as_object_mut() {
                map.insert(
                    "receivedArgs".to_string(),
                    serde_json::json!({
                        "type": "object",
                        "keys": keys,
                    }),
                );
            }
            return Some(payload);
        }

        if let Some(raw) = input.as_str() {
            let mut payload = Self::invalid_tool_payload(
                "write",
                "The write tool arguments could not be parsed into an object. Provide a JSON object with file_path and content.",
            );
            if let Some(map) = payload.as_object_mut() {
                map.insert(
                    "receivedArgs".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "raw_len": raw.len(),
                        "preview": raw.chars().take(240).collect::<String>(),
                    }),
                );
            }
            return Some(payload);
        }

        None
    }

    pub(super) fn sanitize_tool_call_input_for_history(
        tool_name: &str,
        input: &serde_json::Value,
        error: Option<&str>,
    ) -> serde_json::Value {
        if input.is_object() {
            return input.clone();
        }

        if let Some(raw) = input.as_str() {
            if let Some(parsed) = rocode_util::json::try_parse_json_object_robust(raw) {
                return parsed;
            }
            if let Some(recovered) =
                rocode_util::json::recover_tool_arguments_from_jsonish(tool_name, raw)
            {
                return recovered;
            }

            let mut payload = Self::invalid_tool_payload(
                tool_name,
                error.unwrap_or("Tool arguments are malformed or truncated"),
            );
            if let Some(map) = payload.as_object_mut() {
                map.insert(
                    "receivedArgs".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "raw_len": raw.len(),
                        "preview": raw.chars().take(240).collect::<String>(),
                    }),
                );
            }
            return payload;
        }

        Self::invalid_tool_payload(
            tool_name,
            error.unwrap_or("Tool arguments are non-object and cannot be replayed"),
        )
    }

    pub(super) fn tool_call_input_for_execution(
        status: &crate::ToolCallStatus,
        input: &serde_json::Value,
        raw: Option<&str>,
        state: Option<&ModelToolState>,
    ) -> Option<serde_json::Value> {
        let (effective_status, state_input, state_raw) = match state {
            Some(state) => {
                let (state_input, state_raw, state_status) = Self::state_projection(state);
                (state_status, Some(state_input), state_raw)
            }
            None => (*status, None, None),
        };

        tracing::info!(
            status = %format!("{:?}", status),
            effective_status = %format!("{:?}", effective_status),
            input_type = %if input.is_object() { format!("object(keys={})", input.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(",")).unwrap_or_default()) } else if input.is_string() { format!("string(len={})", input.as_str().map(|s| s.len()).unwrap_or(0)) } else { format!("{:?}", input) },
            raw_len = %raw.map(|r| r.len()).unwrap_or(0),
            raw_preview = %raw.unwrap_or("None").chars().take(200).collect::<String>(),
            state_variant = %match state {
                Some(ModelToolState::Pending { .. }) => "Pending",
                Some(ModelToolState::Running { .. }) => "Running",
                Some(ModelToolState::Completed { .. }) => "Completed",
                Some(ModelToolState::Error { .. }) => "Error",
                None => "None",
            },
            "[DIAG] tool_call_input_for_execution entry"
        );

        let legacy_fallback_allowed = state.is_none();
        let raw_input = if legacy_fallback_allowed {
            state_raw.as_deref().or(raw)
        } else {
            state_raw.as_deref()
        }
        .map(str::trim)
        .filter(|s| !s.is_empty());

        match effective_status {
            // TS parity: tool execution begins on "tool-call" (running state),
            // not on partial/pending input fragments.
            crate::ToolCallStatus::Pending => None,
            crate::ToolCallStatus::Running => {
                // Try raw input first (most authoritative source).
                if let Some(raw) = raw_input {
                    let parsed = Self::parse_json_or_string(raw);
                    // If parse_json_or_string returned a non-empty object, use it.
                    if parsed.is_object() && parsed.as_object().is_some_and(|o| !o.is_empty()) {
                        return Some(parsed);
                    }
                    // If it returned a Value::String, the registry normalizer
                    // will try to parse it again. Still usable.
                    if parsed.is_string() {
                        return Some(parsed);
                    }
                }

                // Fall back to state_input or PartType input.
                let fallback = state_input.unwrap_or_else(|| input.clone());

                // If the fallback is an empty object but the PartType input is
                // a string (e.g. Value::String wrapping JSON), try to parse it.
                if fallback.is_object() && fallback.as_object().is_some_and(|o| o.is_empty()) {
                    if legacy_fallback_allowed {
                        // Try the PartType's input directly — it might be a
                        // Value::String containing valid JSON.
                        if let Some(s) = input.as_str() {
                            if let Some(parsed) = rocode_util::json::try_parse_json_object_robust(s)
                            {
                                tracing::debug!(
                                    "tool_call_input_for_execution: recovered args from input string"
                                );
                                return Some(parsed);
                            }
                        }
                        // Also try raw from PartType even if state_raw was empty.
                        if let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) {
                            if let Some(parsed) =
                                rocode_util::json::try_parse_json_object_robust(raw)
                            {
                                tracing::debug!(
                                    "tool_call_input_for_execution: recovered args from raw field"
                                );
                                return Some(parsed);
                            }
                        }
                    }
                }

                Some(fallback)
            }
            crate::ToolCallStatus::Completed | crate::ToolCallStatus::Error => None,
        }
    }

    pub(super) fn append_delta_part(message: &mut SessionMessage, reasoning: bool, delta: &str) {
        if delta.is_empty() {
            return;
        }

        for idx in (0..message.parts.len()).rev() {
            match (&mut message.parts[idx].part_type, reasoning) {
                (PartType::Reasoning { text }, true)
                | (
                    PartType::Text {
                        text,
                        synthetic: _,
                        ignored: _,
                    },
                    false,
                ) => {
                    text.push_str(delta);
                    return;
                }
                _ => continue,
            }
        }

        if reasoning {
            message.add_reasoning(delta.to_string());
        } else {
            message.add_text(delta.to_string());
        }
    }

    /// Mark any tool calls that lack a corresponding tool result as aborted.
    pub(super) fn abort_pending_tool_calls(session: &mut Session) {
        let mut resolved_call_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut pending_calls: std::collections::HashSet<String> = std::collections::HashSet::new();

        for msg in &session.messages {
            for part in session_message_to_unified_message(msg).parts {
                let ModelPart::Tool(tool) = part else {
                    continue;
                };
                match tool.state.status() {
                    crate::ToolCallStatus::Completed | crate::ToolCallStatus::Error => {
                        resolved_call_ids.insert(tool.call_id);
                    }
                    crate::ToolCallStatus::Pending | crate::ToolCallStatus::Running => {
                        pending_calls.insert(tool.call_id);
                    }
                }
            }
        }

        let mut pending_calls: Vec<String> = pending_calls
            .into_iter()
            .filter(|id| !resolved_call_ids.contains(id))
            .collect();
        pending_calls.sort();

        if pending_calls.is_empty() {
            return;
        }

        tracing::info!(
            count = pending_calls.len(),
            "Marking pending tool calls as aborted"
        );
        let pending_set: std::collections::HashSet<String> =
            pending_calls.iter().cloned().collect();
        let now = chrono::Utc::now().timestamp_millis();
        for msg in &mut session.messages {
            for part in &mut msg.parts {
                let PartType::ToolCall {
                    id,
                    name,
                    input,
                    status,
                    raw,
                    state,
                } = &mut part.part_type
                else {
                    continue;
                };
                if !pending_set.contains(id) {
                    continue;
                }

                let input_for_history = state.as_ref().map(|value| value.input()).unwrap_or(input);
                let sanitized_input = Self::sanitize_tool_call_input_for_history(
                    name,
                    input_for_history,
                    Some("Tool execution aborted"),
                );

                let error_state = ModelToolState::Error {
                    input: sanitized_input,
                    error: "Tool execution aborted".to_string(),
                    metadata: None,
                    time: ModelErrorTime {
                        start: now,
                        end: now,
                    },
                };
                *status = crate::ToolCallStatus::Error;
                *raw = None;
                *input = match &error_state {
                    ModelToolState::Error { input, .. } => input.clone(),
                    _ => serde_json::json!({}),
                };
                *state = Some(tool_state_to_canonical(&error_state));
            }
        }

        let mut tool_results_msg = SessionMessage::tool(session.id.clone());
        for call_id in &pending_calls {
            tool_results_msg.add_tool_result(call_id, "Tool execution aborted", true);
        }
        session.messages.push(tool_results_msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PartType, Role, Session, SessionMessage};
    use rocode_message::part::ToolState as CanonToolState;
    use std::collections::HashMap;

    #[test]
    fn abort_pending_tool_calls_marks_unresolved_calls_as_error() {
        let mut session = Session::new(".");
        let sid = session.id.clone();

        // Add a user message
        session
            .messages
            .push(SessionMessage::user(sid.clone(), "do something"));

        // Add an assistant message with two tool calls but only one result
        let mut assistant = SessionMessage::assistant(sid.clone());
        assistant.add_tool_call("call_1", "bash", serde_json::json!({"command": "echo a"}));
        assistant.add_tool_call("call_2", "read_file", serde_json::json!({"path": "foo.rs"}));
        session.messages.push(assistant);
        let mut existing_tool_result = SessionMessage::tool(sid.clone());
        existing_tool_result.add_tool_result("call_1", "output a", false);
        session.messages.push(existing_tool_result);
        // call_2 has no result — simulates abort mid-execution

        SessionPrompt::abort_pending_tool_calls(&mut session);

        // call_2 should now have an error result in the latest tool message
        let last_tool_msg = session
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::Tool))
            .unwrap();

        let error_results: Vec<_> = last_tool_msg
            .parts
            .iter()
            .filter(|p| matches!(
                &p.part_type,
                PartType::ToolResult { tool_call_id, is_error, content, .. }
                    if tool_call_id == "call_2" && *is_error && content == "Tool execution aborted"
            ))
            .collect();

        assert_eq!(error_results.len(), 1, "call_2 should have an error result");
    }

    #[test]
    fn abort_pending_tool_calls_noop_when_all_resolved() {
        let mut session = Session::new(".");
        let sid = session.id.clone();

        session
            .messages
            .push(SessionMessage::user(sid.clone(), "do something"));

        let mut assistant = SessionMessage::assistant(sid.clone());
        assistant.add_tool_call("call_1", "bash", serde_json::json!({"command": "echo a"}));
        session.messages.push(assistant);
        let mut tool_result = SessionMessage::tool(sid.clone());
        tool_result.add_tool_result("call_1", "output a", false);
        session.messages.push(tool_result);

        let part_count_before = session.messages.last().map(|m| m.parts.len()).unwrap_or(0);

        SessionPrompt::abort_pending_tool_calls(&mut session);

        let part_count_after = session.messages.last().map(|m| m.parts.len()).unwrap_or(0);

        assert_eq!(
            part_count_before, part_count_after,
            "No new parts should be added"
        );
    }

    #[test]
    fn abort_pending_tool_calls_handles_multiple_pending() {
        let mut session = Session::new(".");
        let sid = session.id.clone();

        session
            .messages
            .push(SessionMessage::user(sid.clone(), "do something"));

        let mut assistant = SessionMessage::assistant(sid.clone());
        assistant.add_tool_call("call_1", "bash", serde_json::json!({}));
        assistant.add_tool_call("call_2", "read_file", serde_json::json!({}));
        assistant.add_tool_call("call_3", "write_file", serde_json::json!({}));
        // No results at all — all three are pending
        session.messages.push(assistant);

        SessionPrompt::abort_pending_tool_calls(&mut session);

        let last_tool_msg = session
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::Tool))
            .unwrap();

        let abort_results: Vec<_> = last_tool_msg
            .parts
            .iter()
            .filter(|p| {
                matches!(
                    &p.part_type,
                    PartType::ToolResult { is_error, content, .. }
                        if *is_error && content == "Tool execution aborted"
                )
            })
            .collect();

        assert_eq!(
            abort_results.len(),
            3,
            "All three pending calls should be aborted"
        );
    }

    #[test]
    fn tool_call_input_for_execution_skips_empty_pending() {
        let input = serde_json::json!({});
        let resolved = SessionPrompt::tool_call_input_for_execution(
            &crate::ToolCallStatus::Pending,
            &input,
            None,
            None,
        );
        assert!(resolved.is_none());
    }

    #[test]
    fn tool_call_input_for_execution_pending_ignores_raw_input() {
        let input = serde_json::json!({});
        let resolved = SessionPrompt::tool_call_input_for_execution(
            &crate::ToolCallStatus::Pending,
            &input,
            Some("[filePath=/tmp/a.html]"),
            None,
        );
        assert!(resolved.is_none());
    }

    #[test]
    fn tool_call_input_for_execution_running_keeps_object_input() {
        let input = serde_json::json!({});
        let resolved = SessionPrompt::tool_call_input_for_execution(
            &crate::ToolCallStatus::Running,
            &input,
            None,
            None,
        );
        assert_eq!(resolved, Some(serde_json::json!({})));
    }

    #[test]
    fn tool_call_input_for_execution_prefers_state_over_status_and_input() {
        let status = crate::ToolCallStatus::Pending;
        let input = serde_json::Value::String("{}".to_string());
        let state = crate::ToolState::Running {
            input: serde_json::json!({"command": "echo hi"}),
            title: None,
            metadata: None,
            time: crate::RunningTime { start: 1 },
        };

        let resolved =
            SessionPrompt::tool_call_input_for_execution(&status, &input, None, Some(&state));

        assert_eq!(resolved, Some(serde_json::json!({"command": "echo hi"})));
    }

    #[test]
    fn tool_call_input_for_execution_ignores_legacy_raw_when_state_present() {
        let status = crate::ToolCallStatus::Running;
        let input = serde_json::json!({});
        let legacy_raw = Some("{\"legacy\":true}");
        let state = crate::ToolState::Running {
            input: serde_json::json!({"from_state": true}),
            title: None,
            metadata: None,
            time: crate::RunningTime { start: 1 },
        };

        let resolved =
            SessionPrompt::tool_call_input_for_execution(&status, &input, legacy_raw, Some(&state));

        assert_eq!(resolved, Some(serde_json::json!({"from_state": true})));
    }

    #[test]
    fn upsert_tool_call_prefers_existing_state_projection_when_no_tool_state_provided() {
        let mut message = SessionMessage::assistant("ses_1");
        message.parts.push(crate::MessagePart {
            id: "prt_existing".to_string(),
            part_type: PartType::ToolCall {
                id: "call_1".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"stale": true}),
                status: crate::ToolCallStatus::Pending,
                raw: Some("{\"stale\":true}".to_string()),
                state: Some(CanonToolState::Running {
                    input: serde_json::json!({"from_state": true}),
                    title: None,
                    metadata: None,
                    time: rocode_message::part::RunningTime { start: 1 },
                }),
            },
            created_at: chrono::Utc::now(),
            message_id: Some(message.id.clone()),
        });

        SessionPrompt::upsert_tool_call_part(
            &mut message,
            "call_1",
            None,
            None,
            None,
            Some(crate::ToolCallStatus::Running),
            None,
        );

        let updated = message
            .parts
            .iter()
            .find_map(|part| match &part.part_type {
                PartType::ToolCall {
                    id,
                    input,
                    status,
                    state,
                    ..
                } if id == "call_1" => Some((input, status, state)),
                _ => None,
            })
            .expect("updated tool call part missing");

        assert_eq!(updated.0, &serde_json::json!({"from_state": true}));
        assert!(matches!(updated.1, crate::ToolCallStatus::Running));
        assert!(matches!(
            updated.2,
            Some(CanonToolState::Running { input, .. }) if input == &serde_json::json!({"from_state": true})
        ));
    }

    #[test]
    fn upsert_tool_call_ignores_legacy_field_override_when_state_present() {
        let mut message = SessionMessage::assistant("ses_1");
        message.parts.push(crate::MessagePart {
            id: "prt_existing".to_string(),
            part_type: PartType::ToolCall {
                id: "call_2".to_string(),
                name: "bash".to_string(),
                input: serde_json::json!({"stale": true}),
                status: crate::ToolCallStatus::Pending,
                raw: Some("{\"stale\":true}".to_string()),
                state: Some(CanonToolState::Running {
                    input: serde_json::json!({"from_state": true}),
                    title: None,
                    metadata: None,
                    time: rocode_message::part::RunningTime { start: 1 },
                }),
            },
            created_at: chrono::Utc::now(),
            message_id: Some(message.id.clone()),
        });

        SessionPrompt::upsert_tool_call_part(
            &mut message,
            "call_2",
            None,
            Some(serde_json::json!({"legacy": true})),
            Some("{\"legacy\":true}".to_string()),
            Some(crate::ToolCallStatus::Error),
            None,
        );

        let updated = message
            .parts
            .iter()
            .find_map(|part| match &part.part_type {
                PartType::ToolCall {
                    id,
                    input,
                    status,
                    state,
                    ..
                } if id == "call_2" => Some((input, status, state)),
                _ => None,
            })
            .expect("updated tool call part missing");

        assert_eq!(updated.0, &serde_json::json!({"from_state": true}));
        assert!(matches!(updated.1, crate::ToolCallStatus::Running));
        assert!(matches!(
            updated.2,
            Some(CanonToolState::Running { input, .. }) if input == &serde_json::json!({"from_state": true})
        ));
    }

    #[test]
    fn invalid_tool_payload_is_ts_shape() {
        let payload = SessionPrompt::invalid_tool_payload("read", "missing filePath");
        assert_eq!(payload.get("tool").and_then(|v| v.as_str()), Some("read"));
        assert_eq!(
            payload.get("error").and_then(|v| v.as_str()),
            Some("missing filePath")
        );
        assert!(payload.get("receivedArgs").is_none());
    }

    #[test]
    fn sanitize_tool_call_input_for_history_wraps_unrecoverable_string() {
        let input = serde_json::Value::String("not-json".to_string());
        let sanitized = SessionPrompt::sanitize_tool_call_input_for_history(
            "write",
            &input,
            Some("Invalid arguments"),
        );
        assert!(sanitized.is_object());
        assert_eq!(sanitized["tool"], "write");
        assert_eq!(sanitized["error"], "Invalid arguments");
        assert_eq!(sanitized["receivedArgs"]["type"], "string");
    }

    #[test]
    fn abort_pending_tool_calls_sanitizes_pending_tool_input_for_replay() {
        let mut session = Session::new(".");
        let sid = session.id.clone();
        session
            .messages
            .push(SessionMessage::user(sid.clone(), "do something"));

        let mut assistant = SessionMessage::assistant(sid.clone());
        assistant.parts.push(crate::MessagePart {
            id: "prt_bad".to_string(),
            part_type: PartType::ToolCall {
                id: "call_bad".to_string(),
                name: "write".to_string(),
                input: serde_json::Value::String("not-json".to_string()),
                status: crate::ToolCallStatus::Pending,
                raw: Some("not-json".to_string()),
                state: None,
            },
            created_at: chrono::Utc::now(),
            message_id: None,
        });
        session.messages.push(assistant);

        SessionPrompt::abort_pending_tool_calls(&mut session);

        let assistant = session
            .messages
            .iter()
            .find(|m| matches!(m.role, Role::Assistant))
            .expect("assistant message missing");
        let tool_call = assistant
            .parts
            .iter()
            .find_map(|p| match &p.part_type {
                PartType::ToolCall { input, status, .. } => Some((input, status)),
                _ => None,
            })
            .expect("tool call missing");
        assert!(
            tool_call.0.is_object(),
            "pending malformed input should be sanitized for replay"
        );
        assert!(matches!(tool_call.1, crate::ToolCallStatus::Error));
    }

    #[test]
    fn parse_json_or_string_recovers_stringified_object() {
        let inner = r#"{"file_path":"/tmp/a","content":"hello"}"#;
        let outer = serde_json::to_string(inner).expect("stringify should succeed");
        let parsed = SessionPrompt::parse_json_or_string(&outer);
        assert_eq!(parsed["file_path"], "/tmp/a");
        assert_eq!(parsed["content"], "hello");
    }

    #[test]
    fn parse_json_or_string_keeps_plain_json_string_when_not_object() {
        let raw = serde_json::to_string("just text").expect("stringify should succeed");
        let parsed = SessionPrompt::parse_json_or_string(&raw);
        assert_eq!(parsed, serde_json::Value::String("just text".to_string()));
    }

    #[test]
    fn prevalidate_write_args_requires_file_path() {
        let input = serde_json::json!({
            "content": "<html>...</html>"
        });
        let invalid = SessionPrompt::prevalidate_tool_arguments("write", &input)
            .expect("should produce invalid payload");
        assert_eq!(invalid["tool"], "write");
        assert!(
            invalid["error"]
                .as_str()
                .unwrap_or_default()
                .contains("without file_path"),
            "error should mention missing file_path"
        );
        assert_eq!(invalid["receivedArgs"]["type"], "object");
    }

    #[test]
    fn prevalidate_write_args_accepts_complete_payload() {
        let input = serde_json::json!({
            "file_path": "t2.html",
            "content": "<html>...</html>"
        });
        assert!(SessionPrompt::prevalidate_tool_arguments("write", &input).is_none());
    }

    #[test]
    fn extract_tool_attachments_from_metadata_moves_attachment_payload_out() {
        let mut metadata = HashMap::new();
        metadata.insert("note".to_string(), serde_json::json!("ok"));
        metadata.insert(
            "attachments".to_string(),
            serde_json::json!([{ "mime": "application/pdf", "url": "data:application/pdf;base64,AA==" }]),
        );
        metadata.insert(
            "attachment".to_string(),
            serde_json::json!({ "mime": "image/png", "url": "data:image/png;base64,BB==" }),
        );

        let (attachments, file_parts) =
            SessionPrompt::extract_tool_attachments_from_metadata(&mut metadata, "ses_1", "msg_1");

        assert_eq!(metadata.get("note").and_then(|v| v.as_str()), Some("ok"));
        assert!(!metadata.contains_key("attachments"));
        assert!(!metadata.contains_key("attachment"));
        assert_eq!(attachments.as_ref().map(|v| v.len()), Some(2));
        assert_eq!(file_parts.as_ref().map(|v| v.len()), Some(2));
    }
}
