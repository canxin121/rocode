use async_trait::async_trait;
use rocode_core::contracts::tools::{arg_keys as tool_arg_keys, BuiltinToolName};
use rocode_message::message::{
    CompletedTime, ErrorTime, FilePart, Part as ModelPart, RunningTime, ToolPart as ModelToolPart,
    ToolState as ModelToolState,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

use crate::attachment_metadata::{
    collect_attachments_from_metadata, strip_attachments_from_metadata,
};
use crate::{Metadata, Tool, ToolContext, ToolError, ToolResult};

const MAX_BATCH_SIZE: usize = 25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchParams {
    #[serde(default, alias = "toolCalls")]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub tool: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<serde_json::Value>,
}

pub struct BatchTool;

type BatchFuture = Pin<Box<dyn Future<Output = BatchResult> + Send>>;

#[derive(Debug, Deserialize)]
struct AttachmentWire {
    url: String,
    mime: String,
    #[serde(default)]
    filename: Option<String>,
}

#[derive(Debug, Serialize)]
struct BatchResultDetail<'a> {
    tool: &'a str,
    success: bool,
}

fn to_value_or_null<T: Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn attachment_values_to_file_parts(
    values: Vec<serde_json::Value>,
    session_id: &str,
    message_id: &str,
    call_id: &str,
) -> Vec<FilePart> {
    values
        .into_iter()
        .enumerate()
        .filter_map(|(idx, value)| {
            serde_json::from_value::<AttachmentWire>(value)
                .ok()
                .map(|item| (idx, item))
        })
        .map(|(idx, item)| FilePart {
            id: format!("att_{}_{}", call_id, idx),
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            mime: item.mime,
            url: item.url,
            filename: item.filename,
            source: None,
        })
        .collect()
}

#[async_trait]
impl Tool for BatchTool {
    fn id(&self) -> &str {
        BuiltinToolName::Batch.as_str()
    }

