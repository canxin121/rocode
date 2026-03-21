use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::session_runtime::{assistant_visible_text, decision_from_stage_text};
use crate::{ApiError, Result, ServerState};
use rocode_session::message_model::{
    session_message_to_unified_message, unified_parts_to_session, Part as ModelPart,
    ToolState as ModelToolState,
};

use super::session_crud::persist_sessions_if_enabled;

#[derive(Debug, Deserialize)]
pub(crate) struct SendMessageRequest {
    pub content: String,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub scheduler_profile: Option<String>,
    pub variant: Option<String>,
    #[allow(dead_code)]
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(super) struct MessageInfo {
    pub id: String,
    pub session_id: String,
    pub role: rocode_session::Role,
    pub parts: Vec<ModelPart>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub finish: Option<String>,
    pub error: Option<String>,
    pub cost: f64,
    pub tokens: MessageTokensInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Default)]
pub(super) struct MessageTokensInfo {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Debug, Serialize)]
pub(super) struct MessageSummaryInfo {
    pub id: String,
    pub session_id: String,
    pub role: rocode_session::Role,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
}

#[derive(Debug, Serialize)]
struct SchedulerDecisionSpecView<'a> {
    version: &'a str,
    show_header_divider: bool,
    field_order: &'a str,
    field_label_emphasis: &'a str,
    status_palette: &'a str,
    section_spacing: &'a str,
    update_policy: &'a str,
}

#[derive(Debug, Serialize)]
struct SchedulerDecisionFieldView<'a> {
    label: &'a str,
    value: &'a str,
    tone: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct SchedulerDecisionSectionView<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(Debug, Serialize)]
