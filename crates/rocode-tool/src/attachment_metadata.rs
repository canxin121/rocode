use crate::Metadata;
use serde::Deserialize;

pub(crate) fn collect_attachments_from_metadata(metadata: &Metadata) -> Vec<serde_json::Value> {
    let mut attachments = Vec::new();

    let mut push_unique = |value: serde_json::Value| {
        if !attachments.iter().any(|existing| existing == &value) {
            attachments.push(value);
        }
    };

    #[derive(Debug, Deserialize, Default)]
    struct AttachmentMetadataWire {
        #[serde(default)]
        attachments: Option<serde_json::Value>,
        #[serde(default)]
        attachment: Option<serde_json::Value>,
    }

    let wire: AttachmentMetadataWire = rocode_types::parse_map_lossy(metadata);

    if let Some(value) = wire.attachments {
        match value {
            serde_json::Value::Array(array) => {
                for item in array {
                    push_unique(item);
                }
            }
            other => push_unique(other),
        }
    }
    if let Some(value) = wire.attachment {
        push_unique(value);
    }
    attachments
}

pub(crate) fn strip_attachments_from_metadata(metadata: &Metadata) -> Metadata {
    let mut sanitized = metadata.clone();
    sanitized.remove("attachments");
    sanitized.remove("attachment");
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_attachments_reads_both_plural_and_singular_keys() {
        let mut metadata = Metadata::new();
        metadata.insert(
            "attachments".to_string(),
            serde_json::json!([{ "mime": "application/pdf", "url": "data:application/pdf;base64,AA==" }]),
        );
        metadata.insert(
            "attachment".to_string(),
            serde_json::json!({ "mime": "image/png", "url": "data:image/png;base64,BB==" }),
        );

        let attachments = collect_attachments_from_metadata(&metadata);
        assert_eq!(attachments.len(), 2);
    }

    #[test]
    fn collect_attachments_deduplicates_identical_payloads() {
        let attachment = serde_json::json!({ "mime": "application/pdf", "url": "data:application/pdf;base64,AA==" });
        let mut metadata = Metadata::new();
        metadata.insert(
            "attachments".to_string(),
            serde_json::json!([attachment.clone()]),
        );
        metadata.insert("attachment".to_string(), attachment);

        let attachments = collect_attachments_from_metadata(&metadata);
        assert_eq!(attachments.len(), 1);
    }

    #[test]
    fn strip_attachments_removes_attachment_payload_keys() {
        let mut metadata = Metadata::new();
        metadata.insert("foo".to_string(), serde_json::json!("bar"));
        metadata.insert(
            "attachments".to_string(),
            serde_json::json!([{ "mime": "application/pdf", "url": "data:application/pdf;base64,AA==" }]),
        );
        metadata.insert(
            "attachment".to_string(),
            serde_json::json!({ "mime": "image/png", "url": "data:image/png;base64,BB==" }),
        );

        let sanitized = strip_attachments_from_metadata(&metadata);
        #[derive(Debug, Deserialize, Default)]
        struct FooWire {
            #[serde(default)]
            foo: Option<String>,
        }

        let wire: FooWire = rocode_types::parse_map_lossy(&sanitized);
        assert_eq!(wire.foo.as_deref(), Some("bar"));
        assert!(!sanitized.contains_key("attachments"));
        assert!(!sanitized.contains_key("attachment"));
    }
}
