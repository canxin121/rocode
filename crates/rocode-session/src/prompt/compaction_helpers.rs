use crate::message_model::{session_message_to_unified_message, Part as ModelPart};
use crate::{Session, SessionMessage};

pub fn should_compact(messages: &[SessionMessage], max_tokens: u64) -> bool {
    let total_chars: usize = messages
        .iter()
        .map(|m| {
            session_message_to_unified_message(m)
                .parts
                .into_iter()
                .filter_map(|part| match part {
                    ModelPart::Text { text, .. } => Some(text.len()),
                    _ => None,
                })
                .sum::<usize>()
        })
        .sum();

    let estimated_tokens = total_chars / 4;
    estimated_tokens > max_tokens as usize
}

pub fn trigger_compaction(session: &mut Session, messages: &[SessionMessage]) -> Option<String> {
    if !should_compact(messages, 100000) {
        return None;
    }

    let text_content: String = messages
        .iter()
        .rev()
        .take(10)
        .flat_map(|m| {
            session_message_to_unified_message(m)
                .parts
                .into_iter()
                .filter_map(|part| match part {
                    ModelPart::Text { text, .. } => Some(text),
                    _ => None,
                })
        })
        .collect::<Vec<_>>()
        .join("\n");

    let summary = format!(
        "[Context Compaction Triggered]\nRecent messages summarized:\n{}",
        text_content.chars().take(500).collect::<String>()
    );

    // Persist the compaction summary as a Compaction part on a new assistant message.
    let mut compaction_msg = SessionMessage::assistant(session.id.clone());
    compaction_msg.add_compaction(summary.clone());
    session.messages.push(compaction_msg);

    // Mark session as updated so compaction summary is persisted.
    session.touch();

    Some(summary)
}
