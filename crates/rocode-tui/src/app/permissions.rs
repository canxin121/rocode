use std::collections::HashSet;

use crate::components::{PermissionRequest, PermissionType};
use serde::Deserialize;

use super::App;

impl App {
    fn permission_type_from_name(name: &str) -> PermissionType {
        match name {
            "read" => PermissionType::ReadFile,
            "write" => PermissionType::WriteFile,
            "edit" => PermissionType::Edit,
            "bash" => PermissionType::Bash,
            "glob" => PermissionType::Glob,
            "grep" => PermissionType::Grep,
            "list" => PermissionType::List,
            "task" | "task_flow" => PermissionType::Task,
            "webfetch" => PermissionType::WebFetch,
            "websearch" => PermissionType::WebSearch,
            "codesearch" => PermissionType::CodeSearch,
            "external_directory" => PermissionType::ExternalDirectory,
            _ => PermissionType::ExecuteCommand,
        }
    }

    fn permission_request_to_prompt(
        permission: &crate::api::PermissionRequestInfo,
    ) -> PermissionRequest {
        #[derive(Debug, Default, Deserialize)]
        struct PermissionRequestMetadataWire {
            #[serde(
                default,
                deserialize_with = "rocode_types::deserialize_opt_string_lossy"
            )]
            command: Option<String>,
            #[serde(
                default,
                deserialize_with = "rocode_types::deserialize_opt_string_lossy"
            )]
            filepath: Option<String>,
            #[serde(
                default,
                deserialize_with = "rocode_types::deserialize_opt_string_lossy"
            )]
            path: Option<String>,
        }

        #[derive(Debug, Default, Deserialize)]
        struct PermissionRequestInputWire {
            #[serde(
                default,
                deserialize_with = "rocode_types::deserialize_opt_string_lossy"
            )]
            permission: Option<String>,
            #[serde(default, deserialize_with = "rocode_types::deserialize_vec_string_lossy")]
            patterns: Vec<String>,
            #[serde(default)]
            metadata: Option<PermissionRequestMetadataWire>,
        }

        let input: PermissionRequestInputWire = rocode_types::parse_value_lossy(&permission.input);
        let permission_name = input.permission.as_deref().unwrap_or(permission.tool.as_str());
        let resource = (!input.patterns.is_empty())
            .then(|| input.patterns.join(", "))
            .or_else(|| {
                input
                    .metadata
                    .and_then(|metadata| metadata.command.or(metadata.filepath).or(metadata.path))
            })
            .unwrap_or_else(|| permission.message.clone());

        PermissionRequest {
            id: permission.id.clone(),
            permission_type: Self::permission_type_from_name(permission_name),
            resource,
            tool_name: permission_name.to_string(),
        }
    }

    pub(super) fn sync_permission_requests(&mut self) -> bool {
        let Some(client) = self.context.get_api_client() else {
            self.context.set_pending_permissions(0);
            return false;
        };

        let active_session = self.current_session_id();
        let mut permissions = match client.list_permissions() {
            Ok(items) => items,
            Err(error) => {
                tracing::debug!(%error, "failed to sync permission requests");
                return false;
            }
        };

        if let Some(session_id) = active_session.as_deref() {
            permissions.retain(|permission| permission.session_id == session_id);
        }
        permissions.sort_by(|a, b| a.id.cmp(&b.id));

        let latest_ids = permissions
            .iter()
            .map(|permission| permission.id.clone())
            .collect::<HashSet<_>>();
        let mut changed = latest_ids != self.pending_permission_ids;

        for permission in permissions {
            let permission_id = permission.id.clone();
            if self.pending_permission_ids.insert(permission_id.clone()) {
                self.permission_prompt
                    .add_request(Self::permission_request_to_prompt(&permission));
                changed = true;
            }
            self.pending_permissions.insert(permission_id, permission);
        }

        self.pending_permission_ids
            .retain(|id| latest_ids.contains(id));
        self.pending_permissions
            .retain(|id, _| latest_ids.contains(id));
        self.permission_prompt
            .retain_requests(|request| latest_ids.contains(&request.id));

        changed
    }

    pub(super) fn resolve_permission_request(
        &mut self,
        permission_id: &str,
        reply: &str,
        message: Option<String>,
    ) {
        let Some(client) = self.context.get_api_client() else {
            self.alert_dialog
                .set_message("Cannot answer permission request: no API client");
            self.alert_dialog.open();
            return;
        };

        match client.reply_permission(permission_id, reply, message) {
            Ok(()) => {
                self.pending_permission_ids.remove(permission_id);
                self.pending_permissions.remove(permission_id);
                self.permission_prompt.remove_request(permission_id);
                self.toast.show(
                    crate::components::ToastVariant::Success,
                    "Permission updated",
                    2000,
                );
            }
            Err(error) => {
                self.alert_dialog
                    .set_message(&format!("Failed to submit permission response:\n{}", error));
                self.alert_dialog.open();
            }
        }
    }

    pub(super) fn enqueue_permission_request(
        &mut self,
        permission: crate::api::PermissionRequestInfo,
    ) {
        if self.pending_permission_ids.insert(permission.id.clone()) {
            self.permission_prompt
                .add_request(Self::permission_request_to_prompt(&permission));
        }
        self.pending_permissions
            .insert(permission.id.clone(), permission);
    }

    pub(super) fn clear_permission_request(&mut self, permission_id: &str) {
        self.pending_permission_ids.remove(permission_id);
        self.pending_permissions.remove(permission_id);
        self.permission_prompt.remove_request(permission_id);
    }
}
