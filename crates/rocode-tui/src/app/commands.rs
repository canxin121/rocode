use super::*;

impl App {
    pub(super) fn execute_command_action(&mut self, action: CommandAction) -> anyhow::Result<()> {
        match action {
            CommandAction::SubmitPrompt => self.submit_prompt()?,
            CommandAction::ClearPrompt => self.prompt.clear(),
            CommandAction::PasteClipboard => self.paste_clipboard_to_prompt(),
            CommandAction::CopyPrompt => self.copy_prompt_to_clipboard(),
            CommandAction::CutPrompt => self.cut_prompt_to_clipboard(),
            CommandAction::HistoryPrevious => self.prompt.history_previous_entry(),
            CommandAction::HistoryNext => self.prompt.history_next_entry(),
            CommandAction::ToggleSidebar => self.context.toggle_sidebar(),
            CommandAction::ToggleHeader => self.context.toggle_header(),
            CommandAction::ToggleScrollbar => self.context.toggle_scrollbar(),
            CommandAction::SwitchSession => {
                self.refresh_session_list_dialog();
                self.session_list_dialog
                    .open(self.active_session_id.as_deref());
            }
            CommandAction::NavigateChildSession => {
                self.navigate_to_child_session();
            }
            CommandAction::NavigateParentSession => {
                self.navigate_to_parent_session();
            }
            CommandAction::RenameSession => {
                self.open_session_rename_dialog();
            }
            CommandAction::ExportSession => {
                self.open_session_export_dialog();
            }
            CommandAction::PromptStashPush => {
                if self.prompt.stash_current() {
                    self.alert_dialog.set_message("Prompt stashed.");
                    self.alert_dialog.open();
                } else {
                    self.alert_dialog
                        .set_message("Prompt is empty, nothing to stash.");
                    self.alert_dialog.open();
                }
            }
            CommandAction::PromptStashList => {
                self.open_prompt_stash_dialog();
            }
            CommandAction::PromptSkillList => {
                self.open_skill_list_dialog();
            }
            CommandAction::SwitchTheme => {
                self.refresh_theme_list_dialog();
                let current_theme = self.context.current_theme_name();
                self.theme_list_dialog.open(&current_theme);
            }
            CommandAction::CycleVariant => {
                self.cycle_model_variant();
            }
            CommandAction::ToggleAppearance => {
                let _ = self.context.toggle_theme_mode();
            }
            CommandAction::ViewStatus => {
                self.refresh_status_dialog();
                self.status_dialog.open();
            }
            CommandAction::ToggleMcp => {
                let _ = self.refresh_mcp_dialog();
                self.mcp_dialog.open();
            }
            CommandAction::ToggleTips => {
                self.context.toggle_tips_hidden();
            }
            CommandAction::SwitchModel => {
                self.refresh_model_dialog();
                self.model_select.open();
            }
            CommandAction::SwitchAgent => {
                self.refresh_agent_dialog();
                self.agent_select.open();
            }
            CommandAction::NewSession => {
                self.context.navigate(Route::Home);
                self.active_session_id = None;
                self.session_view = None;
            }
            CommandAction::ShowHelp => {
                self.help_dialog.open();
            }
            CommandAction::ToggleCommandPalette => {
                self.sync_command_palette_labels();
                self.command_palette.open();
            }
            CommandAction::ToggleTimestamps => {
                self.context.toggle_timestamps();
            }
            CommandAction::ToggleThinking => {
                self.context.toggle_thinking();
            }
            CommandAction::ToggleToolDetails => {
                self.context.toggle_tool_details();
            }
            CommandAction::ToggleDensity => {
                self.context.toggle_message_density();
            }
            CommandAction::ToggleSemanticHighlight => {
                self.context.toggle_semantic_highlight();
            }
            CommandAction::ExternalEditor => {}
            CommandAction::ConnectProvider => {
                self.populate_provider_dialog();
                self.provider_dialog.open();
            }
            CommandAction::ShareSession => {
                self.handle_share_session();
            }
            CommandAction::UnshareSession => {
                self.handle_unshare_session();
            }
            CommandAction::ForkSession => {
                self.handle_fork_session();
            }
            CommandAction::CompactSession => {
                self.handle_compact_session();
            }
            CommandAction::Timeline => {
                self.handle_open_timeline();
            }
            CommandAction::Undo => {
                self.handle_undo();
            }
            CommandAction::Redo => {
                self.handle_redo();
            }
            CommandAction::ListSessions | CommandAction::OpenSessionList => {
                self.refresh_session_list_dialog();
                self.session_list_dialog
                    .open(self.active_session_id.as_deref());
            }
            CommandAction::CopySession => {
                self.handle_copy_session();
            }
            CommandAction::OpenStash => {
                self.open_prompt_stash_dialog();
            }
            CommandAction::OpenRecoveryList => {
                self.handle_show_recovery_actions();
            }
            CommandAction::OpenSkills => {
                self.open_skill_list_dialog();
            }
            CommandAction::OpenThemeList => {
                self.refresh_theme_list_dialog();
                let current_theme = self.context.current_theme_name();
                self.theme_list_dialog.open(&current_theme);
            }
            CommandAction::ShowStatus => {
                self.refresh_status_dialog();
                self.status_dialog.open();
            }
            CommandAction::ManageMcp | CommandAction::OpenMcpList => {
                let _ = self.refresh_mcp_dialog();
                self.mcp_dialog.open();
            }
            CommandAction::OpenModelList => {
                self.refresh_model_dialog();
                self.model_select.open();
            }
            CommandAction::OpenAgentList => {
                self.refresh_agent_dialog();
                self.agent_select.open();
            }
            CommandAction::Exit => self.state = AppState::Exiting,
            CommandAction::ListTasks => {
                self.handle_list_tasks();
            }
        }

        Ok(())
    }

