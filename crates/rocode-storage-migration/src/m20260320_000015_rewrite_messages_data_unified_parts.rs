use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;
use std::collections::HashMap;

use crate::compat_parts::try_parse_compatible_parts;
use rocode_session::message_model::session_message_to_unified_message;
use rocode_session::{Role, SessionMessage};
use tracing::{info, warn};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260320_000015_rewrite_messages_data_unified_parts"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        let backend = DbBackend::Sqlite;

        let select_stmt = Statement::from_sql_and_values(
            backend,
            "SELECT id, session_id, role, created_at, data FROM messages WHERE data IS NOT NULL"
                .to_string(),
            vec![],
        );
        let rows = conn.query_all(select_stmt).await?;

        let mut rewritten_rows = 0usize;
        let mut already_unified_rows = 0usize;
        let mut skipped_invalid_rows = 0usize;

        for row in rows {
            let message_id_num: i64 = row.try_get("", "id")?;
            let session_id_num: i64 = row.try_get("", "session_id")?;
            let message_id = message_id_num.to_string();
            let session_id = session_id_num.to_string();
            let role_raw: String = row.try_get("", "role")?;
            let message_created_at: i64 = row.try_get("", "created_at")?;
            let data: String = row.try_get("", "data")?;
            let fallback =
                DateTime::from_timestamp_millis(message_created_at).unwrap_or_else(Utc::now);
            let role = match role_raw.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => Role::Assistant,
            };

            let Some(session_parts) =
                try_parse_compatible_parts(&data, fallback, message_id.as_str())
            else {
                skipped_invalid_rows += 1;
                warn!(
                    message_id = %message_id,
                    "skipping messages.data rewrite for unsupported payload"
                );
                continue;
            };

            let session_message = SessionMessage {
                id: message_id.clone(),
                session_id: session_id.clone(),
                role,
                parts: session_parts,
                created_at: fallback,
                metadata: HashMap::new(),
                usage: None,
                finish: None,
            };
            let unified_parts = session_message_to_unified_message(&session_message).parts;
            let next_data =
                serde_json::to_string(&unified_parts).map_err(|e| DbErr::Custom(e.to_string()))?;

            if next_data == data {
                already_unified_rows += 1;
                continue;
            }

            let update_stmt = Statement::from_sql_and_values(
                backend,
                "UPDATE messages SET data = ? WHERE id = ?".to_string(),
                vec![next_data.into(), message_id_num.into()],
            );
            conn.execute(update_stmt).await?;
            rewritten_rows += 1;
        }

        info!(
            rewritten_rows,
            already_unified_rows,
            skipped_invalid_rows,
            "messages.data rewrite to unified parts completed"
        );

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