    fn description(&self) -> &str {
        "Execute multiple tool calls in parallel. Maximum 25 tools per batch. Use this for optimal performance when you need to run multiple independent operations."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                (tool_arg_keys::TOOL_CALLS_CAMEL): {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 25,
                    "items": {
                        "type": "object",
                        "properties": {
                            (tool_arg_keys::TOOL): {
                                "type": "string",
                                "description": "The name of the tool to execute"
                            },
                            (tool_arg_keys::PARAMETERS): {
                                "type": "object",
                                "description": "Parameters for the tool"
                            }
                        },
                        "required": [tool_arg_keys::TOOL, tool_arg_keys::PARAMETERS]
                    },
                    "description": "Array of tool calls to execute in parallel"
                }
            },
            "required": [tool_arg_keys::TOOL_CALLS_CAMEL]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: BatchParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArguments(format!("Invalid parameters: {}", e)))?;

        let total_calls = params.tool_calls.len();
        let tool_calls: Vec<_> = params.tool_calls.into_iter().take(MAX_BATCH_SIZE).collect();
        let discarded_count = total_calls.saturating_sub(MAX_BATCH_SIZE);

        if tool_calls.is_empty() {
            return Err(ToolError::ValidationError(
                "Provide at least one tool call".to_string(),
            ));
        }

        let registry = match &ctx.registry {
            Some(r) => r.clone(),
            None => {
                return Err(ToolError::ExecutionError(
                    "Tool registry not available. Batch execution requires registry access."
                        .to_string(),
                ));
            }
        };

        let mut futures: Vec<BatchFuture> = Vec::new();

        for call in tool_calls {
            if BuiltinToolName::parse(&call.tool).is_some_and(|tool| tool == BuiltinToolName::Batch)
            {
                let tool_name = call.tool.clone();
                let err_msg = format!(
                    "Tool '{}' is not allowed in batch. Disallowed: {}",
                    tool_name,
                    BuiltinToolName::Batch.as_str(),
                );
                futures.push(Box::pin(async move {
                    BatchResult {
                        tool: tool_name,
                        success: false,
                        error: Some(err_msg),
                        attachments: Vec::new(),
                    }
                }) as BatchFuture);
                continue;
            }

            let registry = registry.clone();
            let tool_name = call.tool.clone();
            let tool_params = call.parameters.clone();
            let ctx_clone = ctx.clone();
            let session_id = ctx.session_id.clone();
            let message_id = ctx.message_id.clone();
            let call_id = uuid::Uuid::new_v4().to_string();
            let call_start_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            futures.push(Box::pin(async move {
                let running_part = to_value_or_null(ModelPart::Tool(ModelToolPart {
                    id: call_id.clone(),
                    session_id: session_id.clone(),
                    message_id: message_id.clone(),
                    call_id: call_id.clone(),
                    tool: tool_name.clone(),
                    state: ModelToolState::Running {
                        input: tool_params.clone(),
                        title: None,
                        metadata: None,
                        time: RunningTime {
                            start: call_start_time,
                        },
                    },
                    metadata: None,
                }));
                let _ = ctx_clone.do_update_part(running_part).await;

                let result = match registry.get(&tool_name).await {
                    Some(tool) => {
                        match tool.execute(tool_params.clone(), ctx_clone.clone()).await {
                            Ok(res) => {
                                let call_end_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                                    as i64;

                                let metadata = strip_attachments_from_metadata(&res.metadata);
                                let attachments = collect_attachments_from_metadata(&res.metadata);
                                let attachment_parts = attachment_values_to_file_parts(
                                    attachments.clone(),
                                    &session_id,
                                    &message_id,
                                    &call_id,
                                );

                                let completed_part = to_value_or_null(ModelPart::Tool(ModelToolPart {
                                    id: call_id.clone(),
                                    session_id: session_id.clone(),
                                    message_id: message_id.clone(),
                                    call_id: call_id.clone(),
                                    tool: tool_name.clone(),
                                    state: ModelToolState::Completed {
                                        input: tool_params.clone(),
                                        output: res.output.clone(),
                                        title: res.title.clone(),
                                        metadata,
                                        time: CompletedTime {
                                            start: call_start_time,
                                            end: call_end_time,
                                            compacted: None,
                                        },
                                        attachments: (!attachment_parts.is_empty())
                                            .then_some(attachment_parts),
                                    },
                                    metadata: None,
                                }));
                                let _ = ctx_clone.do_update_part(completed_part).await;

                                BatchResult {
                                    tool: tool_name,
                                    success: true,
                                    error: None,
                                    attachments,
                                }
                            }
                            Err(e) => {
                                let call_end_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                                    as i64;

                                let error_part = to_value_or_null(ModelPart::Tool(ModelToolPart {
                                    id: call_id.clone(),
                                    session_id: session_id.clone(),
                                    message_id: message_id.clone(),
                                    call_id: call_id.clone(),
                                    tool: tool_name.clone(),
                                    state: ModelToolState::Error {
                                        input: tool_params.clone(),
                                        error: e.to_string(),
                                        metadata: None,
                                        time: ErrorTime {
                                            start: call_start_time,
                                            end: call_end_time,
                                        },
                                    },
                                    metadata: None,
                                }));
                                let _ = ctx_clone.do_update_part(error_part).await;

                                BatchResult {
                                    tool: tool_name,
                                    success: false,
                                    error: Some(e.to_string()),
                                    attachments: Vec::new(),
                                }
                            }
                        }
                    }
                    None => {
                        let available = registry.suggest_tools(&tool_name).await;
                        let err_msg = format!(
                            "Tool '{}' not in registry. External tools (MCP, environment) cannot be batched - call them directly. Available tools: {}",
                            tool_name,
                            available.join(", ")
                        );
                        BatchResult {
                            tool: tool_name.clone(),
                            success: false,
                            error: Some(err_msg),
                            attachments: Vec::new(),
                        }
                    }
                };

                result
            }) as BatchFuture);
        }

        let results: Vec<BatchResult> = futures::future::join_all(futures).await;

        let mut final_results = results;

        if discarded_count > 0 {
            final_results.push(BatchResult {
                tool: BuiltinToolName::Batch.as_str().to_string(),
                success: false,
                error: Some(format!(
                    "{} additional calls discarded (max {} per batch)",
                    discarded_count, MAX_BATCH_SIZE
                )),
                attachments: Vec::new(),
            });
        }

        let successful = final_results.iter().filter(|r| r.success).count();
        let failed = final_results.len() - successful;

        let output = if failed > 0 {
            format!(
                "Executed {}/{} tools successfully. {} failed.",
                successful,
                final_results.len(),
                failed
            )
        } else {
            format!(
                "All {} tools executed successfully.\n\nKeep using the batch tool for optimal performance in your next response!",
                successful
            )
        };

        let tools_list: Vec<&str> = final_results.iter().map(|r| r.tool.as_str()).collect();
        let aggregated_attachments: Vec<serde_json::Value> = final_results
            .iter()
            .filter(|r| r.success)
            .flat_map(|r| r.attachments.clone())
            .collect();

        let mut metadata = Metadata::new();
        metadata.insert(
            "total".to_string(),
            serde_json::Value::Number((final_results.len() as u64).into()),
        );
        metadata.insert(
            "successful".to_string(),
            serde_json::Value::Number((successful as u64).into()),
        );
        metadata.insert(
            "failed".to_string(),
            serde_json::Value::Number((failed as u64).into()),
        );
        metadata.insert(
            "tools".to_string(),
            serde_json::to_value(tools_list).unwrap_or(serde_json::Value::Null),
        );
        metadata.insert(
            "details".to_string(),
            serde_json::to_value(
                final_results
                    .iter()
                    .map(|r| BatchResultDetail {
                        tool: &r.tool,
                        success: r.success,
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or(serde_json::Value::Null),
        );
        if !aggregated_attachments.is_empty() {
            metadata.insert(
                "attachments".to_string(),
                serde_json::Value::Array(aggregated_attachments),
            );
        }

        Ok(ToolResult {
            output,
            title: format!("Batch execution ({}/{})", successful, final_results.len()),
            metadata,
            truncated: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BatchParams;
    use rocode_core::contracts::tools::BuiltinToolName;

    #[test]
    fn batch_params_accepts_camel_case_tool_calls() {
        let value = serde_json::json!({
            "toolCalls": [
                { "tool": BuiltinToolName::Read.as_str(), "parameters": { "file_path": "index.html" } }
            ]
        });
        let parsed: BatchParams = serde_json::from_value(value).expect("should parse toolCalls");
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].tool, BuiltinToolName::Read.as_str());
    }

    #[test]
    fn batch_params_defaults_tool_calls_when_missing() {
        let parsed: BatchParams =
            serde_json::from_value(serde_json::json!({})).expect("should parse empty object");
        assert!(parsed.tool_calls.is_empty());
    }
}
