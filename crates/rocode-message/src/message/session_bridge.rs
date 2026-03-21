use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

use super::session_message::SessionMessage;
use super::{
    canonical_tool_state_to_message, tool_state_to_canonical, AssistantTime, AssistantTokens,
    CacheTokens, CompletedTime, ErrorTime, MessageError, MessageInfo, MessagePath,
    MessageWithParts, ModelRef, Part, ReasoningTime, RunningTime, StepFinishPart, StepStartPart,
    StepTokens, ToolPart, ToolState, UserTime,
};
use crate::part::{MessagePart, PartType};
use crate::status::ToolCallStatus;

#[derive(Debug, Deserialize)]
struct UnifiedPartsEnvelope {
    #[serde(default)]
    parts: Vec<Part>,
}

fn datetime_from_millis_or_fallback(ms: Option<i64>, fallback: DateTime<Utc>) -> DateTime<Utc> {
    ms.and_then(DateTime::from_timestamp_millis)
        .unwrap_or(fallback)
}

fn assistant_usage_from_info(info: &MessageInfo) -> Option<crate::usage::MessageUsage> {
    let MessageInfo::Assistant { tokens, cost, .. } = info else {
        return None;
    };

    Some(crate::usage::MessageUsage {
        input_tokens: tokens.input.max(0) as u64,
        output_tokens: tokens.output.max(0) as u64,
        reasoning_tokens: tokens.reasoning.max(0) as u64,
        cache_write_tokens: tokens.cache.write.max(0) as u64,
        cache_read_tokens: tokens.cache.read.max(0) as u64,
        total_cost: *cost,
    })
}

