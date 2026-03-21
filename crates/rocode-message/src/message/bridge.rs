use super::{CompletedTime, ErrorTime, FilePart, RunningTime, ToolState};
use crate::part::{
    CompletedTime as CanonCompletedTime, ErrorTime as CanonErrorTime,
    RunningTime as CanonRunningTime, ToolState as CanonToolState,
};

/// Convert unified message-model tool state to canonical tool state.
pub fn tool_state_to_canonical(state: &ToolState) -> CanonToolState {
    match state {
        ToolState::Pending { input, raw } => CanonToolState::Pending {
            input: input.clone(),
            raw: raw.clone(),
        },
        ToolState::Running {
            input,
            title,
            metadata,
            time,
        } => CanonToolState::Running {
            input: input.clone(),
            title: title.clone(),
            metadata: metadata.clone(),
            time: CanonRunningTime { start: time.start },
        },
        ToolState::Completed {
            input,
            output,
            title,
            metadata,
            time,
            attachments,
        } => CanonToolState::Completed {
            input: input.clone(),
            output: output.clone(),
            title: title.clone(),
            metadata: metadata.clone(),
            time: CanonCompletedTime {
                start: time.start,
                end: time.end,
                compacted: time.compacted,
            },
            attachments: attachments.as_ref().map(|files| {
                files
                    .iter()
                    .filter_map(|file| serde_json::to_value(file).ok())
                    .collect()
            }),
        },
        ToolState::Error {
            input,
            error,
            metadata,
            time,
        } => CanonToolState::Error {
            input: input.clone(),
            error: error.clone(),
            metadata: metadata.clone(),
            time: CanonErrorTime {
                start: time.start,
                end: time.end,
            },
        },
    }
}

/// Convert canonical tool state to unified message-model tool state.
pub fn canonical_tool_state_to_message(state: &CanonToolState) -> ToolState {
    match state {
        CanonToolState::Pending { input, raw } => ToolState::Pending {
            input: input.clone(),
            raw: raw.clone(),
        },
        CanonToolState::Running {
            input,
            title,
            metadata,
            time,
        } => ToolState::Running {
            input: input.clone(),
            title: title.clone(),
            metadata: metadata.clone(),
            time: RunningTime { start: time.start },
        },
        CanonToolState::Completed {
            input,
            output,
            title,
            metadata,
            time,
            attachments,
        } => {
            let files = attachments.as_ref().map(|values| {
                values
                    .iter()
                    .filter_map(|value| serde_json::from_value::<FilePart>(value.clone()).ok())
                    .collect::<Vec<_>>()
            });

            ToolState::Completed {
                input: input.clone(),
                output: output.clone(),
                title: title.clone(),
                metadata: metadata.clone(),
                time: CompletedTime {
                    start: time.start,
                    end: time.end,
                    compacted: time.compacted,
                },
                attachments: files,
            }
        }
        CanonToolState::Error {
            input,
            error,
            metadata,
            time,
        } => ToolState::Error {
            input: input.clone(),
            error: error.clone(),
            metadata: metadata.clone(),
            time: ErrorTime {
                start: time.start,
                end: time.end,
            },
        },
    }
}
