use serde_json::{Map, Value};

use crate::{HookContext, HookEvent};

/// Build (input, output) JSON payloads for script-style hooks.
///
/// This mirrors the TS plugin-host expectations where each hook receives
/// (input, output) objects with a small set of normalized keys.
pub(crate) fn hook_io_from_context(context: &HookContext) -> (Value, Value) {
    let source = context_values(context);
    let mut input = Map::new();
    let mut output = Map::new();

    match context.event {
        HookEvent::ToolDefinition => {
            copy_first(&source, &mut input, "toolID", &["toolID"]);
            copy_first(&source, &mut output, "description", &["description"]);
            copy_first(&source, &mut output, "parameters", &["parameters"]);
        }
        HookEvent::ToolExecuteBefore => {
            copy_first(&source, &mut input, "tool", &["tool"]);
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "callID", &["callID"]);
            copy_first(&source, &mut output, "args", &["args"]);
        }
        HookEvent::ToolExecuteAfter => {
            copy_first(&source, &mut input, "tool", &["tool"]);
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "callID", &["callID"]);
            copy_first(&source, &mut input, "args", &["args"]);
            copy_first(&source, &mut input, "error", &["error"]);
            copy_first(&source, &mut output, "title", &["title"]);
            copy_first(&source, &mut output, "output", &["output"]);
            copy_first(&source, &mut output, "metadata", &["metadata"]);
        }
        HookEvent::ChatSystemTransform => {
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "model", &["model"]);
            if !input.contains_key("model") {
                if let Some(model) = synthesize_model(&source) {
                    input.insert("model".to_string(), model);
                }
            }
            copy_first(&source, &mut output, "system", &["system"]);
        }
        HookEvent::ChatMessagesTransform => {
            copy_first(&source, &mut output, "messages", &["messages"]);
        }
        HookEvent::ChatParams => {
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "agent", &["agent"]);
            copy_first(&source, &mut input, "model", &["model"]);
            if !input.contains_key("model") {
                if let Some(model) = synthesize_model(&source) {
                    input.insert("model".to_string(), model);
                }
            }
            copy_first(&source, &mut input, "provider", &["provider"]);
            if !input.contains_key("provider") {
                if let Some(provider) = synthesize_provider(&source) {
                    input.insert("provider".to_string(), provider);
                }
            }
            copy_first(&source, &mut input, "message", &["message"]);
            copy_first(&source, &mut output, "temperature", &["temperature"]);
            copy_first(&source, &mut output, "topP", &["topP"]);
            copy_first(&source, &mut output, "topK", &["topK"]);
            copy_first(&source, &mut output, "options", &["options"]);
            copy_first(&source, &mut output, "maxTokens", &["maxTokens"]);
        }
        HookEvent::ChatHeaders => {
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "agent", &["agent"]);
            copy_first(&source, &mut input, "model", &["model"]);
            if !input.contains_key("model") {
                if let Some(model) = synthesize_model(&source) {
                    input.insert("model".to_string(), model);
                }
            }
            copy_first(&source, &mut input, "provider", &["provider"]);
            if !input.contains_key("provider") {
                if let Some(provider) = synthesize_provider(&source) {
                    input.insert("provider".to_string(), provider);
                }
            }
            copy_first(&source, &mut input, "message", &["message"]);
            copy_first(&source, &mut output, "headers", &["headers"]);
        }
        HookEvent::ChatMessage => {
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "agent", &["agent"]);
            copy_first(&source, &mut input, "model", &["model"]);
            if !input.contains_key("model") {
                if let Some(model) = synthesize_model(&source) {
                    input.insert("model".to_string(), model);
                }
            }
            copy_first(&source, &mut input, "messageID", &["messageID"]);
            copy_first(&source, &mut input, "variant", &["variant"]);
            copy_first(&source, &mut input, "has_tool_calls", &["has_tool_calls"]);
            copy_first(&source, &mut output, "message", &["message"]);
            copy_first(&source, &mut output, "parts", &["parts"]);
        }
        HookEvent::SessionCompacting => {
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "auto", &["auto"]);
            copy_first(&source, &mut input, "completed", &["completed"]);
            copy_first(&source, &mut output, "context", &["context"]);
            copy_first(&source, &mut output, "prompt", &["prompt"]);
        }
        HookEvent::TextComplete => {
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "messageID", &["messageID"]);
            copy_first(&source, &mut input, "partID", &["partID"]);
            copy_first(&source, &mut output, "text", &["text"]);
        }
        HookEvent::ShellEnv => {
            copy_first(&source, &mut input, "cwd", &["cwd"]);
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "callID", &["callID"]);
            copy_first(&source, &mut output, "env", &["env"]);
        }
        HookEvent::CommandExecuteBefore => {
            copy_first(&source, &mut input, "command", &["command"]);
            copy_first(&source, &mut input, "sessionID", &["sessionID"]);
            copy_first(&source, &mut input, "arguments", &["arguments"]);
            copy_first(&source, &mut input, "source", &["source"]);
            copy_first(&source, &mut output, "parts", &["parts"]);
        }
        HookEvent::PermissionAsk => {
            copy_first(&source, &mut input, "permission", &["permission"]);
            copy_first(
                &source,
                &mut input,
                "permission_type",
                &["permission_type", "permissionType"],
            );
            copy_first(
                &source,
                &mut input,
                "permission_id",
                &["permission_id", "permissionID"],
            );
            copy_first(&source, &mut output, "status", &["status"]);
        }
        _ => {
            input = source.clone();
            output = source;
        }
    }

    seed_hook_output(context.event.clone(), &mut output);

    (Value::Object(input), Value::Object(output))
}