    pub(super) fn execute_typed_interactive_command(
        &mut self,
        command: InteractiveCommand,
    ) -> anyhow::Result<bool> {
        match command {
            InteractiveCommand::Exit => {
                self.execute_command_action(CommandAction::Exit)?;
            }
            InteractiveCommand::ShowHelp => {
                self.execute_command_action(CommandAction::ShowHelp)?;
            }
            InteractiveCommand::Abort => {
                if let Some(session_id) = &self.active_session_id {
                    if let Some(api) = self.context.get_api_client() {
                        match api.abort_session(session_id) {
                            Err(e) => {
                                self.toast.show(
                                    ToastVariant::Error,
                                    &format!("Failed to cancel run: {}", e),
                                    3000,
                                );
                            }
                            Ok(value) => {
                                let message = value
                                    .get("target")
                                    .and_then(|value| value.as_str())
                                    .map(|target| match target {
                                        "stage" => {
                                            let stage = value
                                                .get("stage")
                                                .and_then(|value| value.as_str())
                                                .unwrap_or("current stage");
                                            format!("Stage cancellation requested: {}", stage)
                                        }
                                        _ => "Run cancellation requested".to_string(),
                                    })
                                    .unwrap_or_else(|| "Run cancellation requested".to_string());
                                self.toast.show(ToastVariant::Info, &message, 3000);
                            }
                        }
                    }
                }
            }
            InteractiveCommand::ShowRecovery => {
                self.handle_show_recovery_actions();
            }
            InteractiveCommand::ExecuteRecovery(selector) => {
                self.handle_execute_recovery_action(&selector);
            }
            InteractiveCommand::NewSession => {
                self.execute_command_action(CommandAction::NewSession)?;
            }
            InteractiveCommand::ShowStatus => {
                self.execute_command_action(CommandAction::ShowStatus)?;
            }
            InteractiveCommand::ListModels => {
                self.execute_command_action(CommandAction::OpenModelList)?;
            }
            InteractiveCommand::SelectModel(model_ref) => {
                self.set_active_model_selection(model_ref.clone(), provider_from_model(&model_ref));
                self.toast.show(
                    ToastVariant::Success,
                    &format!("Model set to {}", model_ref),
                    1800,
                );
            }
            InteractiveCommand::ListProviders => {
                self.execute_command_action(CommandAction::ConnectProvider)?;
            }
            InteractiveCommand::ListThemes => {
                self.execute_command_action(CommandAction::OpenThemeList)?;
            }
            InteractiveCommand::ListPresets => {
                self.execute_command_action(CommandAction::OpenAgentList)?;
            }
            InteractiveCommand::ListSessions => {
                self.execute_command_action(CommandAction::OpenSessionList)?;
            }
            InteractiveCommand::ParentSession => {
                self.execute_command_action(CommandAction::NavigateParentSession)?;
            }
            InteractiveCommand::ListTasks => {
                self.handle_list_tasks();
            }
            InteractiveCommand::ShowTask(id) => {
                self.handle_show_task(&id);
            }
            InteractiveCommand::KillTask(id) => {
                self.handle_kill_task(&id);
            }
            InteractiveCommand::ClearScreen => {
                // TUI doesn't need clear-screen — no-op
            }
            InteractiveCommand::Compact => {
                // TODO: compact conversation
            }
            InteractiveCommand::Copy => {
                self.paste_clipboard_to_prompt();
            }
            InteractiveCommand::ListAgents => {
                self.execute_command_action(CommandAction::OpenAgentList)?;
            }
            InteractiveCommand::SelectAgent(name) => {
                if let Some(mode) = self
                    .agent_select
                    .agents()
                    .iter()
                    .find(|mode| mode.kind == ModeKind::Agent && mode.name == name)
                {
                    apply_selected_mode(&self.context, mode);
                    self.toast.show(
                        ToastVariant::Success,
                        &format!("Agent set to {}", mode.name),
                        1800,
                    );
                }
            }
            InteractiveCommand::SelectPreset(name) => {
                if let Some(mode) = self.agent_select.agents().iter().find(|mode| {
                    matches!(mode.kind, ModeKind::Preset | ModeKind::Profile) && mode.name == name
                }) {
                    apply_selected_mode(&self.context, mode);
                    self.toast.show(
                        ToastVariant::Success,
                        &format!("Preset set to {}", mode.name),
                        1800,
                    );
                }
            }
            InteractiveCommand::ToggleSidebar
            | InteractiveCommand::ToggleActive
            | InteractiveCommand::ScrollUp
            | InteractiveCommand::ScrollDown
            | InteractiveCommand::ScrollBottom => {
                // Layout toggling / scrolling not applicable in TUI — TUI has its own layout
            }
            InteractiveCommand::InspectStage(_stage_id) => {
                // Stage inspection not yet wired in TUI — planned for inspector panel
            }
            InteractiveCommand::Unknown(_) => {
                // Ignore unknown commands in TUI
            }
        }

        Ok(true)
    }
}
