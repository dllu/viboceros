//! Object picking and confirmation are prompt phases, not model transactions.
use super::*;
use viboceros_command::{ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow};
use viboceros_document::{ObjectId, SelectionMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ObjectPromptPhase {
    Selecting,
    Options,
    Menu(usize),
    Choice(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingObjectCommand {
    pub(super) description: ObjectSelectionPrompt,
    pub(super) phase: ObjectPromptPhase,
    pub(super) postselected: bool,
}

impl PendingObjectCommand {
    pub(super) fn selection_filter(&self) -> Option<ObjectSelectionFilter> {
        (self.phase == ObjectPromptPhase::Selecting).then_some(self.description.filter)
    }

    pub(super) fn label(&self) -> &'static str {
        match self.phase {
            ObjectPromptPhase::Menu(index) => self.description.menus[index].name,
            ObjectPromptPhase::Choice(index) => self.description.choices[index].name,
            _ => self.description.command,
        }
    }

    pub(super) fn hint(&self) -> &'static str {
        match self.phase {
            ObjectPromptPhase::Selecting => match self.description.workflow {
                ObjectSelectionWorkflow::OptionsDuringSelection => {
                    "Select meshes or type options; Enter finishes, Esc cancels"
                }
                ObjectSelectionWorkflow::ConfirmAfterSelection => {
                    "Select objects; Enter opens conversion options, Esc cancels"
                }
                ObjectSelectionWorkflow::ChooseBooleanAfterSelection => {
                    "Select curves and surfaces; Enter opens the deletion question, Esc cancels"
                }
            },
            ObjectPromptPhase::Options
                if self.description.workflow
                    == ObjectSelectionWorkflow::ChooseBooleanAfterSelection =>
            {
                "Delete input? Yes or No converts; Enter uses the shown choice, Esc cancels"
            }
            ObjectPromptPhase::Options => "Set conversion options; Enter converts, Esc cancels",
            ObjectPromptPhase::Menu(_) => {
                "Set mesh options; Enter returns to conversion options, Esc cancels"
            }
            ObjectPromptPhase::Choice(_) => {
                "Choose a value; Enter keeps the shown choice, Esc cancels"
            }
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_start_object_prompt(&mut self, input: &str) -> bool {
        let description = match self.commands.object_selection_prompt(input) {
            Ok(Some(prompt)) => prompt,
            Ok(None) => return false,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input = input.into();
                return true;
            }
        };
        let preselected = self
            .document
            .selected_objects()
            .any(|o| description.filter.accepts(o.geometry()));
        if preselected {
            if description.workflow == ObjectSelectionWorkflow::OptionsDuringSelection {
                return false;
            }
            // An explicit answer in a complete preselected invocation executes
            // directly. The bare command still asks its Yes/No question.
            if description.workflow == ObjectSelectionWorkflow::ChooseBooleanAfterSelection
                && input.split_whitespace().nth(1).is_some()
            {
                return false;
            }
            match self
                .commands
                .object_selection_confirmation(&self.document, &description)
            {
                Ok(Some(description)) => {
                    if let Err(error) = self.commands.accept_object_selection_options(&description)
                    {
                        self.push_log(format!("Error: {error}"));
                        return true;
                    }
                    self.cancel_interactive_command(false);
                    self.object_prompt = Some(PendingObjectCommand {
                        description,
                        phase: ObjectPromptPhase::Options,
                        postselected: false,
                    });
                }
                Ok(None) => return false,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.command_input = input.into();
                    return true;
                }
            }
        } else {
            self.cancel_interactive_command(false);
            if description.workflow == ObjectSelectionWorkflow::OptionsDuringSelection
                && let Err(error) = self.commands.accept_object_selection_options(&description)
            {
                self.push_log(format!("Error: {error}"));
                return true;
            }
            self.document.clear_selection();
            self.object_prompt = Some(PendingObjectCommand {
                description,
                phase: ObjectPromptPhase::Selecting,
                postselected: true,
            });
        }
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_object_prompt();
        true
    }

    fn log_object_prompt(&mut self) {
        if let Some(pending) = &self.object_prompt {
            let mut message = format!("{}. {}", pending.hint(), pending.description.command_line());
            for (index, choice) in pending.description.choices.iter().enumerate() {
                if pending.phase == ObjectPromptPhase::Selecting
                    || matches!(pending.phase, ObjectPromptPhase::Choice(selected) if selected != index)
                {
                    continue;
                }
                message.push_str(&format!("; {}: {}", choice.name, choice.choices.join("/")));
                if pending.phase == ObjectPromptPhase::Options
                    && let Some(toggle) = &choice.toggle
                    && toggle.values.contains(&choice.value)
                {
                    message.push_str(&format!("; {}", toggle.name));
                }
            }
            self.push_log(message);
        }
    }

    pub(super) fn try_continue_object_prompt(&mut self, input: &str) -> bool {
        let Some(mut pending) = self.object_prompt.clone() else {
            return false;
        };
        if input.is_empty() {
            if matches!(
                pending.phase,
                ObjectPromptPhase::Menu(_) | ObjectPromptPhase::Choice(_)
            ) {
                pending.phase = ObjectPromptPhase::Options;
                self.object_prompt = Some(pending);
                self.log_object_prompt();
                self.command_input.clear();
                return true;
            }
            if pending.phase == ObjectPromptPhase::Selecting {
                if !self
                    .document
                    .selected_objects()
                    .any(|o| pending.description.filter.accepts(o.geometry()))
                {
                    self.push_log("Select at least one eligible object; Esc cancels".into());
                    return true;
                }
                if pending.description.workflow != ObjectSelectionWorkflow::OptionsDuringSelection {
                    match self
                        .commands
                        .object_selection_confirmation(&self.document, &pending.description)
                    {
                        Ok(Some(description)) => {
                            if let Err(error) =
                                self.commands.accept_object_selection_options(&description)
                            {
                                self.push_log(format!("Error: {error}"));
                                return true;
                            }
                            pending.description = description;
                            pending.phase = ObjectPromptPhase::Options;
                            self.object_prompt = Some(pending);
                            self.command_input.clear();
                            self.log_object_prompt();
                            return true;
                        }
                        Ok(None) => {} // No-op selection finishes without accepting choices.
                        Err(error) => {
                            self.push_log(format!("Error: {error}"));
                            return true;
                        }
                    }
                }
            }
            let command = pending.description.command_line();
            self.push_log(format!("> {command}"));
            let context = viboceros_command::CommandContext {
                construction_plane: self.viewports[self.active_viewport].construction_plane(),
            };
            let result = if pending.postselected {
                self.commands
                    .execute_postselected(&mut self.document, &command, context)
            } else {
                self.commands
                    .execute_in_context(&mut self.document, &command, context)
            };
            match result {
                Ok(message) => {
                    self.object_prompt = None;
                    self.push_log(message);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        let normalized = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if normalized == "selall" || normalized == "selnone" {
            if pending.phase != ObjectPromptPhase::Selecting {
                self.push_log("Selection is fixed; finish or cancel the conversion".into());
                return true;
            }
            if normalized == "selnone" {
                self.document.clear_selection();
            } else {
                let ids = self
                    .document
                    .objects()
                    .filter(|o| {
                        self.document.is_object_selectable(o.id())
                            && pending.description.filter.accepts(o.geometry())
                    })
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                self.select_prompt_objects(ids, SelectionMode::Add);
            }
            self.command_input.clear();
            return true;
        }
        // Active option names take precedence over global command aliases
        // (notably Direction, which is also an alias of Dir).
        let option_name = normalized
            .split_whitespace()
            .next()
            .unwrap_or("")
            .split('=')
            .next()
            .unwrap_or("");
        let local_choice = pending.description.choices.iter().any(|c| {
            c.name.eq_ignore_ascii_case(option_name)
                || c.toggle
                    .as_ref()
                    .is_some_and(|t| t.name.eq_ignore_ascii_case(option_name))
        });
        let local_value = matches!(pending.phase, ObjectPromptPhase::Choice(index)
            if pending.description.choices[index].choices.iter().any(|v| v.eq_ignore_ascii_case(&normalized)));
        if !local_choice
            && !local_value
            && input
                .split_whitespace()
                .next()
                .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_object_prompt(true);
            return false;
        }
        if pending.phase == ObjectPromptPhase::Selecting
            && pending.description.workflow != ObjectSelectionWorkflow::OptionsDuringSelection
        {
            self.push_log("Select objects first; Enter opens conversion options".into());
            return true;
        }
        if pending.phase == ObjectPromptPhase::Options
            && let Some(index) = pending
                .description
                .choices
                .iter()
                .position(|choice| choice.name.eq_ignore_ascii_case(&normalized))
        {
            pending.phase = ObjectPromptPhase::Choice(index);
            self.object_prompt = Some(pending);
            self.command_input.clear();
            self.log_object_prompt();
            return true;
        }
        if pending.phase == ObjectPromptPhase::Options
            && let Some(index) = pending
                .description
                .menus
                .iter()
                .position(|menu| menu.name.eq_ignore_ascii_case(&normalized))
        {
            pending.phase = ObjectPromptPhase::Menu(index);
            self.object_prompt = Some(pending);
            self.command_input.clear();
            self.log_object_prompt();
            return true;
        }
        let update = match pending.phase {
            ObjectPromptPhase::Menu(index) => pending.description.update_menu_options(index, input),
            ObjectPromptPhase::Choice(index) => pending.description.choices[index].set(input),
            _ => pending.description.update_options(input),
        };
        match update.and_then(|()| {
            self.commands
                .accept_object_selection_options(&pending.description)
        }) {
            Ok(()) => {
                if matches!(pending.phase, ObjectPromptPhase::Choice(_)) {
                    pending.phase = ObjectPromptPhase::Options;
                }
                let answered = pending.description.workflow
                    == ObjectSelectionWorkflow::ChooseBooleanAfterSelection;
                self.object_prompt = Some(pending);
                self.command_input.clear();
                if answered {
                    return self.try_continue_object_prompt("");
                }
                self.log_object_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn cancel_object_prompt(&mut self, announce: bool) {
        if let Some(pending) = self.object_prompt.take() {
            if pending.postselected {
                self.document.clear_selection();
            }
            self.command_input.clear();
            if announce {
                self.push_log(format!("Cancelled {}", pending.description.command));
            }
        }
    }

    pub(super) fn select_prompt_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let Some(filter) = self
            .object_prompt
            .as_ref()
            .and_then(PendingObjectCommand::selection_filter)
        else {
            return;
        };
        let ids = ids
            .into_iter()
            .filter(|id| {
                self.document.object(*id).is_some_and(|o| {
                    self.document.is_object_selectable(*id) && filter.accepts(o.geometry())
                })
            })
            .collect::<Vec<_>>();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        match self.document.select_objects_direct(ids, mode) {
            Ok(count) => self.push_log(format!("Selected {count} object(s); Enter continues")),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
