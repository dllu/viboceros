//! Object picking and confirmation are prompt phases, not model transactions.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::{ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow};
use viboceros_document::{Geometry, ObjectId, SelectionMode};

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
    pub(super) measurement_display_units: Option<&'static str>,
    pub(super) command_override: Option<String>,
    pub(super) excluded_object: Option<ObjectId>,
    pub(super) selection_before: Option<Vec<ObjectId>>,
    pub(super) cloud_removal: Option<PendingCloudRemoval>,
    pub(super) cloud_action_target: Option<ObjectId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingCloudRemoval {
    pub(super) target: ObjectId,
    pub(super) indices: BTreeSet<usize>,
    pub(super) output_cloud: bool,
}

impl PendingObjectCommand {
    pub(super) fn selection_filter(&self) -> Option<ObjectSelectionFilter> {
        (self.phase == ObjectPromptPhase::Selecting
            && self.cloud_removal.is_none()
            && self.cloud_action_target.is_none())
        .then_some(self.description.filter)
    }

    pub(super) fn label(&self) -> &'static str {
        match self.phase {
            ObjectPromptPhase::Menu(index) => self.description.menus[index].name,
            ObjectPromptPhase::Choice(index) => self.description.choices[index].name,
            _ => self.description.command,
        }
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.cloud_action_target.is_some() {
            return "Selected point cloud: type Add or Remove; Esc cancels";
        }
        if self.cloud_removal.is_some() {
            return "Pick cloud points or drag a window; Output=Points|PointCloud; Enter removes, Esc cancels";
        }
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
    fn point_cloud_prompt_target(&self, input: &str) -> Option<ObjectId> {
        let explicit = input
            .split_whitespace()
            .skip(2)
            .filter_map(|argument| argument.split_once('='))
            .find(|(name, _)| name.trim_start_matches('_').eq_ignore_ascii_case("Target"))
            .and_then(|(_, value)| value.parse::<ObjectId>().ok());
        let target = explicit.or_else(|| {
            let mut clouds = self
                .document
                .selected_objects()
                .filter(|object| matches!(object.geometry(), Geometry::PointCloud(_)));
            let first = clouds.next()?.id();
            clouds.next().is_none().then_some(first)
        })?;
        self.document
            .object(target)
            .filter(|object| matches!(object.geometry(), Geometry::PointCloud(_)))
            .map(|_| target)
    }

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
        let measurement_display_units = matches!(description.command, "Length" | "Area")
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
        let sole_selected_cloud = (self.document.selected_object_count() == 1)
            .then(|| {
                self.document.selected_objects().next().and_then(|object| {
                    matches!(object.geometry(), Geometry::PointCloud(_)).then_some(object.id())
                })
            })
            .flatten();
        if description.filter == ObjectSelectionFilter::PointCloudSources
            && let Some(target) = sole_selected_cloud
        {
            self.cancel_interactive_command(false);
            self.object_prompt = Some(PendingObjectCommand {
                description,
                phase: ObjectPromptPhase::Options,
                postselected: false,
                subcurve_measurement: false,
                measurement_display_units: None,
                command_override: None,
                excluded_object: None,
                selection_before: None,
                cloud_removal: None,
                cloud_action_target: Some(target),
            });
            self.command_input.clear();
            self.push_log(format!("> {input}"));
            self.log_object_prompt();
            return true;
        }
        if description.filter == ObjectSelectionFilter::PointCloudAddSources {
            let Some(target) = self.point_cloud_prompt_target(input) else {
                return false;
            };
            if self
                .document
                .selected_objects()
                .any(|object| object.id() != target && description.filter.accepts_object(object))
            {
                return false;
            }
            let selection_before = self.document.selected_object_ids().collect();
            self.cancel_interactive_command(false);
            self.object_prompt = Some(PendingObjectCommand {
                description,
                phase: ObjectPromptPhase::Selecting,
                postselected: false,
                subcurve_measurement: false,
                measurement_display_units: None,
                command_override: Some(format!("PointCloud Add Target={target}")),
                excluded_object: Some(target),
                selection_before: Some(selection_before),
                cloud_removal: None,
                cloud_action_target: None,
            });
            self.command_input.clear();
            self.push_log(format!("> {input}"));
            self.log_object_prompt();
            return true;
        }
        if description.filter == ObjectSelectionFilter::PointCloudRemoveTarget {
            let Some(target) = self.point_cloud_prompt_target(input) else {
                return false;
            };
            let output_cloud = input
                .split_whitespace()
                .skip(2)
                .filter_map(|argument| argument.split_once('='))
                .find(|(name, _)| name.trim_start_matches('_').eq_ignore_ascii_case("Output"))
                .is_some_and(|(_, value)| value.eq_ignore_ascii_case("PointCloud"));
            self.cancel_interactive_command(false);
            self.object_prompt = Some(PendingObjectCommand {
                description,
                phase: ObjectPromptPhase::Selecting,
                postselected: false,
                subcurve_measurement: false,
                measurement_display_units: None,
                command_override: Some(format!(
                    "PointCloud Remove Target={target} Output={}",
                    if output_cloud { "PointCloud" } else { "Points" }
                )),
                excluded_object: None,
                selection_before: None,
                cloud_removal: Some(PendingCloudRemoval {
                    target,
                    indices: BTreeSet::new(),
                    output_cloud,
                }),
                cloud_action_target: None,
            });
            self.command_input.clear();
            self.push_log(format!("> {input}"));
            self.log_object_prompt();
            return true;
        }
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
                        measurement_display_units,
                        command_override: None,
                        excluded_object: None,
                        selection_before: None,
                        cloud_removal: None,
                        cloud_action_target: None,
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
                measurement_display_units,
                command_override: None,
                excluded_object: None,
                selection_before: None,
                cloud_removal: None,
                cloud_action_target: None,
            });
        }
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_object_prompt();
        true
    }

    fn log_object_prompt(&mut self) {
        if let Some(pending) = &self.object_prompt {
            let command = pending
                .command_override
                .clone()
                .unwrap_or_else(|| pending.description.command_line());
            let mut message = format!("{}. {}", pending.hint(), command);
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
        if let Some(target) = pending.cloud_action_target {
            return self.continue_cloud_action(input, target);
        }
        if pending.cloud_removal.is_some() {
            return self.continue_cloud_removal(input, pending);
        }
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
                if !self.document.selected_objects().any(|o| {
                    Some(o.id()) != pending.excluded_object
                        && pending.description.filter.accepts_object(o)
                }) {
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
            let mut command = pending
                .command_override
                .clone()
                .unwrap_or_else(|| pending.description.command_line());
            if pending.subcurve_measurement {
                command.push_str(" SubCrv");
            }
            if let Some(units) = pending.measurement_display_units {
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
        if matches!(pending.description.command, "Length" | "Area")
            && pending.phase == ObjectPromptPhase::Selecting
            && let Some((name, value)) = normalized.split_once('=')
            && name.eq_ignore_ascii_case("units")
        {
            match viboceros_command::distance_display_units(value) {
                Ok(units) => {
                    let name = pending.description.command;
                    pending.measurement_display_units = units;
                    self.object_prompt = Some(pending);
                    self.push_log(format!(
                        "{} display units: {}",
                        name,
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
                    .filter(|o| {
                        Some(o.id()) != pending.excluded_object
                            && pending.description.filter.accepts_object(o)
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

    fn continue_cloud_action(&mut self, input: &str, target: ObjectId) -> bool {
        let action = input.trim_start_matches(['_', '-']);
        if action.eq_ignore_ascii_case("Add") || action.eq_ignore_ascii_case("Remove") {
            self.object_prompt = None;
            let command = format!("PointCloud {action} Target={target}");
            if !self.try_start_object_prompt(&command) {
                self.push_log("Error: the selected point cloud is no longer available".into());
            }
            self.command_input.clear();
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_object_prompt(true);
            return false;
        }
        self.push_log("Type Add or Remove; Esc cancels".into());
        self.command_input.clear();
        true
    }

    fn continue_cloud_removal(&mut self, input: &str, mut pending: PendingObjectCommand) -> bool {
        let removal = pending.cloud_removal.as_mut().unwrap();
        if input.is_empty() {
            if removal.indices.is_empty() {
                self.push_log("Pick at least one cloud point; Esc cancels".into());
                return true;
            }
            let indices = removal
                .indices
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let command = format!(
                "{} Indices={indices}",
                pending.command_override.as_deref().unwrap()
            );
            let context = viboceros_command::CommandContext {
                construction_plane: self.viewports[self.active_viewport].construction_plane(),
            };
            self.push_log(format!("> {command}"));
            match self
                .commands
                .execute_in_context(&mut self.document, &command, context)
            {
                Ok(message) => {
                    self.object_prompt = None;
                    self.push_log(message);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        let normalized = input.trim_start_matches(['_', '-']);
        if normalized.eq_ignore_ascii_case("SelAll") || normalized.eq_ignore_ascii_case("SelNone") {
            if normalized.eq_ignore_ascii_case("SelNone") {
                removal.indices.clear();
            } else if let Some(object) = self.document.object(removal.target)
                && let Geometry::PointCloud(cloud) = object.geometry()
            {
                removal.indices = (0..cloud.points().len()).collect();
            }
        } else if let Some((name, value)) = normalized.split_once('=')
            && name.eq_ignore_ascii_case("Output")
        {
            removal.output_cloud = if value.eq_ignore_ascii_case("PointCloud") {
                true
            } else if value.eq_ignore_ascii_case("Points") {
                false
            } else {
                self.push_log("Output must be Points or PointCloud".into());
                return true;
            };
            pending.command_override = Some(format!(
                "PointCloud Remove Target={} Output={}",
                removal.target,
                if removal.output_cloud {
                    "PointCloud"
                } else {
                    "Points"
                }
            ));
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_object_prompt(true);
            return false;
        } else {
            self.push_log(
                "Pick cloud points or type Output=Points|PointCloud, SelAll, or SelNone".into(),
            );
            return true;
        }
        let count = removal.indices.len();
        self.push_log(format!("{count} cloud point(s) selected"));
        self.object_prompt = Some(pending);
        self.command_input.clear();
        true
    }

    pub(super) fn select_cloud_points(&mut self, indices: &[usize], mode: SelectionMode) {
        let Some(pending) = self.object_prompt.as_mut() else {
            return;
        };
        let Some(removal) = pending.cloud_removal.as_mut() else {
            return;
        };
        match mode {
            SelectionMode::Replace | SelectionMode::Add => {
                removal.indices.extend(indices.iter().copied());
            }
            SelectionMode::Remove => {
                for index in indices {
                    removal.indices.remove(index);
                }
            }
            SelectionMode::Toggle => {
                for index in indices {
                    if !removal.indices.insert(*index) {
                        removal.indices.remove(index);
                    }
                }
            }
        }
        let count = removal.indices.len();
        self.push_log(format!("{count} cloud point(s) selected"));
    }

    pub(super) fn cancel_object_prompt(&mut self, announce: bool) {
        if let Some(pending) = self.object_prompt.take() {
            if let Some(selection_before) = pending.selection_before {
                if let Err(error) = self
                    .document
                    .select_objects_direct(selection_before, SelectionMode::Replace)
                {
                    self.push_log(format!("Error restoring selection: {error}"));
                }
            } else if pending.postselected {
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
        let excluded = self
            .object_prompt
            .as_ref()
            .and_then(|prompt| prompt.excluded_object);
        let requested = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|object| requested.contains(&object.id()))
            .filter(|object| Some(object.id()) != excluded)
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
