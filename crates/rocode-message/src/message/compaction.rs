use super::{MessageInfo, MessageWithParts, Part};

/// Filter messages down to the window after the last compaction boundary.
///
/// The TS equivalent (`MessageV2.filterCompacted`) accepts an `AsyncIterable`
/// produced by the paginated `MessageV2.stream()` generator, which allows lazy
/// loading and early termination. In Rust we accept a pre-loaded `Vec` instead.
/// This is an intentional design choice: Rust's ownership model and the SQLite
/// backend make eager loading into a Vec both simpler and efficient enough for
/// typical session sizes. The functional semantics (newest-first iteration,
/// early break on compaction boundary, final reverse) are identical.
pub async fn filter_compacted(messages: Vec<MessageWithParts>) -> Vec<MessageWithParts> {
    let mut result = Vec::new();
    let mut completed = std::collections::HashSet::new();

    for msg in messages {
        match &msg.info {
            MessageInfo::User { id, .. } => {
                let has_compaction = msg.parts.iter().any(|p| matches!(p, Part::Compaction(_)));
                if completed.contains(id) && has_compaction {
                    result.push(msg);
                    break;
                }
            }
            MessageInfo::Assistant {
                summary,
                finish,
                parent_id,
                ..
            } => {
                if summary.is_some() && (finish.is_some() || msg.finish.is_some()) {
                    completed.insert(parent_id.clone());
                }
            }
            MessageInfo::System { .. } | MessageInfo::Tool { .. } => {}
        }
        result.push(msg);
    }

    result.reverse();
    result
}
