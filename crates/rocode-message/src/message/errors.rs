use std::collections::HashMap;

use rocode_provider::{
    parse_api_call_error, parse_stream_error, ParsedAPICallError, ParsedStreamError, ProviderError,
};

use super::MessageError;

/// Convert an arbitrary error into a [`MessageError`].
///
/// This mirrors the TS `MessageV2.fromError` function. It inspects the error
/// chain for well-known provider error types and falls back to `Unknown`.
pub fn error_from_anyhow(e: anyhow::Error, provider_id: &str) -> MessageError {
    let err_str = e.to_string();

    // 1. AbortError – the operation was cancelled / aborted.
    if err_str.contains("abort") || err_str.contains("cancelled") || err_str.contains("AbortError")
    {
        return MessageError::AbortedError { message: err_str };
    }

    // 2. OutputLengthError – model hit its max output token limit.
    if err_str.contains("output length")
        || err_str.contains("max_tokens")
        || err_str.contains("output_length")
        || err_str.contains("OutputLengthError")
    {
        return MessageError::OutputLengthError { message: err_str };
    }

    // 3. AuthError – API key / credential issues.
    if err_str.contains("auth")
        || err_str.contains("api key")
        || err_str.contains("API key")
        || err_str.contains("LoadAPIKeyError")
        || err_str.contains("unauthorized")
        || err_str.contains("Unauthorized")
    {
        return MessageError::AuthError {
            provider_id: provider_id.to_string(),
            message: err_str,
        };
    }

    // 4. ECONNRESET / connection reset.
    if err_str.contains("ECONNRESET")
        || err_str.contains("connection reset")
        || err_str.contains("Connection reset")
    {
        let mut metadata = HashMap::new();
        metadata.insert("code".to_string(), "ECONNRESET".to_string());
        metadata.insert("message".to_string(), err_str.clone());
        return MessageError::ApiError {
            message: "Connection reset by server".to_string(),
            status_code: None,
            is_retryable: true,
            response_headers: None,
            response_body: None,
            metadata: Some(metadata),
        };
    }

    // 5. Try to downcast to ProviderError for structured handling.
    if let Some(provider_err) = e.downcast_ref::<ProviderError>() {
        let parsed = parse_api_call_error(provider_id, provider_err);
        return match parsed {
            ParsedAPICallError::ContextOverflow {
                message,
                response_body,
                ..
            } => MessageError::ContextOverflowError {
                message,
                response_body,
            },
            ParsedAPICallError::ApiError {
                message,
                status_code,
                is_retryable,
                response_headers,
                response_body,
                metadata,
                ..
            } => MessageError::ApiError {
                message,
                status_code: status_code.map(|s| s as i32),
                is_retryable,
                response_headers,
                response_body,
                metadata,
            },
        };
    }

    // 6. Context overflow heuristic on the raw string.
    if ProviderError::is_overflow(&err_str) {
        return MessageError::ContextOverflowError {
            message: err_str,
            response_body: None,
        };
    }

    // 7. Try to parse as a stream error (JSON body with `type: "error"`).
    if let Some(parsed) = try_parse_stream_error(&err_str) {
        return parsed;
    }

    // 8. Generic connection / network errors that are retryable.
    if err_str.contains("connection") || err_str.contains("reset") || err_str.contains("timed out")
    {
        return MessageError::ApiError {
            message: err_str,
            status_code: None,
            is_retryable: true,
            response_headers: None,
            response_body: None,
            metadata: None,
        };
    }

    // 9. Fallback.
    MessageError::Unknown { message: err_str }
}

/// Attempt to interpret `raw` as a JSON stream error body and convert it.
fn try_parse_stream_error(raw: &str) -> Option<MessageError> {
    let parsed = parse_stream_error(raw)?;
    Some(match parsed {
        ParsedStreamError::ContextOverflow {
            message,
            response_body,
        } => MessageError::ContextOverflowError {
            message,
            response_body: Some(response_body),
        },
        ParsedStreamError::ApiError {
            message,
            is_retryable,
            response_body,
        } => MessageError::ApiError {
            message,
            status_code: None,
            is_retryable,
            response_headers: None,
            response_body: Some(response_body),
            metadata: None,
        },
    })
}
