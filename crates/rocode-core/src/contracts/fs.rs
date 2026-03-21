use strum_macros::{AsRefStr, Display, EnumString};

/// Shared file-system / file-watcher wire contracts.
///
/// These are used across:
/// - tools that edit files (`edit`, `write`, `apply_patch`, etc.)
/// - server/runtime components that react to file edits
///
/// Keep them stable — they are part of the cross-crate contract.

/// Bus event payload keys for file-related events.
pub mod keys {
    /// Path string field used in file bus events.
    pub const FILE: &str = "file";
    /// Event kind field used in file-watcher update events.
    pub const EVENT: &str = "event";
}

/// File watcher event kinds surfaced in `file_watcher.updated` bus events.
///
/// Wire format: lowercase strings (`"add"`, `"change"`, `"unlink"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, AsRefStr, EnumString)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum FileWatcherEventKind {
    Add,
    Change,
    Unlink,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_watcher_event_kind_round_trips() {
        let values: &[FileWatcherEventKind] = &[
            FileWatcherEventKind::Add,
            FileWatcherEventKind::Change,
            FileWatcherEventKind::Unlink,
        ];
        for value in values {
            assert_eq!(
                value.to_string().parse::<FileWatcherEventKind>().ok(),
                Some(*value)
            );
            assert_eq!(value.to_string(), value.as_ref());
        }
    }
}