struct MessagePartResponse {
    part: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub(super) struct MessageDeletedResponse {
    deleted: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct AddPartResponse {
    added: bool,
    session_id: String,
    message_id: String,
    part_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct DeletePartResponse {
    deleted: bool,
    session_id: String,
    message_id: String,
    part_id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListMessageSummariesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub(super) async fn list_message_summaries(
    State(state): State<Arc<ServerState>>,
    Path(session_id): Path<String>,
    Query(query): Query<ListMessageSummariesQuery>,
) -> Result<(HeaderMap, Json<Vec<MessageSummaryInfo>>)> {
    let session_exists = state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?;
    if !session_exists {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.filter(|value| *value > 0).unwrap_or(50);

    // Use in-memory summaries as runtime source of truth.
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
    let total = session.messages.len();

    let mut infos = Vec::new();
    for message in session.messages.iter().skip(offset).take(limit) {
        infos.push(MessageSummaryInfo {
            id: message.id.clone(),
            session_id: session_id.clone(),
            role: message.role,
            created_at: message.created_at.timestamp_millis(),
            finish: message.finish.clone(),
        });
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Total-Count",
        HeaderValue::from_str(&total.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Returned-Count",
        HeaderValue::from_str(&infos.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Offset",
        HeaderValue::from_str(&offset.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Limit",
        HeaderValue::from_str(&limit.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    Ok((headers, Json(infos)))
}

fn model_part_type_name(part: &ModelPart) -> &'static str {
    match part {
        ModelPart::Text { .. } => "text",
        ModelPart::Subtask(_) => "subtask",
        ModelPart::Reasoning { .. } => "reasoning",
        ModelPart::File(_) => "file",
        ModelPart::Tool(_) => "tool",
        ModelPart::StepStart(_) => "step-start",
        ModelPart::StepFinish(_) => "step-finish",
        ModelPart::Snapshot { .. } => "snapshot",
        ModelPart::Patch { .. } => "patch",
        ModelPart::Agent(_) => "agent",
        ModelPart::Retry(_) => "retry",
        ModelPart::Compaction(_) => "compaction",
    }
}

fn model_tool_status_name(state: &ModelToolState) -> &'static str {
    match state {
        ModelToolState::Pending { .. } => "pending",
        ModelToolState::Running { .. } => "running",
        ModelToolState::Completed { .. } => "completed",
        ModelToolState::Error { .. } => "error",
    }
}

fn model_part_created_at(part: &ModelPart, fallback_ms: i64) -> i64 {
    match part {
        ModelPart::Text {
            time: Some(time), ..
        } => time.start.or(time.end).unwrap_or(fallback_ms),
        ModelPart::Reasoning { time, .. } => time.start,
        ModelPart::Tool(tool) => match &tool.state {
            ModelToolState::Pending { .. } => fallback_ms,
            ModelToolState::Running { time, .. } => time.start,
            ModelToolState::Completed { time, .. } => time.end,
            ModelToolState::Error { time, .. } => time.end,
        },
        ModelPart::Retry(retry) => retry.time.created,
        _ => fallback_ms,
    }
}

#[derive(Debug, Serialize)]
pub(super) struct PartSummaryInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub part_type: String,
    pub created_at: i64,
    pub sort_order: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListMessagePartsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub(super) async fn list_message_parts(
    State(state): State<Arc<ServerState>>,
    Path((session_id, msg_id)): Path<(String, String)>,
    Query(query): Query<ListMessagePartsQuery>,
) -> Result<(HeaderMap, Json<Vec<PartSummaryInfo>>)> {
    let session_exists = state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?;
    if !session_exists {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.filter(|value| *value > 0).unwrap_or(200);

    // Read from in-memory session messages as runtime source of truth.
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
    let message = session
        .get_message(&msg_id)
        .ok_or_else(|| ApiError::NotFound(format!("Message not found: {}", msg_id)))?;

    let model_parts = session_message_to_unified_message(message).parts;
    let total = model_parts.len();
    let mut infos = Vec::new();
    for (idx, part) in model_parts.into_iter().enumerate().skip(offset).take(limit) {
        let (tool_name, tool_call_id, tool_status) = match &part {
            ModelPart::Tool(tool) => (
                Some(tool.tool.clone()),
                Some(tool.call_id.clone()),
                Some(model_tool_status_name(&tool.state).to_string()),
            ),
            _ => (None, None, None),
        };

        infos.push(PartSummaryInfo {
            id: part.id().unwrap_or_default().to_string(),
            part_type: model_part_type_name(&part).to_string(),
            created_at: model_part_created_at(&part, message.created_at.timestamp_millis()),
            sort_order: idx as i64,
            tool_name,
            tool_call_id,
            tool_status,
        });
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Total-Count",
        HeaderValue::from_str(&total.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Returned-Count",
        HeaderValue::from_str(&infos.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Offset",
        HeaderValue::from_str(&offset.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Limit",
        HeaderValue::from_str(&limit.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    Ok((headers, Json(infos)))
}

pub(super) async fn get_message_part(
    State(state): State<Arc<ServerState>>,
    Path((session_id, msg_id, part_id)): Path<(String, String, String)>,
) -> Result<Json<serde_json::Value>> {
    let session_exists = state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?;
    if !session_exists {
        return Err(ApiError::SessionNotFound(session_id));
    }

    // Read from in-memory session as runtime source of truth.
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
    let message = session
        .get_message(&msg_id)
        .ok_or_else(|| ApiError::NotFound(format!("Message not found: {}", msg_id)))?;
    let part = session_message_to_unified_message(message)
        .parts
        .into_iter()
        .find(|part| part.id().is_some_and(|id| id == part_id))
        .ok_or_else(|| ApiError::NotFound(format!("Part not found: {}", part_id)))?;

    Ok(Json(
        serde_json::to_value(MessagePartResponse {
            part: serde_json::to_value(part).unwrap_or(serde_json::Value::Null),
        })
        .unwrap_or(serde_json::Value::Null),
    ))
}

fn message_to_info(session_id: &str, message: &rocode_session::SessionMessage) -> MessageInfo {
    #[derive(Debug, Default, Deserialize)]
    struct MessageMetadataWire {
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        model_id: Option<String>,
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        model_provider: Option<String>,
        #[serde(default, deserialize_with = "rocode_types::deserialize_opt_f64_lossy")]
        cost: Option<f64>,
        #[serde(default, deserialize_with = "rocode_types::deserialize_opt_i64_lossy")]
        completed_at: Option<i64>,
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        agent: Option<String>,
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        mode: Option<String>,
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        finish_reason: Option<String>,
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        error: Option<String>,
    }

    fn message_metadata_wire(metadata: &HashMap<String, serde_json::Value>) -> MessageMetadataWire {
        let Ok(value) = serde_json::to_value(metadata) else {
            return MessageMetadataWire::default();
        };
        serde_json::from_value::<MessageMetadataWire>(value).unwrap_or_default()
    }

    let wire = message_metadata_wire(&message.metadata);
    let mut metadata = message.metadata.clone();
    augment_scheduler_decision_metadata_for_response(&mut metadata, message);
    let usage = message.usage.clone().unwrap_or_default();
    let model_id = wire.model_id.clone();
    let model_provider = wire.model_provider.clone();
    let model = match (model_provider.as_deref(), model_id.as_deref()) {
        (Some(provider), Some(model)) => Some(format!("{}/{}", provider, model)),
        (None, Some(model)) => Some(model.to_string()),
        _ => None,
    };
    let cost = if usage.total_cost > 0.0 {
        usage.total_cost
    } else {
        wire.cost.unwrap_or(0.0)
    };

    let model_parts = session_message_to_unified_message(message).parts;

    MessageInfo {
        id: message.id.clone(),
        session_id: session_id.to_string(),
        role: message.role,
        parts: model_parts,
        created_at: message.created_at.timestamp_millis(),
        completed_at: wire.completed_at,
        agent: wire.agent.clone(),
        model,
        mode: wire.mode.clone(),
        finish: message.finish.clone().or(wire.finish_reason.clone()),
        error: wire.error.clone(),
        cost,
        tokens: MessageTokensInfo {
            input: usage.input_tokens,
            output: usage.output_tokens,
            reasoning: usage.reasoning_tokens,
            cache_read: usage.cache_read_tokens,
            cache_write: usage.cache_write_tokens,
        },
        metadata: (!metadata.is_empty()).then_some(metadata),
    }
}

fn augment_scheduler_decision_metadata_for_response(
    metadata: &mut HashMap<String, serde_json::Value>,
    message: &rocode_session::SessionMessage,
) {
    if metadata.contains_key("scheduler_decision_title") {
        return;
    }
    #[derive(Debug, Default, Deserialize)]
    struct SchedulerStageWire {
        #[serde(
            default,
            deserialize_with = "rocode_types::deserialize_opt_string_lossy"
        )]
        scheduler_stage: Option<String>,
    }

    let stage = serde_json::to_value(&*metadata)
        .ok()
        .and_then(|value| serde_json::from_value::<SchedulerStageWire>(value).ok())
        .and_then(|wire| wire.scheduler_stage);
    let Some(stage) = stage.as_deref() else {
        return;
    };
    let text = assistant_visible_text(message);
    let Some(decision) = decision_from_stage_text(stage, &text) else {
        return;
    };

    metadata.insert(
        "scheduler_decision_kind".to_string(),
        serde_json::to_value(decision.kind).unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "scheduler_decision_title".to_string(),
        serde_json::to_value(decision.title).unwrap_or(serde_json::Value::Null),
    );
    let spec = SchedulerDecisionSpecView {
        version: &decision.spec.version,
        show_header_divider: decision.spec.show_header_divider,
        field_order: &decision.spec.field_order,
        field_label_emphasis: &decision.spec.field_label_emphasis,
        status_palette: &decision.spec.status_palette,
        section_spacing: &decision.spec.section_spacing,
        update_policy: &decision.spec.update_policy,
    };
    metadata.insert(
        "scheduler_decision_spec".to_string(),
        serde_json::to_value(spec).unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "scheduler_decision_fields".to_string(),
        serde_json::to_value(
            decision
                .fields
                .iter()
                .map(|field| SchedulerDecisionFieldView {
                    label: &field.label,
                    value: &field.value,
                    tone: field.tone.as_deref(),
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "scheduler_decision_sections".to_string(),
        serde_json::to_value(
            decision
                .sections
                .iter()
                .map(|section| SchedulerDecisionSectionView {
                    title: &section.title,
                    body: &section.body,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or(serde_json::Value::Null),
    );
}

pub(super) async fn resolve_provider_and_model(
    state: &ServerState,
    request_model: Option<&str>,
    config_model: Option<&str>,
    config_provider: Option<&str>,
) -> Result<(Arc<dyn rocode_provider::Provider>, String, String)> {
    let providers = state.providers.read().await;
    let resolve_from_model = |model: &str| -> Result<(String, String)> {
        providers
            .parse_model_string(model)
            .ok_or_else(|| ApiError::BadRequest(format!("Model not found: {}", model)))
    };

    let (provider_id, model_id) = if let Some(model) = request_model {
        resolve_from_model(model)?
    } else if let Some(model) = config_model {
        if let Some(provider_id) = config_provider {
            (provider_id.to_string(), model.to_string())
        } else {
            resolve_from_model(model)?
        }
    } else {
        let first = providers
            .list_models()
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::BadRequest("No providers configured".to_string()))?;
        (first.provider, first.id)
    };

    let provider = providers
        .get_provider(&provider_id)
        .map_err(|e| ApiError::ProviderError(e.to_string()))?;
    if provider.get_model(&model_id).is_none() {
        return Err(ApiError::BadRequest(format!(
            "Model `{}` not found for provider `{}`",
            model_id, provider_id
        )));
    }

    Ok((provider, provider_id, model_id))
}

pub(super) async fn send_message(
    State(state): State<Arc<ServerState>>,
    Path(session_id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<MessageInfo>> {
    if !state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?
    {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let mut sessions = state.sessions.lock().await;
    let assistant_msg = sessions
        .mutate_session(&session_id, |session| {
            session.add_user_message(&req.content);
            if let Some(variant) = req.variant.as_deref() {
                session
                    .metadata
                    .insert("model_variant".to_string(), serde_json::json!(variant));
            }
            session.add_assistant_message().clone()
        })
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
    let info = message_to_info(&session_id, &assistant_msg);
    drop(sessions);
    state.touch_session_cache(&session_id).await;
    persist_sessions_if_enabled(&state).await;
    Ok(Json(info))
}

pub(super) async fn list_messages(
    State(state): State<Arc<ServerState>>,
    Path(session_id): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> Result<(HeaderMap, Json<Vec<MessageInfo>>)> {
    state
        .api_perf
        .list_messages_calls
        .fetch_add(1, Ordering::Relaxed);
    if query.after.is_some() {
        state
            .api_perf
            .list_messages_incremental_calls
            .fetch_add(1, Ordering::Relaxed);
    } else {
        state
            .api_perf
            .list_messages_full_calls
            .fetch_add(1, Ordering::Relaxed);
    }

    if query.after.is_some() && query.offset.is_some() {
        return Err(ApiError::BadRequest(
            "Query parameters `after` and `offset` are mutually exclusive".to_string(),
        ));
    }

    let session_exists = state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?;
    if !session_exists {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
    let all_messages = session.messages.clone();

    let total = all_messages.len();
    let limit = query.limit.filter(|value| *value > 0);
    let start_offset = if let Some(after) = query.after.as_deref() {
        all_messages
            .iter()
            .position(|m| m.id == after)
            .map(|pos| pos.saturating_add(1))
            // If the anchor message is unknown, fall back to a full list so clients can recover.
            .unwrap_or(0)
    } else {
        query.offset.unwrap_or(0)
    };

    let mut messages = Vec::new();
    for message in all_messages.iter().skip(start_offset) {
        messages.push(message_to_info(&session_id, message));
        if let Some(limit) = limit {
            if messages.len() >= limit {
                break;
            }
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Total-Count",
        HeaderValue::from_str(&total.to_string()).unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Returned-Count",
        HeaderValue::from_str(&messages.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    headers.insert(
        "X-Offset",
        HeaderValue::from_str(&start_offset.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    if let Some(limit) = limit {
        headers.insert(
            "X-Limit",
            HeaderValue::from_str(&limit.to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
    }

    Ok((headers, Json(messages)))
}

#[derive(Debug, Deserialize)]
pub(super) struct ListMessagesQuery {
    pub after: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub(super) async fn delete_message(
    State(state): State<Arc<ServerState>>,
    Path((session_id, msg_id)): Path<(String, String)>,
) -> Result<Json<MessageDeletedResponse>> {
    if !state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?
    {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let mut sessions = state.sessions.lock().await;
    if sessions.get(&session_id).is_none() {
        return Err(ApiError::SessionNotFound(session_id));
    }
    if sessions.remove_message(&session_id, &msg_id).is_none() {
        return Err(ApiError::NotFound(format!("Message not found: {}", msg_id)));
    }
    drop(sessions);
    state.touch_session_cache(&session_id).await;
    persist_sessions_if_enabled(&state).await;
    Ok(Json(MessageDeletedResponse { deleted: true }))
}

#[derive(Debug, Deserialize)]
pub(super) struct AddPartRequest {
    pub part: ModelPart,
}

fn expected_kind_for_model_part(part: &ModelPart) -> rocode_session::PartKind {
    match part {
        ModelPart::Text { .. } => rocode_session::PartKind::Text,
        ModelPart::Reasoning { .. } => rocode_session::PartKind::Reasoning,
        ModelPart::File(_) => rocode_session::PartKind::File,
        ModelPart::Tool(tool) => match &tool.state {
            ModelToolState::Pending { .. } | ModelToolState::Running { .. } => {
                rocode_session::PartKind::ToolCall
            }
            ModelToolState::Completed { .. } | ModelToolState::Error { .. } => {
                rocode_session::PartKind::ToolResult
            }
        },
        ModelPart::StepStart(_) => rocode_session::PartKind::StepStart,
        ModelPart::StepFinish(_) => rocode_session::PartKind::StepFinish,
        ModelPart::Snapshot { .. } => rocode_session::PartKind::Snapshot,
        ModelPart::Patch { .. } => rocode_session::PartKind::Patch,
        ModelPart::Agent(_) => rocode_session::PartKind::Agent,
        ModelPart::Subtask(_) => rocode_session::PartKind::Subtask,
        ModelPart::Retry(_) => rocode_session::PartKind::Retry,
        ModelPart::Compaction(_) => rocode_session::PartKind::Compaction,
    }
}

fn build_message_part(payload: ModelPart, msg_id: &str) -> Result<rocode_session::MessagePart> {
    let created_at = chrono::Utc::now();
    let expected_kind = expected_kind_for_model_part(&payload);
    let mut session_parts = unified_parts_to_session(vec![payload], created_at, msg_id);
    if session_parts.is_empty() {
        return Err(ApiError::InternalError(
            "failed to convert part for storage".to_string(),
        ));
    }
    let idx = session_parts
        .iter()
        .position(|part| part.kind() == expected_kind)
        .unwrap_or(0);
    let mut part = session_parts.swap_remove(idx);
    if part.id.trim().is_empty() {
        part.id = format!("prt_{}", uuid::Uuid::new_v4().simple());
    }
    part.message_id = Some(msg_id.to_string());
    Ok(part)
}

pub(super) async fn add_message_part(
    State(state): State<Arc<ServerState>>,
    Path((session_id, msg_id)): Path<(String, String)>,
    Json(req): Json<AddPartRequest>,
) -> Result<Json<AddPartResponse>> {
    if !state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?
    {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let part = build_message_part(req.part, &msg_id)?;
    let part_id = part.id.clone();

    let mut sessions = state.sessions.lock().await;
    if sessions.get(&session_id).is_none() {
        return Err(ApiError::SessionNotFound(session_id));
    }
    if sessions.update_part(&session_id, &msg_id, part).is_none() {
        return Err(ApiError::NotFound(format!("Message not found: {}", msg_id)));
    }
    drop(sessions);
    state.touch_session_cache(&session_id).await;
    persist_sessions_if_enabled(&state).await;

    Ok(Json(AddPartResponse {
        added: true,
        session_id,
        message_id: msg_id,
        part_id,
    }))
}

pub(super) async fn delete_part(
    State(state): State<Arc<ServerState>>,
    Path((session_id, msg_id, part_id)): Path<(String, String, String)>,
) -> Result<Json<DeletePartResponse>> {
    if !state
        .ensure_session_hydrated(&session_id)
        .await
        .map_err(|err| ApiError::InternalError(format!("failed to hydrate session: {}", err)))?
    {
        return Err(ApiError::SessionNotFound(session_id));
    }

    let mut sessions = state.sessions.lock().await;
    if sessions.get(&session_id).is_none() {
        return Err(ApiError::SessionNotFound(session_id));
    }
    if sessions
        .remove_part(&session_id, &msg_id, &part_id)
        .is_none()
    {
        return Err(ApiError::NotFound(format!("Part not found: {}", part_id)));
    }
    drop(sessions);
    state.touch_session_cache(&session_id).await;
    persist_sessions_if_enabled(&state).await;

    Ok(Json(DeletePartResponse {
        deleted: true,
        session_id,
        message_id: msg_id,
        part_id,
    }))
}
