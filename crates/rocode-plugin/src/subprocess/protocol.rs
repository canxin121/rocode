//! Plugin subprocess JSON-RPC 2.0 protocol types.
//!
//! JSON-RPC 2.0 envelope types are re-exported from `rocode_core::jsonrpc`
//! (the single authority per Constitution Article 1).

// Re-export from the single authority.
pub use rocode_core::jsonrpc::{
    JsonRpcError, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
};
