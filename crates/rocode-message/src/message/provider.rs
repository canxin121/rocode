use serde_json::Value;
use std::collections::HashMap;

use rocode_provider::{
    Content, ContentPart, ImageUrl, Message as ProviderMessage, Role,
    ToolResult as ProviderToolResult, ToolUse,
};

use super::{FilePart, MessageError, MessageInfo, MessageWithParts, Part, ToolState};

/// Minimal model context needed by [`to_model_messages`].
///
/// Callers construct this from whatever provider/model representation they have.
#[derive(Debug, Clone)]
pub struct ModelContext {
    /// The provider identifier, e.g. `"anthropic"`.
    pub provider_id: String,
    /// The model identifier, e.g. `"claude-sonnet-4-20250514"`.
    pub model_id: String,
    /// The npm SDK package name used by the provider, e.g. `"@ai-sdk/anthropic"`.
    /// Used to decide whether media can be inlined in tool results.
    pub api_npm: String,
    /// The provider-level API id (used for Gemini version checks).
    pub api_id: String,
}

pub fn model_context_from_ids(provider_id: &str, model_id: &str) -> ModelContext {
    let api_npm = match provider_id {
        "anthropic" => "@ai-sdk/anthropic",
        "openai" => "@ai-sdk/openai",
        "bedrock" | "amazon-bedrock" => "@ai-sdk/amazon-bedrock",
        "google" | "gemini" => "@ai-sdk/google",
        _ => "",
    };

    ModelContext {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        api_npm: api_npm.to_string(),
        api_id: model_id.to_string(),
    }
}

/// Determines whether the given provider SDK supports media (images, PDFs)
/// directly inside tool result content blocks.
///
/// Providers that do NOT support this require media to be extracted and
/// re-injected as a separate user message.
fn supports_media_in_tool_results(api_npm: &str, api_id: &str) -> bool {
    match api_npm {
        "@ai-sdk/anthropic"
        | "@ai-sdk/openai"
        | "@ai-sdk/amazon-bedrock"
        | "@ai-sdk/google-vertex/anthropic" => true,
        "@ai-sdk/google" => {
            let lower = api_id.to_lowercase();
            lower.contains("gemini-3") && !lower.contains("gemini-2")
        }
        _ => false,
    }
}

/// Extract the base64 payload from a `data:` URL, stripping the prefix.
// TODO: Wire for file attachment support
#[allow(dead_code)]
fn extract_base64_data(url: &str) -> &str {
    if let Some(comma_idx) = url.find(',') {
        &url[comma_idx + 1..]
    } else {
        url
    }
}

/// Convert a tool output value into provider-level content parts.
///
/// The TS `toModelOutput` helper handles three shapes:
/// - plain string  -> single text tool result
/// - object with `text` + optional `attachments` -> text + media parts
/// - anything else -> JSON-serialised text
fn tool_output_to_content_parts(
    output: &str,
    attachments: &[FilePart],
    compacted: bool,
    supports_media: bool,
) -> (String, Vec<ContentPart>, Vec<MediaAttachment>) {
    let text = if compacted {
        "[Old tool result content cleared]".to_string()
    } else {
        output.to_string()
    };

    let effective_attachments: Vec<&FilePart> = if compacted {
        vec![]
    } else {
        attachments.iter().collect()
    };

    let mut inline_parts = Vec::new();
    let mut deferred_media = Vec::new();

    for att in &effective_attachments {
        let is_media = att.mime.starts_with("image/") || att.mime == "application/pdf";
        if is_media && !supports_media {
            deferred_media.push(MediaAttachment {
                mime: att.mime.clone(),
                url: att.url.clone(),
            });
        } else if is_media
            && supports_media
            && att.url.starts_with("data:")
            && att.url.contains(',')
        {
            inline_parts.push(ContentPart {
                content_type: "image_url".to_string(),
                image_url: Some(ImageUrl {
                    url: att.url.clone(),
                }),
                media_type: Some(att.mime.clone()),
                ..Default::default()
            });
        }
    }

    (text, inline_parts, deferred_media)
}

