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
    pub(super) subcurve_measurement: bool,
    pub(super) length_display_units: Option<&'static str>,
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
        if self.phase == ObjectPromptPhase::Selecting
            && self.description.allows_selection_options()
            && matches!(
                self.description.workflow,
                ObjectSelectionWorkflow::QuestionAfterSelection { .. }
            )
        {
            return "Select objects or type options; Enter finishes, Esc cancels";
        }
        match self.phase {
            ObjectPromptPhase::Selecting => match self.description.workflow {
                ObjectSelectionWorkflow::OptionsDuringSelection => {
                    if self.description.filter == ObjectSelectionFilter::Grouped {
                        "Select grouped objects or type options; Enter finishes, Esc cancels"
                    } else {
                        "Select objects or type options; Enter finishes, Esc cancels"
                    }
                }
                ObjectSelectionWorkflow::ConfirmAfterSelection => {
                    "Select objects; Enter opens conversion options, Esc cancels"
                }
                ObjectSelectionWorkflow::ChooseBooleanAfterSelection => {
                    "Select curves and surfaces; Enter opens the deletion question, Esc cancels"
                }
                ObjectSelectionWorkflow::QuestionAfterSelection { .. } => {
                    "Select objects; Enter continues, Esc cancels selection"
                }
            },
            ObjectPromptPhase::Options
                if self.description.workflow
                    == ObjectSelectionWorkflow::ChooseBooleanAfterSelection =>
            {
                "Delete input? Yes or No converts; Enter uses the shown choice, Esc cancels"
            }
            ObjectPromptPhase::Options
                if matches!(
                    self.description.workflow,
                    ObjectSelectionWorkflow::QuestionAfterSelection { .. }
                ) =>
            {
                let ObjectSelectionWorkflow::QuestionAfterSelection { message, .. } =
                    self.description.workflow
                else {
                    unreachable!()
                };
                message
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
    pub(super) fn viewport_object_filter(&self) -> Option<ObjectSelectionFilter> {
        if self.edge_prompt.is_some() {
            return None;
        }
        if self.picking_alignment_curve() {
            return Some(ObjectSelectionFilter::Curves);
        }
        if self.group_prompt == Some(group_prompt::GroupPrompt::Target) {
            return Some(ObjectSelectionFilter::Grouped);
        }
        if self.intersection_prompt.is_some() {
            return Some(ObjectSelectionFilter::Parametric);
        }
        self.object_prompt
            .as_ref()
            .map_or(Some(ObjectSelectionFilter::Any), |prompt| {
                prompt.selection_filter()
            })
    }

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
        let subcurve_measurement = matches!(description.command, "Domain" | "Length")
            && input.split_whitespace().nth(1).is_some_and(|option| {
                option
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("SubCrv")
            });
        let length_display_units = (description.command == "Length")
            .then(|| {
                input
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|argument| argument.split_once('='))
                    .find(|(name, _)| name.trim_start_matches('_').eq_ignore_ascii_case("Units"))
                    .and_then(|(_, value)| viboceros_command::distance_display_units(value).ok())
                    .flatten()
            })
            .flatten();
        let preselected = self
            .document
            .selected_objects()
            .any(|o| description.filter.accepts_object(o));
        if preselected {
            if description.workflow == ObjectSelectionWorkflow::OptionsDuringSelection {
                return false;
            }
            // An explicit answer in a complete preselected invocation executes
            // directly. The bare command still asks its Yes/No question.
            if description.workflow.answers_immediately()
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
                        subcurve_measurement,
                        length_display_units,
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
            if description.allows_selection_options()
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
                subcurve_measurement,
                length_display_units,
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
                pending.phase = if pending.description.allows_selection_options() {
                    ObjectPromptPhase::Selecting
                } else {
                    ObjectPromptPhase::Options
                };
                self.object_prompt = Some(pending);
                self.log_object_prompt();
                self.command_input.clear();
                return true;
            }
            if pending.phase == ObjectPromptPhase::Selecting {
                if !self
                    .document
                    .selected_objects()
                    .any(|o| pending.description.filter.accepts_object(o))
                {
                    if self
                        .commands
                        .cancel_empty_object_selection(&pending.description)
                    {
                        self.cancel_object_prompt(true);
                        return true;
                    }
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
            let mut command = pending.description.command_line();
            if pending.subcurve_measurement {
                command.push_str(" SubCrv");
            }
            if let Some(units) = pending.length_display_units {
                command.push_str(&format!(" Units={units}"));
            }
            if pending.subcurve_measurement && self.domain_has_one_curve() {
                self.object_prompt = None;
                self.try_start_interactive_command(&command);
                return true;
            }
            if pending.description.command == "Align" {
                self.object_prompt = None;
                if self.try_start_interactive_command(&command) {
                    if let Some(InteractiveCommand::Align { postselected, .. }) =
                        &mut self.active_command
                    {
                        *postselected = pending.postselected;
                    }
                    return true;
                }
                // No-point modes execute directly after selection acceptance.
            }
            if pending.description.command == "EvaluateUVPt" && self.evaluate_uv_can_pick() {
                self.object_prompt = None;
                self.try_start_interactive_command(&command);
                return true;
            }
            if command == "Domain" && self.domain_needs_face_pick() {
                // This is acceptance, not cancellation: retain the picked
                // object while handing control to the component-point phase.
                self.object_prompt = None;
                self.try_start_interactive_command(&command);
                return true;
            }
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
                Err(error) => {
                    if matches!(error, viboceros_command::CommandError::OperationDeclined) {
                        self.object_prompt = None;
                        self.push_log(format!("{} declined", pending.description.command));
                    } else {
                        self.push_log(format!("Error: {error}"));
                    }
                }
            }
            self.command_input.clear();
            return true;
        }
        let normalized = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if pending.description.command == "Length"
            && pending.phase == ObjectPromptPhase::Selecting
            && let Some((name, value)) = normalized.split_once('=')
            && name.eq_ignore_ascii_case("units")
        {
            match viboceros_command::distance_display_units(value) {
                Ok(units) => {
                    pending.length_display_units = units;
                    self.object_prompt = Some(pending);
                    self.push_log(format!(
                        "Length display units: {}",
                        units.unwrap_or("Model_Units")
                    ));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        if normalized == "selall" || normalized == "selnone" {
            if pending.phase != ObjectPromptPhase::Selecting {
                self.push_log("Selection is fixed; finish or cancel the command".into());
                return true;
            }
            if normalized == "selnone" {
                self.document.clear_selection();
            } else {
                let ids = self
                    .document
                    .selectable_objects()
                    .filter(|o| pending.description.filter.accepts_object(o))
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
            && !pending.description.allows_selection_options()
        {
            self.push_log("Select objects first; Enter continues".into());
            return true;
        }
        if (pending.phase == ObjectPromptPhase::Options
            || (pending.phase == ObjectPromptPhase::Selecting
                && pending.description.allows_selection_options()))
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
        let update = if pending.description.command == "Align" {
            self.update_align_selection(&pending.description, input)
                .map(|description| pending.description = description)
        } else {
            match pending.phase {
                ObjectPromptPhase::Menu(index) => {
                    pending.description.update_menu_options(index, input)
                }
                ObjectPromptPhase::Choice(index) => pending.description.choices[index].set(input),
                _ => pending.description.update_options(input),
            }
        };
        match update.and_then(|()| {
            self.commands
                .accept_object_selection_options(&pending.description)
        }) {
            Ok(()) => {
                if matches!(pending.phase, ObjectPromptPhase::Choice(_)) {
                    pending.phase = if pending.description.allows_selection_options() {
                        ObjectPromptPhase::Selecting
                    } else {
                        ObjectPromptPhase::Options
                    };
                }
                let answered = pending.phase == ObjectPromptPhase::Options
                    && pending.description.workflow.answers_immediately();
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

    /// Keyboard Escape at an explicit warning may mean a default answer, unlike
    /// replacing a command, cancelling selection, or closing a document.
    pub(super) fn answer_object_prompt_escape(&mut self) -> bool {
        let Some(pending) = &self.object_prompt else {
            return false;
        };
        if pending.phase != ObjectPromptPhase::Options {
            return false;
        }
        let ObjectSelectionWorkflow::QuestionAfterSelection {
            escape_answer: Some(answer),
            ..
        } = pending.description.workflow
        else {
            return false;
        };
        self.try_continue_object_prompt(if answer { "Yes" } else { "No" })
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
        let requested = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|object| requested.contains(&object.id()))
            .filter(|object| filter.accepts_object(object))
            .map(|object| object.id())
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
        if let Some(pending) = &self.object_prompt {
            match self
                .commands
                .object_selection_complete(&self.document, &pending.description)
            {
                Ok(true) => {
                    self.try_continue_object_prompt("");
                }
                Ok(false) => {}
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }
}
