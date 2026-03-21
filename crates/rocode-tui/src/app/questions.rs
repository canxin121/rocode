use super::*;

impl App {
    pub(super) fn question_info_to_prompt(question: &QuestionInfo) -> Option<QuestionRequest> {
        if let Some(item) = question.items.first() {
            let mut options: Vec<QuestionOption> = item
                .options
                .iter()
                .map(|o| QuestionOption {
                    id: o.label.clone(),
                    label: o.label.clone(),
                    description: o.description.clone(),
                })
                .collect();
            if !options
                .iter()
                .any(|option| option.id.eq_ignore_ascii_case(OTHER_OPTION_ID))
            {
                options.push(QuestionOption {
                    id: OTHER_OPTION_ID.to_string(),
                    label: OTHER_OPTION_LABEL.to_string(),
                    description: None,
                });
            }
            let question_type = if options.is_empty() {
                QuestionType::Text
            } else if item.multiple {
                QuestionType::MultipleChoice
            } else {
                QuestionType::SingleChoice
            };
            return Some(QuestionRequest {
                id: question.id.clone(),
                question: item.question.clone(),
                question_type,
                options,
            });
        }
        None
    }

    pub(super) fn clear_question_tracking(&mut self, question_id: &str) {
        self.pending_question_ids.remove(question_id);
        self.pending_questions.remove(question_id);
        self.pending_question_queue.retain(|id| id != question_id);
    }

    pub(super) fn open_next_question_prompt(&mut self) -> bool {
        if self.question_prompt.is_open {
            return false;
        }

        while let Some(question_id) = self.pending_question_queue.pop_front() {
            let Some(question) = self.pending_questions.get(&question_id).cloned() else {
                continue;
            };
            if let Some(prompt) = Self::question_info_to_prompt(&question) {
                self.question_prompt.ask(prompt);
                return true;
            }
            self.clear_question_tracking(&question_id);
        }
        false
    }

    pub(super) fn sync_question_requests(&mut self) -> bool {
        self.perf.question_sync = self.perf.question_sync.saturating_add(1);
        let Some(client) = self.context.get_api_client() else {
            return false;
        };

        let active_session = match self.context.current_route() {
            Route::Session { session_id } => Some(session_id),
            _ => None,
        };

        let mut questions = match client.list_questions() {
            Ok(items) => items,
            Err(err) => {
                tracing::debug!(%err, "failed to list pending questions");
                return false;
            }
        };

        if let Some(session_id) = active_session.as_deref() {
            questions.retain(|q| q.session_id == session_id);
        }
        questions.sort_by(|a, b| a.id.cmp(&b.id));

        let latest_ids = questions
            .iter()
            .map(|q| q.id.clone())
            .collect::<HashSet<_>>();
        let mut changed = latest_ids != self.pending_question_ids;

        for question in questions {
            let question_id = question.id.clone();
            self.pending_questions.insert(question_id.clone(), question);
            if self.pending_question_ids.insert(question_id.clone()) {
                self.pending_question_queue.push_back(question_id);
                changed = true;
            }
        }

        self.pending_question_ids
            .retain(|id| latest_ids.contains(id));
        self.pending_questions
            .retain(|id, _| latest_ids.contains(id));
        self.pending_question_queue
            .retain(|id| latest_ids.contains(id));

        if let Some(current_id) = self.question_prompt.current().map(|q| q.id.clone()) {
            if !latest_ids.contains(&current_id) {
                self.question_prompt.close();
                changed = true;
            }
        }

        if self.open_next_question_prompt() {
            changed = true;
        }
        changed
    }

    pub(super) fn submit_question_reply(&mut self, question_id: &str, answers: Vec<String>) {
        let Some(client) = self.context.get_api_client() else {
            self.alert_dialog
                .set_message("Cannot answer question: no API client");
            self.alert_dialog.open();
            return;
        };

        let question = self.pending_questions.get(question_id).cloned();
        let mut first_answer = answers
            .into_iter()
            .map(|answer| answer.trim().to_string())
            .filter(|answer| !answer.is_empty())
            .collect::<Vec<_>>();
        if first_answer.is_empty() {
            if let Some(default_option) =
                question
                    .as_ref()
                    .and_then(|q| q.items.first())
                    .and_then(|item| {
                        item.options
                            .iter()
                            .find(|option| !option.label.eq_ignore_ascii_case(OTHER_OPTION_LABEL))
                            .map(|option| option.label.clone())
                    })
            {
                first_answer.push(default_option);
            }
        }

        let question_count = question.as_ref().map(|q| q.items.len()).unwrap_or(1);
        let mut answers = vec![Vec::<String>::new(); question_count.max(1)];
        answers[0] = first_answer;

        match client.reply_question(question_id, answers) {
            Ok(()) => {
                self.clear_question_tracking(question_id);
                self.toast
                    .show(ToastVariant::Success, "Question answered", 2000);
                let _ = self.open_next_question_prompt();
            }
            Err(err) => {
                self.alert_dialog
                    .set_message(&format!("Failed to submit question response:\n{}", err));
                self.alert_dialog.open();
                if let Some(question) = question.and_then(|q| Self::question_info_to_prompt(&q)) {
                    self.question_prompt.ask(question);
                }
            }
        }
    }

    pub(super) fn reject_question(&mut self, question_id: &str) {
        let Some(client) = self.context.get_api_client() else {
            self.alert_dialog
                .set_message("Cannot reject question: no API client");
            self.alert_dialog.open();
            return;
        };

        match client.reject_question(question_id) {
            Ok(()) => {
                self.clear_question_tracking(question_id);
                self.toast
                    .show(ToastVariant::Info, "Question rejected", 1500);
                let _ = self.open_next_question_prompt();
            }
            Err(err) => {
                self.alert_dialog
                    .set_message(&format!("Failed to reject question:\n{}", err));
                self.alert_dialog.open();
            }
        }
    }
}