fn context_values(context: &HookContext) -> Map<String, Value> {
    let mut values = Map::new();
    for (key, value) in &context.data {
        values.insert(key.clone(), value.clone());
        let normalized = normalize_hook_key(key);
        if normalized != *key {
            values.entry(normalized).or_insert_with(|| value.clone());
        }
    }
    if let Some(session_id) = &context.session_id {
        values
            .entry("sessionID".to_string())
            .or_insert_with(|| Value::String(session_id.clone()));
    }
    values
}

fn first_value(source: &Map<String, Value>, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| source.get(*key).cloned())
}

fn copy_first(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    target_key: &str,
    candidate_keys: &[&str],
) {
    if let Some(value) = first_value(source, candidate_keys) {
        target.insert(target_key.to_string(), value);
    }
}

fn synthesize_model(source: &Map<String, Value>) -> Option<Value> {
    let model_id = first_value(source, &["modelID", "model_id"])?;
    let mut model = Map::new();
    model.insert("modelID".to_string(), model_id.clone());
    model.insert("id".to_string(), model_id);
    if let Some(provider_id) = first_value(source, &["providerID", "provider_id"]) {
        model.insert("providerID".to_string(), provider_id);
    }
    Some(Value::Object(model))
}

fn synthesize_provider(source: &Map<String, Value>) -> Option<Value> {
    let provider_id = first_value(source, &["providerID", "provider_id"])?;
    let mut provider = Map::new();
    provider.insert("id".to_string(), provider_id.clone());
    provider.insert(
        "info".to_string(),
        Value::Object(Map::from_iter([("id".to_string(), provider_id)])),
    );
    Some(Value::Object(provider))
}

fn normalize_hook_key(key: &str) -> String {
    match key {
        "tool_id" => "toolID".to_string(),
        "call_id" => "callID".to_string(),
        "model_id" => "modelID".to_string(),
        "provider_id" => "providerID".to_string(),
        "message_id" => "messageID".to_string(),
        "part_id" => "partID".to_string(),
        "max_tokens" => "maxTokens".to_string(),
        _ => key.to_string(),
    }
}

fn ensure_default(map: &mut Map<String, Value>, key: &str, value: Value) {
    map.entry(key.to_string()).or_insert(value);
}

fn ensure_object(map: &mut Map<String, Value>, key: &str) {
    ensure_default(map, key, Value::Object(Map::new()));
}

fn ensure_array(map: &mut Map<String, Value>, key: &str) {
    ensure_default(map, key, Value::Array(Vec::new()));
}

fn seed_hook_output(event: HookEvent, output: &mut Map<String, Value>) {
    match event {
        HookEvent::ToolDefinition => {
            ensure_default(output, "description", Value::String(String::new()));
            ensure_object(output, "parameters");
        }
        HookEvent::ToolExecuteBefore => {
            ensure_object(output, "args");
        }
        HookEvent::ToolExecuteAfter => {
            ensure_default(output, "title", Value::String(String::new()));
            ensure_default(output, "output", Value::String(String::new()));
            ensure_object(output, "metadata");
        }
        HookEvent::ChatHeaders => {
            ensure_object(output, "headers");
        }
        HookEvent::ChatParams => {
            ensure_default(output, "temperature", Value::Null);
            ensure_default(output, "topP", Value::Null);
            ensure_default(output, "topK", Value::Null);
            ensure_object(output, "options");
        }
        HookEvent::ChatMessage => {
            ensure_default(output, "message", Value::Null);
            ensure_array(output, "parts");
        }
        HookEvent::ChatMessagesTransform => {
            ensure_array(output, "messages");
        }
        HookEvent::ChatSystemTransform => {
            ensure_array(output, "system");
        }
        HookEvent::SessionCompacting => {
            ensure_array(output, "context");
        }
        HookEvent::TextComplete => {
            ensure_default(output, "text", Value::String(String::new()));
        }
        HookEvent::ShellEnv => {
            ensure_object(output, "env");
        }
        HookEvent::CommandExecuteBefore => {
            ensure_array(output, "parts");
        }
        HookEvent::PermissionAsk => {
            ensure_default(output, "status", Value::String("ask".to_string()));
        }
        _ => {}
    }
}