fn metadata_string(metadata: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn metadata_i64(metadata: &HashMap<String, serde_json::Value>, key: &str) -> Option<i64> {
    match metadata.get(key) {
        Some(serde_json::Value::Number(value)) => value.as_i64(),
        Some(serde_json::Value::String(value)) => value.parse::<i64>().ok(),
        _ => None,
    }
}

fn metadata_f64(metadata: &HashMap<String, serde_json::Value>, key: &str) -> Option<f64> {
    match metadata.get(key) {
        Some(serde_json::Value::Number(value)) => value.as_f64(),
        Some(serde_json::Value::String(value)) => value.parse::<f64>().ok(),
        _ => None,
    }
}

fn clamp_u64_to_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

/// Convert canonical session message into unified message payload.
pub fn session_message_to_unified_message(message: &SessionMessage) -> MessageWithParts {
    let created_at = message.created_at.timestamp_millis();
    let usage = message.usage.clone();
    let usage_ref = usage.as_ref();
    let metadata = message.metadata.clone();

    let model_provider = metadata_string(&metadata, "model_provider")
        .or_else(|| metadata_string(&metadata, "provider_id"))
        .unwrap_or_else(|| "unknown".to_string());
    let model_id = metadata_string(&metadata, "model_id").unwrap_or_else(|| "unknown".to_string());
    let agent = metadata_string(&metadata, "agent").unwrap_or_else(|| "general".to_string());
    let mode = metadata_string(&metadata, "mode").unwrap_or_else(|| "default".to_string());
    let variant = metadata_string(&metadata, "variant");
    let finish = message
        .finish
        .clone()
        .or_else(|| metadata_string(&metadata, "finish_reason"));

    let info = match message.role {
        rocode_types::Role::User => MessageInfo::User {
            id: message.id.clone(),
            session_id: message.session_id.clone(),
            time: UserTime {
                created: created_at,
            },
            agent,
            model: ModelRef {
                provider_id: model_provider,
                model_id,
            },
            format: None,
            summary: None,
            system: None,
            tools: None,
            variant,
        },
        rocode_types::Role::Assistant => {
            let input_tokens = usage_ref
                .map(|u| clamp_u64_to_i32(u.input_tokens))
                .unwrap_or(0);
            let output_tokens = usage_ref
                .map(|u| clamp_u64_to_i32(u.output_tokens))
                .unwrap_or(0);
            let reasoning_tokens = usage_ref
                .map(|u| clamp_u64_to_i32(u.reasoning_tokens))
                .unwrap_or(0);
            let cache_read_tokens = usage_ref
                .map(|u| clamp_u64_to_i32(u.cache_read_tokens))
                .unwrap_or(0);
            let cache_write_tokens = usage_ref
                .map(|u| clamp_u64_to_i32(u.cache_write_tokens))
                .unwrap_or(0);
            let cost = usage_ref
                .map(|u| u.total_cost)
                .or_else(|| metadata_f64(&metadata, "cost"))
                .unwrap_or(0.0);
            let cwd = metadata_string(&metadata, "cwd").unwrap_or_default();
            let root = metadata_string(&metadata, "root").unwrap_or_else(|| cwd.clone());

            MessageInfo::Assistant {
                id: message.id.clone(),
                session_id: message.session_id.clone(),
                time: AssistantTime {
                    created: created_at,
                    completed: metadata_i64(&metadata, "completed_at"),
                },
                parent_id: metadata_string(&metadata, "parent_id")
                    .unwrap_or_else(|| message.id.clone()),
                model_id,
                provider_id: model_provider,
                mode,
                agent,
                path: MessagePath { cwd, root },
                summary: None,
                cost,
                tokens: AssistantTokens {
                    total: Some(input_tokens.saturating_add(output_tokens)),
                    input: input_tokens,
                    output: output_tokens,
                    reasoning: reasoning_tokens,
                    cache: CacheTokens {
                        read: cache_read_tokens,
                        write: cache_write_tokens,
                    },
                },
                error: metadata_string(&metadata, "error")
                    .map(|message| MessageError::Unknown { message }),
                structured: metadata.get("structured").cloned(),
                variant,
                finish: finish.clone(),
            }
        }
        rocode_types::Role::System => MessageInfo::System {
            id: message.id.clone(),
            session_id: message.session_id.clone(),
            time: UserTime {
                created: created_at,
            },
        },
        rocode_types::Role::Tool => MessageInfo::Tool {
            id: message.id.clone(),
            session_id: message.session_id.clone(),
            time: UserTime {
                created: created_at,
            },
        },
    };

    MessageWithParts {
        info,
        parts: session_parts_to_unified(&message.parts, &message.session_id, &message.id),
        metadata,
        usage,
        finish,
    }
}

/// Convert unified message payload into canonical session message.
pub fn unified_message_to_session_message(message: MessageWithParts) -> SessionMessage {
    let MessageWithParts {
        info,
        parts,
        metadata,
        usage,
        finish,
    } = message;

    let id = info.id().to_string();
    let session_id = info.session_id().to_string();
    let role = info.role();
    let created_at =
        DateTime::from_timestamp_millis(info.created_at_millis()).unwrap_or_else(Utc::now);
    let usage = usage.or_else(|| assistant_usage_from_info(&info));
    let finish = finish.or_else(|| info.finish_reason().map(ToString::to_string));
    let parts = unified_parts_to_session(parts, created_at, &id);

    SessionMessage {
        id,
        session_id,
        role,
        parts,
        created_at,
        metadata,
        usage,
        finish,
    }
}

fn session_tool_state_from_status(
    status: &ToolCallStatus,
    input: &serde_json::Value,
    raw: Option<&String>,
    tool_name: &str,
    created_at_ms: i64,
) -> ToolState {
    match status {
        ToolCallStatus::Pending => ToolState::Pending {
            input: input.clone(),
            raw: raw
                .cloned()
                .unwrap_or_else(|| serde_json::to_string(input).unwrap_or_default()),
        },
        ToolCallStatus::Running => ToolState::Running {
            input: input.clone(),
            title: (!tool_name.trim().is_empty()).then_some(tool_name.to_string()),
            metadata: None,
            time: RunningTime {
                start: created_at_ms,
            },
        },
        ToolCallStatus::Completed => ToolState::Completed {
            input: input.clone(),
            output: String::new(),
            title: if tool_name.trim().is_empty() {
                "tool".to_string()
            } else {
                tool_name.to_string()
            },
            metadata: HashMap::new(),
            time: CompletedTime {
                start: created_at_ms,
                end: created_at_ms,
                compacted: None,
            },
            attachments: None,
        },
        ToolCallStatus::Error => ToolState::Error {
            input: input.clone(),
            error: "Tool execution failed".to_string(),
            metadata: None,
            time: ErrorTime {
                start: created_at_ms,
                end: created_at_ms,
            },
        },
    }
}

/// Convert one canonical session message part into unified message part.
fn session_part_to_unified(part: &MessagePart, session_id: &str, message_id: &str) -> Option<Part> {
    let created_at_ms = part.created_at.timestamp_millis();

    Some(match &part.part_type {
        PartType::Text {
            text,
            synthetic,
            ignored,
        } => Part::Text {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            text: text.clone(),
            synthetic: *synthetic,
            ignored: *ignored,
            time: None,
            metadata: None,
        },
        PartType::ToolCall {
            id,
            name,
            input,
            status,
            raw,
            state,
        } => {
            let resolved_state = state
                .as_ref()
                .map(canonical_tool_state_to_message)
                .unwrap_or_else(|| {
                    session_tool_state_from_status(status, input, raw.as_ref(), name, created_at_ms)
                });

            Part::Tool(ToolPart {
                id: part.id.clone(),
                session_id: session_id.to_string(),
                message_id: message_id.to_string(),
                call_id: id.clone(),
                tool: name.clone(),
                state: resolved_state,
                metadata: None,
            })
        }
        PartType::ToolResult {
            tool_call_id,
            content,
            is_error,
            title,
            metadata,
            attachments,
        } => {
            let parsed_attachments = attachments.as_ref().map(|values| {
                values
                    .iter()
                    .filter_map(|value| {
                        serde_json::from_value::<super::FilePart>(value.clone()).ok()
                    })
                    .collect::<Vec<_>>()
            });

            let state = if *is_error {
                ToolState::Error {
                    input: serde_json::json!({}),
                    error: content.clone(),
                    metadata: metadata.clone(),
                    time: ErrorTime {
                        start: created_at_ms,
                        end: created_at_ms,
                    },
                }
            } else {
                ToolState::Completed {
                    input: serde_json::json!({}),
                    output: content.clone(),
                    title: title
                        .clone()
                        .unwrap_or_else(|| "Legacy Tool Result".to_string()),
                    metadata: metadata.clone().unwrap_or_default(),
                    time: CompletedTime {
                        start: created_at_ms,
                        end: created_at_ms,
                        compacted: None,
                    },
                    attachments: parsed_attachments,
                }
            };

            Part::Tool(ToolPart {
                id: part.id.clone(),
                session_id: session_id.to_string(),
                message_id: message_id.to_string(),
                call_id: tool_call_id.clone(),
                tool: title.clone().unwrap_or_else(|| "tool_result".to_string()),
                state,
                metadata: None,
            })
        }
        PartType::Reasoning { text } => Part::Reasoning {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            text: text.clone(),
            metadata: None,
            time: ReasoningTime {
                start: created_at_ms,
                end: Some(created_at_ms),
            },
        },
        PartType::File {
            url,
            filename,
            mime,
        } => Part::File(super::FilePart {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            mime: mime.clone(),
            url: url.clone(),
            filename: Some(filename.clone()),
            source: None,
        }),
        PartType::StepStart { name, .. } => Part::StepStart(StepStartPart {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            snapshot: (!name.trim().is_empty()).then_some(name.clone()),
        }),
        PartType::StepFinish { output, .. } => Part::StepFinish(StepFinishPart {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            reason: output.clone().unwrap_or_else(|| "stop".to_string()),
            snapshot: output.clone(),
            cost: 0.0,
            tokens: StepTokens {
                total: None,
                input: 0,
                output: 0,
                reasoning: 0,
                cache: CacheTokens { read: 0, write: 0 },
            },
        }),
        PartType::Snapshot { content } => Part::Snapshot {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            snapshot: content.clone(),
        },
        PartType::Patch {
            old_string,
            new_string,
            filepath,
            ..
        } => Part::Patch {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            old: (!old_string.is_empty()).then_some(old_string.clone()),
            hash: new_string.clone(),
            files: (!filepath.trim().is_empty())
                .then_some(vec![filepath.clone()])
                .unwrap_or_default(),
        },
        PartType::Agent { name, status } => Part::Agent(super::AgentPart {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            name: name.clone(),
            status: Some(status.clone()),
            source: None,
        }),
        PartType::Subtask {
            id,
            description,
            status,
        } => Part::Subtask(super::SubtaskPart {
            id: id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            prompt: description.clone(),
            description: description.clone(),
            agent: "general".to_string(),
            status: Some(status.clone()),
            model: None,
            command: None,
        }),
        PartType::Retry { count, reason } => Part::Retry(super::RetryPart {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            attempt: *count as i32,
            error: super::ApiError {
                message: reason.clone(),
                status_code: None,
                is_retryable: true,
                response_headers: None,
                response_body: None,
                metadata: None,
            },
            time: super::RetryTime {
                created: created_at_ms,
            },
        }),
        PartType::Compaction { summary } => Part::Compaction(super::CompactionPart {
            id: part.id.clone(),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            auto: !summary.trim().is_empty(),
        }),
    })
}

/// Convert canonical session parts to unified message parts.
fn session_parts_to_unified(
    parts: &[MessagePart],
    session_id: &str,
    message_id: &str,
) -> Vec<Part> {
    parts
        .iter()
        .filter_map(|part| session_part_to_unified(part, session_id, message_id))
        .collect()
}

fn unified_tool_status(state: &ToolState) -> ToolCallStatus {
    match state {
        ToolState::Pending { .. } => ToolCallStatus::Pending,
        ToolState::Running { .. } => ToolCallStatus::Running,
        ToolState::Completed { .. } => ToolCallStatus::Completed,
        ToolState::Error { .. } => ToolCallStatus::Error,
    }
}

fn unified_part_created_at(part: &Part, fallback: DateTime<Utc>) -> DateTime<Utc> {
    match part {
        Part::Text {
            time: Some(time), ..
        } => datetime_from_millis_or_fallback(time.start.or(time.end), fallback),
        Part::Reasoning { time, .. } => {
            datetime_from_millis_or_fallback(Some(time.start), fallback)
        }
        Part::Tool(tp) => match &tp.state {
            ToolState::Pending { .. } => fallback,
            ToolState::Running { time, .. } => {
                datetime_from_millis_or_fallback(Some(time.start), fallback)
            }
            ToolState::Completed { time, .. } => {
                datetime_from_millis_or_fallback(Some(time.start), fallback)
            }
            ToolState::Error { time, .. } => {
                datetime_from_millis_or_fallback(Some(time.start), fallback)
            }
        },
        Part::Retry(retry) => datetime_from_millis_or_fallback(Some(retry.time.created), fallback),
        _ => fallback,
    }
}

fn unified_tool_attachments_json(
    attachments: &Option<Vec<super::FilePart>>,
) -> Option<Vec<serde_json::Value>> {
    let values = attachments
        .as_ref()
        .map(|files| {
            files
                .iter()
                .filter_map(|file| serde_json::to_value(file).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (!values.is_empty()).then_some(values)
}

fn unified_part_to_session_parts(
    part: Part,
    fallback_created_at: DateTime<Utc>,
    message_id: &str,
) -> Vec<MessagePart> {
    let created_at = unified_part_created_at(&part, fallback_created_at);

    match part {
        Part::Text {
            id,
            text,
            synthetic,
            ignored,
            ..
        } => vec![MessagePart {
            id,
            part_type: PartType::Text {
                text,
                synthetic,
                ignored,
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Reasoning { id, text, .. } => vec![MessagePart {
            id,
            part_type: PartType::Reasoning { text },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::File(file) => vec![MessagePart {
            id: file.id,
            part_type: PartType::File {
                url: file.url,
                filename: file.filename.unwrap_or_else(|| "attachment".to_string()),
                mime: file.mime,
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Tool(tool) => {
            let (input, raw_input) = match &tool.state {
                ToolState::Pending { input, raw } => (input.clone(), Some(raw.clone())),
                ToolState::Running { input, .. }
                | ToolState::Completed { input, .. }
                | ToolState::Error { input, .. } => (input.clone(), None),
            };

            let mut out = vec![MessagePart {
                id: tool.id.clone(),
                part_type: PartType::ToolCall {
                    id: tool.call_id.clone(),
                    name: tool.tool.clone(),
                    input,
                    status: unified_tool_status(&tool.state),
                    raw: raw_input,
                    state: Some(tool_state_to_canonical(&tool.state)),
                },
                created_at,
                message_id: Some(message_id.to_string()),
            }];

            match &tool.state {
                ToolState::Completed {
                    output,
                    title,
                    metadata,
                    attachments,
                    time,
                    ..
                } => {
                    let result_created_at =
                        datetime_from_millis_or_fallback(Some(time.end), created_at);
                    out.push(MessagePart {
                        id: format!("{}_result", tool.id),
                        part_type: PartType::ToolResult {
                            tool_call_id: tool.call_id,
                            content: output.clone(),
                            is_error: false,
                            title: Some(title.clone()),
                            metadata: (!metadata.is_empty()).then_some(metadata.clone()),
                            attachments: unified_tool_attachments_json(attachments),
                        },
                        created_at: result_created_at,
                        message_id: Some(message_id.to_string()),
                    });
                }
                ToolState::Error {
                    error,
                    metadata,
                    time,
                    ..
                } => {
                    let result_created_at =
                        datetime_from_millis_or_fallback(Some(time.end), created_at);
                    out.push(MessagePart {
                        id: format!("{}_error", tool.id),
                        part_type: PartType::ToolResult {
                            tool_call_id: tool.call_id,
                            content: error.clone(),
                            is_error: true,
                            title: Some(tool.tool),
                            metadata: metadata.clone(),
                            attachments: None,
                        },
                        created_at: result_created_at,
                        message_id: Some(message_id.to_string()),
                    });
                }
                ToolState::Pending { .. } | ToolState::Running { .. } => {}
            }

            out
        }
        Part::StepStart(step) => vec![MessagePart {
            id: step.id.clone(),
            part_type: PartType::StepStart {
                id: step.id,
                name: String::new(),
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::StepFinish(step) => vec![MessagePart {
            id: step.id.clone(),
            part_type: PartType::StepFinish {
                id: step.id,
                output: step.snapshot.or_else(|| Some(step.reason)),
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Snapshot { id, snapshot, .. } => vec![MessagePart {
            id,
            part_type: PartType::Snapshot { content: snapshot },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Patch {
            id,
            old,
            hash,
            files,
            ..
        } => vec![MessagePart {
            id,
            part_type: PartType::Patch {
                old_string: old.unwrap_or_default(),
                new_string: hash,
                filepath: files.into_iter().next().unwrap_or_default(),
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Agent(agent) => vec![MessagePart {
            id: agent.id,
            part_type: PartType::Agent {
                name: agent.name,
                status: agent.status.unwrap_or_else(|| "pending".to_string()),
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Subtask(subtask) => vec![MessagePart {
            id: subtask.id.clone(),
            part_type: PartType::Subtask {
                id: subtask.id,
                description: subtask.description,
                status: subtask.status.unwrap_or_else(|| "pending".to_string()),
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Retry(retry) => vec![MessagePart {
            id: retry.id,
            part_type: PartType::Retry {
                count: retry.attempt.max(0) as u32,
                reason: retry.error.message,
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
        Part::Compaction(compaction) => vec![MessagePart {
            id: compaction.id,
            part_type: PartType::Compaction {
                summary: if compaction.auto {
                    "auto".to_string()
                } else {
                    String::new()
                },
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }],
    }
}

/// Convert unified message parts into canonical session parts.
pub fn unified_parts_to_session(
    parts: Vec<Part>,
    fallback_created_at: DateTime<Utc>,
    message_id: &str,
) -> Vec<MessagePart> {
    parts
        .into_iter()
        .flat_map(|part| unified_part_to_session_parts(part, fallback_created_at, message_id))
        .collect()
}

/// Parse unified-only storage payload into canonical session parts.
///
/// Supports:
/// - unified `Vec<Part>`
/// - unified `MessageWithParts`
/// - `{ parts: ... }` envelope for unified arrays
pub fn try_parse_unified_parts(
    raw: &str,
    fallback_created_at: DateTime<Utc>,
    message_id: &str,
) -> Option<Vec<MessagePart>> {
    if let Ok(parts) = serde_json::from_str::<Vec<Part>>(raw) {
        return Some(unified_parts_to_session(
            parts,
            fallback_created_at,
            message_id,
        ));
    }
    if let Ok(message) = serde_json::from_str::<MessageWithParts>(raw) {
        return Some(unified_parts_to_session(
            message.parts,
            fallback_created_at,
            message_id,
        ));
    }
    if let Ok(env) = serde_json::from_str::<UnifiedPartsEnvelope>(raw) {
        return Some(unified_parts_to_session(
            env.parts,
            fallback_created_at,
            message_id,
        ));
    }
    None
}

/// Option-aware wrapper that returns empty vec on missing/invalid data.
pub fn parse_unified_parts(
    raw: Option<&str>,
    fallback_created_at: DateTime<Utc>,
    message_id: &str,
) -> Vec<MessagePart> {
    raw.and_then(|value| try_parse_unified_parts(value, fallback_created_at, message_id))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::PartKind;

    fn sample_usage() -> crate::usage::MessageUsage {
        crate::usage::MessageUsage {
            input_tokens: 11,
            output_tokens: 22,
            reasoning_tokens: 33,
            cache_read_tokens: 44,
            cache_write_tokens: 55,
            total_cost: 0.42,
        }
    }

    #[test]
    fn unified_message_to_session_preserves_metadata_usage_and_finish() {
        let message = MessageWithParts {
            info: MessageInfo::System {
                id: "msg_1".to_string(),
                session_id: "ses_1".to_string(),
                time: super::super::UserTime {
                    created: 1_700_000_000_000,
                },
            },
            parts: vec![Part::Text {
                id: "prt_1".to_string(),
                session_id: "ses_1".to_string(),
                message_id: "msg_1".to_string(),
                text: "hello".to_string(),
                synthetic: None,
                ignored: None,
                time: None,
                metadata: None,
            }],
            metadata: HashMap::from([(String::from("scope"), serde_json::json!("system"))]),
            usage: Some(sample_usage()),
            finish: Some("stop".to_string()),
        };

        let converted = unified_message_to_session_message(message);
        assert_eq!(converted.id, "msg_1");
        assert_eq!(converted.session_id, "ses_1");
        assert_eq!(converted.role, rocode_types::Role::System);
        assert_eq!(
            converted.metadata.get("scope"),
            Some(&serde_json::json!("system"))
        );
        assert_eq!(converted.usage, Some(sample_usage()));
        assert_eq!(converted.finish.as_deref(), Some("stop"));
        assert_eq!(converted.parts.len(), 1);
        assert_eq!(converted.parts[0].kind(), PartKind::Text);
    }

    #[test]
    fn unified_message_to_session_derives_assistant_usage_when_missing() {
        let message = MessageWithParts {
            info: MessageInfo::Assistant {
                id: "msg_assistant".to_string(),
                session_id: "ses_1".to_string(),
                time: super::super::AssistantTime {
                    created: 1_700_000_000_001,
                    completed: Some(1_700_000_000_123),
                },
                parent_id: "msg_parent".to_string(),
                model_id: "model-x".to_string(),
                provider_id: "provider-y".to_string(),
                mode: "chat".to_string(),
                agent: "general".to_string(),
                path: super::super::MessagePath {
                    cwd: ".".to_string(),
                    root: "/tmp".to_string(),
                },
                summary: None,
                cost: 1.25,
                tokens: super::super::AssistantTokens {
                    total: Some(100),
                    input: 10,
                    output: 20,
                    reasoning: 30,
                    cache: CacheTokens {
                        read: 40,
                        write: 50,
                    },
                },
                error: None,
                structured: None,
                variant: None,
                finish: Some("stop".to_string()),
            },
            parts: Vec::new(),
            metadata: HashMap::new(),
            usage: None,
            finish: None,
        };

        let converted = unified_message_to_session_message(message);
        assert_eq!(
            converted.usage,
            Some(crate::usage::MessageUsage {
                input_tokens: 10,
                output_tokens: 20,
                reasoning_tokens: 30,
                cache_read_tokens: 40,
                cache_write_tokens: 50,
                total_cost: 1.25,
            })
        );
        assert_eq!(converted.finish.as_deref(), Some("stop"));
    }

    #[test]
    fn unified_tool_error_preserves_metadata_in_tool_result() {
        let metadata = HashMap::from([(String::from("code"), serde_json::json!(500))]);
        let parts = unified_parts_to_session(
            vec![Part::Tool(ToolPart {
                id: "prt_tool".to_string(),
                session_id: "ses_1".to_string(),
                message_id: "msg_1".to_string(),
                call_id: "call_1".to_string(),
                tool: "read".to_string(),
                state: ToolState::Error {
                    input: serde_json::json!({"path":"a.txt"}),
                    error: "boom".to_string(),
                    metadata: Some(metadata.clone()),
                    time: ErrorTime { start: 1, end: 2 },
                },
                metadata: None,
            })],
            chrono::Utc::now(),
            "msg_1",
        );

        let tool_result = parts
            .into_iter()
            .find(|part| part.kind() == PartKind::ToolResult);
        let Some(tool_result) = tool_result else {
            panic!("tool result should exist");
        };
        match tool_result.part_type {
            PartType::ToolResult {
                is_error,
                metadata: result_metadata,
                ..
            } => {
                assert!(is_error);
                assert_eq!(result_metadata, Some(metadata));
            }
            other => panic!("unexpected part type: {:?}", other),
        }
    }

    #[test]
    fn session_agent_and_subtask_status_are_preserved_through_unified_bridge() {
        let created_at = chrono::Utc::now();
        let message_id = "msg_1";
        let session_id = "ses_1";

        let session_parts = vec![
            MessagePart {
                id: "prt_agent".to_string(),
                part_type: PartType::Agent {
                    name: "planner".to_string(),
                    status: "running".to_string(),
                },
                created_at,
                message_id: Some(message_id.to_string()),
            },
            MessagePart {
                id: "prt_subtask".to_string(),
                part_type: PartType::Subtask {
                    id: "st_1".to_string(),
                    description: "collect evidence".to_string(),
                    status: "completed".to_string(),
                },
                created_at,
                message_id: Some(message_id.to_string()),
            },
        ];

        let unified = session_parts_to_unified(&session_parts, session_id, message_id);
        let restored = unified_parts_to_session(unified, created_at, message_id);

        let agent = restored.iter().find_map(|part| match &part.part_type {
            PartType::Agent { status, .. } => Some(status.as_str()),
            _ => None,
        });
        assert_eq!(agent, Some("running"));

        let subtask = restored.iter().find_map(|part| match &part.part_type {
            PartType::Subtask { status, .. } => Some(status.as_str()),
            _ => None,
        });
        assert_eq!(subtask, Some("completed"));
    }

    #[test]
    fn patch_old_string_roundtrip_is_preserved() {
        let created_at = chrono::Utc::now();
        let message_id = "msg_patch";
        let session_id = "ses_patch";
        let session_parts = vec![MessagePart {
            id: "prt_patch".to_string(),
            part_type: PartType::Patch {
                old_string: "old line\n".to_string(),
                new_string: "new line\n".to_string(),
                filepath: "src/lib.rs".to_string(),
            },
            created_at,
            message_id: Some(message_id.to_string()),
        }];

        let unified = session_parts_to_unified(&session_parts, session_id, message_id);
        let restored = unified_parts_to_session(unified, created_at, message_id);

        let patch = restored.into_iter().find_map(|part| match part.part_type {
            PartType::Patch {
                old_string,
                new_string,
                filepath,
            } => Some((old_string, new_string, filepath)),
            _ => None,
        });

        assert_eq!(
            patch,
            Some((
                "old line\n".to_string(),
                "new line\n".to_string(),
                "src/lib.rs".to_string(),
            ))
        );
    }

    #[test]
    fn try_parse_unified_parts_rejects_canonical_parts_array() {
        let raw = serde_json::to_string(&vec![MessagePart::new(PartType::Text {
            text: "legacy".to_string(),
            synthetic: None,
            ignored: None,
        })])
        .expect("serialize canonical part array");

        let parsed = try_parse_unified_parts(&raw, chrono::Utc::now(), "msg_legacy");
        assert!(parsed.is_none());
    }

    #[test]
    fn session_message_to_unified_preserves_usage_and_finish() {
        let mut metadata = HashMap::new();
        metadata.insert("model_provider".to_string(), serde_json::json!("openai"));
        metadata.insert("model_id".to_string(), serde_json::json!("gpt-4o"));
        metadata.insert("agent".to_string(), serde_json::json!("general"));
        metadata.insert("mode".to_string(), serde_json::json!("chat"));

        let message = SessionMessage {
            id: "msg_1".to_string(),
            session_id: "ses_1".to_string(),
            role: rocode_types::Role::Assistant,
            parts: vec![MessagePart::new(PartType::Text {
                text: "hello".to_string(),
                synthetic: None,
                ignored: None,
            })],
            created_at: chrono::Utc::now(),
            metadata,
            usage: Some(sample_usage()),
            finish: Some("stop".to_string()),
        };

        let unified = session_message_to_unified_message(&message);
        assert_eq!(unified.usage, Some(sample_usage()));
        assert_eq!(unified.finish.as_deref(), Some("stop"));
        assert_eq!(unified.parts.len(), 1);
        assert!(matches!(unified.parts.first(), Some(Part::Text { .. })));

        match unified.info {
            MessageInfo::Assistant {
                provider_id,
                model_id,
                finish,
                ..
            } => {
                assert_eq!(provider_id, "openai");
                assert_eq!(model_id, "gpt-4o");
                assert_eq!(finish.as_deref(), Some("stop"));
            }
            other => panic!("unexpected info variant: {other:?}"),
        }
    }
}
