//! TypeScript plugin subprocess support.
//!
//! This module provides the ability to load TS/JS plugins by spawning a
//! `plugin-host.ts` child process and communicating over Content-Length
//! framed JSON-RPC 2.0 (stdin/stdout).

pub mod auth;
pub mod client;
pub mod loader;
pub mod protocol;
pub mod runtime;

// Re-exports for convenience
pub use auth::{
    PluginAuthBridge, PluginAuthError, PluginFetchRequest, PluginFetchResponse,
    PluginFetchStreamResponse,
};
pub use client::{PluginContext, PluginSubprocess, PluginSubprocessError};
pub use loader::{
    get_tool_call_tracking, remove_tool_call_tracking, track_tool_call, PluginLoader,
    PluginLoaderError, PluginToolCallRef,
};
pub use runtime::{detect_runtime, JsRuntime};

use crate::HookEvent;

/// Map a TS hook name string to the corresponding `HookEvent` variant.
pub fn hook_name_to_event(name: &str) -> Option<HookEvent> {
    match name {
        // Core lifecycle
        "config.loaded" => Some(HookEvent::ConfigLoaded),
        "session.start" => Some(HookEvent::SessionStart),
        "session.end" => Some(HookEvent::SessionEnd),
        "tool.call" => Some(HookEvent::ToolCall),
        "tool.result" => Some(HookEvent::ToolResult),
        "message.sent" => Some(HookEvent::MessageSent),
        "message.received" => Some(HookEvent::MessageReceived),
        "error" => Some(HookEvent::Error),
        "file.change" => Some(HookEvent::FileChange),
        "provider.change" => Some(HookEvent::ProviderChange),

        // TS/OpenCode-compatible hooks
        "chat.headers" => Some(HookEvent::ChatHeaders),
        "chat.params" => Some(HookEvent::ChatParams),
        "chat.message" => Some(HookEvent::ChatMessage),
        "tool.execute.before" => Some(HookEvent::ToolExecuteBefore),
        "tool.execute.after" => Some(HookEvent::ToolExecuteAfter),
        "tool.definition" => Some(HookEvent::ToolDefinition),
        "permission.ask" => Some(HookEvent::PermissionAsk),
        "command.execute.before" => Some(HookEvent::CommandExecuteBefore),
        "shell.env" => Some(HookEvent::ShellEnv),
        "experimental.chat.system.transform" => Some(HookEvent::ChatSystemTransform),
        "experimental.chat.messages.transform" => Some(HookEvent::ChatMessagesTransform),
        "experimental.session.compacting" => Some(HookEvent::SessionCompacting),
        "experimental.text.complete" => Some(HookEvent::TextComplete),
        _ => None,
    }
}
