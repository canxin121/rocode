use sea_orm_migration::prelude::*;

mod idents;
mod m20260317_000001_create_sessions;
mod m20260317_000002_create_messages;
mod m20260317_000003_create_parts;
mod m20260317_000004_create_todos;
mod m20260317_000005_create_permissions;
mod m20260317_000006_create_session_shares;
mod m20260317_000007_create_indexes;
mod m20260317_000010_add_pagination_indexes;
mod m20260317_000011_add_part_todo_pagination_indexes;
mod m20260319_000013_int_primary_keys;
mod m20260319_000014_session_shares_session_id_pk;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260317_000001_create_sessions::Migration),
            Box::new(m20260317_000002_create_messages::Migration),
            Box::new(m20260317_000003_create_parts::Migration),
            Box::new(m20260317_000004_create_todos::Migration),
            Box::new(m20260317_000005_create_permissions::Migration),
            Box::new(m20260317_000006_create_session_shares::Migration),
            Box::new(m20260317_000007_create_indexes::Migration),
            Box::new(m20260317_000010_add_pagination_indexes::Migration),
            Box::new(m20260317_000011_add_part_todo_pagination_indexes::Migration),
            Box::new(m20260319_000013_int_primary_keys::Migration),
            Box::new(m20260319_000014_session_shares_session_id_pk::Migration),
        ]
    }
}