/// A media attachment that could not be inlined in a tool result and must be
/// sent as a separate user message.
#[derive(Debug, Clone)]
struct MediaAttachment {
    mime: String,
    url: String,
}

/// Convert a sequence of [`MessageWithParts`] into provider-level
/// [`ProviderMessage`]s suitable for sending to an LLM API.
///
/// This is the Rust equivalent of the TS `MessageV2.toModelMessages()`.
///
/// The function:
/// - Filters ignored text parts and non-media file parts from user messages.
/// - Converts assistant text / reasoning / tool parts into the provider format.
/// - Handles tool state (completed, error, pending/running interruption).
/// - Extracts media from tool results for providers that don't support inline
///   media, injecting them as separate user messages.
/// - Skips assistant messages that have errors (unless aborted with real parts).
pub fn to_model_messages(input: &[MessageWithParts], model: &ModelContext) -> Vec<ProviderMessage> {
    let mut result: Vec<ProviderMessage> = Vec::new();
    let supports_media = supports_media_in_tool_results(&model.api_npm, &model.api_id);
    let model_key = format!("{}/{}", model.provider_id, model.model_id);

    for msg in input {
        if msg.parts.is_empty() {
            continue;
        }

        match &msg.info {
            // ---------------------------------------------------------------
            // User messages
            // ---------------------------------------------------------------
            MessageInfo::User { .. } => {
                let mut parts: Vec<ContentPart> = Vec::new();

                for part in &msg.parts {
                    match part {
                        Part::Text { text, ignored, .. } => {
                            if ignored != &Some(true) {
                                parts.push(ContentPart {
                                    content_type: "text".to_string(),
                                    text: Some(text.clone()),
                                    ..Default::default()
                                });
                            }
                        }
                        Part::File(fp) => {
                            if fp.mime != "text/plain" && fp.mime != "application/x-directory" {
                                parts.push(ContentPart {
                                    content_type: "file".to_string(),
                                    image_url: Some(ImageUrl {
                                        url: fp.url.clone(),
                                    }),
                                    media_type: Some(fp.mime.clone()),
                                    filename: fp.filename.clone(),
                                    ..Default::default()
                                });
                            }
                        }
                        Part::Compaction(_) => {
                            parts.push(ContentPart {
                                content_type: "text".to_string(),
                                text: Some("What did we do so far?".to_string()),
                                ..Default::default()
                            });
                        }
                        Part::Subtask(_) => {
                            parts.push(ContentPart {
                                content_type: "text".to_string(),
                                text: Some(
                                    "The following tool was executed by the user".to_string(),
                                ),
                                ..Default::default()
                            });
                        }
                        _ => {}
                    }
                }

                if !parts.is_empty() {
                    result.push(ProviderMessage {
                        role: Role::User,
                        content: Content::Parts(parts),
                        cache_control: None,
                        provider_options: None,
                    });
                }
            }

            // ---------------------------------------------------------------
            // Assistant messages
            // ---------------------------------------------------------------
            MessageInfo::Assistant {
                provider_id,
                model_id: msg_model_id,
                error,
                ..
            } => {
                let different_model = model_key != format!("{}/{}", provider_id, msg_model_id);

                // Skip messages with errors, unless it's an AbortedError and
                // the message has substantive parts (not just step-start / reasoning).
                if let Some(err) = error {
                    let is_aborted = matches!(err, MessageError::AbortedError { .. });
                    let has_real_parts = msg
                        .parts
                        .iter()
                        .any(|p| !matches!(p, Part::StepStart(_) | Part::Reasoning { .. }));
                    if !(is_aborted && has_real_parts) {
                        continue;
                    }
                }

                let mut assistant_parts: Vec<ContentPart> = Vec::new();
                let mut tool_results: Vec<ProviderMessage> = Vec::new();
                let mut pending_media: Vec<MediaAttachment> = Vec::new();

                for part in &msg.parts {
                    match part {
                        Part::Text { text, metadata, .. } => {
                            let provider_meta = if different_model {
                                None
                            } else {
                                metadata.as_ref().map(|m| {
                                    m.iter()
                                        .map(|(k, v)| (k.clone(), v.clone()))
                                        .collect::<HashMap<String, Value>>()
                                })
                            };
                            assistant_parts.push(ContentPart {
                                content_type: "text".to_string(),
                                text: Some(text.clone()),
                                provider_options: provider_meta,
                                ..Default::default()
                            });
                        }
                        Part::StepStart(_) => {
                            // step-start is a UI-only marker; skip for model messages
                        }
                        Part::Tool(tp) => {
                            let call_meta = if different_model {
                                None
                            } else {
                                tp.metadata.clone()
                            };
                            // Emit the tool_use on the assistant side
                            assistant_parts.push(ContentPart {
                                content_type: "tool_use".to_string(),
                                tool_use: Some(ToolUse {
                                    id: tp.call_id.clone(),
                                    name: tp.tool.clone(),
                                    input: match &tp.state {
                                        ToolState::Pending { input, .. }
                                        | ToolState::Running { input, .. }
                                        | ToolState::Completed { input, .. }
                                        | ToolState::Error { input, .. } => input.clone(),
                                    },
                                }),
                                provider_options: call_meta.clone(),
                                ..Default::default()
                            });

                            // Emit the corresponding tool result
                            match &tp.state {
                                ToolState::Completed {
                                    output,
                                    attachments,
                                    time,
                                    ..
                                } => {
                                    let compacted = time.compacted.is_some();
                                    let atts = attachments.as_deref().unwrap_or(&[]);
                                    let (text, _inline, media) = tool_output_to_content_parts(
                                        output,
                                        atts,
                                        compacted,
                                        supports_media,
                                    );

                                    pending_media.extend(media);

                                    tool_results.push(ProviderMessage {
                                        role: Role::Tool,
                                        content: Content::Parts(vec![ContentPart {
                                            content_type: "tool_result".to_string(),
                                            text: Some(text),
                                            tool_result: Some(ProviderToolResult {
                                                tool_use_id: tp.call_id.clone(),
                                                content: output.clone(),
                                                is_error: Some(false),
                                            }),
                                            ..Default::default()
                                        }]),
                                        cache_control: None,
                                        provider_options: None,
                                    });
                                }
                                ToolState::Error { error, .. } => {
                                    tool_results.push(ProviderMessage {
                                        role: Role::Tool,
                                        content: Content::Parts(vec![ContentPart {
                                            content_type: "tool_result".to_string(),
                                            text: Some(error.clone()),
                                            tool_result: Some(ProviderToolResult {
                                                tool_use_id: tp.call_id.clone(),
                                                content: error.clone(),
                                                is_error: Some(true),
                                            }),
                                            ..Default::default()
                                        }]),
                                        cache_control: None,
                                        provider_options: None,
                                    });
                                }
                                ToolState::Pending { .. } | ToolState::Running { .. } => {
                                    tool_results.push(ProviderMessage {
                                        role: Role::Tool,
                                        content: Content::Parts(vec![ContentPart {
                                            content_type: "tool_result".to_string(),
                                            text: Some(
                                                "[Tool execution was interrupted]".to_string(),
                                            ),
                                            tool_result: Some(ProviderToolResult {
                                                tool_use_id: tp.call_id.clone(),
                                                content: "[Tool execution was interrupted]"
                                                    .to_string(),
                                                is_error: Some(true),
                                            }),
                                            ..Default::default()
                                        }]),
                                        cache_control: None,
                                        provider_options: None,
                                    });
                                }
                            }
                        }
                        Part::Reasoning { text, metadata, .. } => {
                            let provider_meta = if different_model {
                                None
                            } else {
                                metadata.as_ref().map(|m| {
                                    m.iter()
                                        .map(|(k, v)| (k.clone(), v.clone()))
                                        .collect::<HashMap<String, Value>>()
                                })
                            };
                            assistant_parts.push(ContentPart {
                                content_type: "reasoning".to_string(),
                                text: Some(text.clone()),
                                provider_options: provider_meta,
                                ..Default::default()
                            });
                        }
                        _ => {}
                    }
                }

                // Only emit the assistant message if it has content
                if !assistant_parts.is_empty() {
                    result.push(ProviderMessage {
                        role: Role::Assistant,
                        content: Content::Parts(assistant_parts),
                        cache_control: None,
                        provider_options: None,
                    });

                    // Append tool result messages
                    result.extend(tool_results);

                    // Inject deferred media as a separate user message
                    if !pending_media.is_empty() {
                        let mut media_parts = vec![ContentPart {
                            content_type: "text".to_string(),
                            text: Some("Attached image(s) from tool result:".to_string()),
                            ..Default::default()
                        }];
                        for att in &pending_media {
                            media_parts.push(ContentPart {
                                content_type: "file".to_string(),
                                image_url: Some(ImageUrl {
                                    url: att.url.clone(),
                                }),
                                media_type: Some(att.mime.clone()),
                                ..Default::default()
                            });
                        }
                        result.push(ProviderMessage {
                            role: Role::User,
                            content: Content::Parts(media_parts),
                            cache_control: None,
                            provider_options: None,
                        });
                    }
                }
            }

            // ---------------------------------------------------------------
            // System messages
            // ---------------------------------------------------------------
            MessageInfo::System { .. } => {
                let text = msg
                    .parts
                    .iter()
                    .filter_map(|part| match part {
                        Part::Text { text, ignored, .. } if ignored != &Some(true) => {
                            Some(text.clone())
                        }
                        Part::Reasoning { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if !text.trim().is_empty() {
                    result.push(ProviderMessage {
                        role: Role::System,
                        content: Content::Text(text),
                        cache_control: None,
                        provider_options: None,
                    });
                }
            }

            // ---------------------------------------------------------------
            // Tool messages
            // ---------------------------------------------------------------
            MessageInfo::Tool { .. } => {
                for part in &msg.parts {
                    if let Part::Tool(tp) = part {
                        match &tp.state {
                            ToolState::Completed { output, .. } => {
                                result.push(ProviderMessage {
                                    role: Role::Tool,
                                    content: Content::Parts(vec![ContentPart {
                                        content_type: "tool_result".to_string(),
                                        text: Some(output.clone()),
                                        tool_result: Some(ProviderToolResult {
                                            tool_use_id: tp.call_id.clone(),
                                            content: output.clone(),
                                            is_error: Some(false),
                                        }),
                                        ..Default::default()
                                    }]),
                                    cache_control: None,
                                    provider_options: None,
                                });
                            }
                            ToolState::Error { error, .. } => {
                                result.push(ProviderMessage {
                                    role: Role::Tool,
                                    content: Content::Parts(vec![ContentPart {
                                        content_type: "tool_result".to_string(),
                                        text: Some(error.clone()),
                                        tool_result: Some(ProviderToolResult {
                                            tool_use_id: tp.call_id.clone(),
                                            content: error.clone(),
                                            is_error: Some(true),
                                        }),
                                        ..Default::default()
                                    }]),
                                    cache_control: None,
                                    provider_options: None,
                                });
                            }
                            ToolState::Pending { .. } | ToolState::Running { .. } => {
                                result.push(ProviderMessage {
                                    role: Role::Tool,
                                    content: Content::Parts(vec![ContentPart {
                                        content_type: "tool_result".to_string(),
                                        text: Some("[Tool execution was interrupted]".to_string()),
                                        tool_result: Some(ProviderToolResult {
                                            tool_use_id: tp.call_id.clone(),
                                            content: "[Tool execution was interrupted]".to_string(),
                                            is_error: Some(true),
                                        }),
                                        ..Default::default()
                                    }]),
                                    cache_control: None,
                                    provider_options: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Filter out messages that consist only of step-start markers
    result
        .into_iter()
        .filter(|msg| match &msg.content {
            Content::Text(t) => !t.is_empty(),
            Content::Parts(parts) => parts.iter().any(|p| p.content_type != "step-start"),
        })
        .collect()
}
