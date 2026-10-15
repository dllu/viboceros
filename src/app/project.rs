//! Directional projection source/target picking and session options.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Vector3;

#[derive(Clone, Debug)]
pub(super) struct ProjectOptions {
    direction: String,
    delete_input: bool,
    output_layer: &'static str,
    loose: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlaneOperation {
    Project,
    Pull,
    IPlane,
}
/// Shared source selection and plane getter state for Project, Pull and IPlane.
#[derive(Clone, Debug)]
pub(super) struct ProjectSession {
    pub(super) operation: PlaneOperation,
    pub(super) options: ProjectOptions,
    pub(super) custom_requested: bool,
    pub(super) finish_targets: Option<Vec<ObjectId>>,
    pub(super) planes: Vec<String>,
    pub(super) plane_getter: Option<super::project_plane::PlaneGetter>,
}
impl ProjectSession {
    pub(super) fn is_iplane(&self) -> bool {
        self.operation == PlaneOperation::IPlane
    }
    pub(super) fn is_pull(&self) -> bool {
        self.operation == PlaneOperation::Pull
    }
    pub(super) fn name(&self) -> &'static str {
        match self.operation {
            PlaneOperation::Project => "Project",
            PlaneOperation::Pull => "Pull",
            PlaneOperation::IPlane => "IPlane",
        }
    }
    fn command(&self) -> String {
        if self.is_iplane() {
            return "IPlane".into();
        }
        if self.is_pull() {
            return self.options.pull_command();
        }
        self.options
            .command_with_direction(if self.custom_requested {
                "Custom"
            } else {
                &self.options.direction
            })
    }
}
fn custom_option(argument: &str) -> bool {
    argument
        .trim_start_matches('_')
        .split_once('=')
        .is_some_and(|(key, value)| {
            key.eq_ignore_ascii_case("Direction")
                && value.trim_start_matches('_').eq_ignore_ascii_case("Custom")
        })
}

impl Default for ProjectOptions {
    fn default() -> Self {
        Self {
            direction: "CPlaneZ".into(),
            delete_input: false,
            output_layer: "Current",
            loose: false,
        }
    }
}
impl ProjectOptions {
    fn argument(&mut self, argument: &str) -> bool {
        let Some((key, value)) = argument.trim_start_matches('_').split_once('=') else {
            return false;
        };
        let value = value.trim_start_matches('_');
        match key.to_ascii_lowercase().as_str() {
            "direction" => {
                if value.eq_ignore_ascii_case("CPlaneZ") {
                    self.direction = "CPlaneZ".into();
                } else if value.eq_ignore_ascii_case("View") {
                    self.direction = "View".into();
                } else {
                    let coordinates = value
                        .split(',')
                        .map(str::parse::<f64>)
                        .collect::<Result<Vec<_>, _>>();
                    let Ok(coordinates) = coordinates else {
                        return false;
                    };
                    let Ok(coordinates) = <[f64; 3]>::try_from(coordinates) else {
                        return false;
                    };
                    if Vector3::try_from(coordinates)
                        .and_then(|v| v.normalized_nonzero())
                        .is_err()
                    {
                        return false;
                    }
                    self.direction = value.into();
                }
            }
            "deleteinput" => match value.to_ascii_lowercase().as_str() {
                "yes" => self.delete_input = true,
                "no" => self.delete_input = false,
                _ => return false,
            },
            "loose" => match value.to_ascii_lowercase().as_str() {
                "yes" => self.loose = true,
                "no" => self.loose = false,
                _ => return false,
            },
            "outputlayer" => {
                self.output_layer = match value.to_ascii_lowercase().as_str() {
                    "current" => "Current",
                    "input" => "Input",
                    "targetobject" => "TargetObject",
                    _ => return false,
                }
            }
            _ => return false,
        }
        true
    }
    fn argument_for(&mut self, argument: &str, pull: bool) -> bool {
        let key = argument
            .trim_start_matches('_')
            .split_once('=')
            .map(|(k, _)| k);
        if pull && key.is_some_and(|k| k.eq_ignore_ascii_case("Direction")) {
            return false;
        }
        self.argument(argument)
    }
    pub(super) fn pull_command(&self) -> String {
        format!(
            "Pull DeleteInput={} OutputLayer={} Loose={}",
            if self.delete_input { "Yes" } else { "No" },
            self.output_layer,
            if self.loose { "Yes" } else { "No" }
        )
    }
    pub(super) fn command(&self) -> String {
        self.command_with_direction(&self.direction)
    }
    fn command_with_direction(&self, direction: &str) -> String {
        format!(
            "Project Direction={} DeleteInput={} OutputLayer={} Loose={}",
            direction,
            if self.delete_input { "Yes" } else { "No" },
            self.output_layer,
            if self.loose { "Yes" } else { "No" }
        )
    }
}
impl VibocerosApp {
    pub(super) fn try_start_project_prompt(&mut self, input: &str) -> bool {
        if self.try_start_iplane_prompt(input) {
            return true;
        }
        let mut words = input.split_whitespace();
        let Some(name) = words.next() else {
            return false;
        };
        let name = name.trim_start_matches(['_', '-']);
        let pull = name.eq_ignore_ascii_case("Pull");
        if !pull && !name.eq_ignore_ascii_case("Project") {
            return false;
        }
        let mut options = if pull {
            self.pull_options.clone()
        } else {
            self.project_options.clone()
        };
        let mut custom_requested = false;
        let mut seen = std::collections::BTreeSet::new();
        for argument in words {
            let Some((key, _)) = argument.split_once('=') else {
                return false;
            };
            if !seen.insert(key.trim_start_matches('_').to_ascii_lowercase()) {
                return false;
            }
            if !pull && custom_option(argument) {
                custom_requested = true;
                continue;
            }
            if !options.argument_for(argument, pull) {
                return false;
            }
        }
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let first = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::ApplyCurves.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let targets = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::ProjectTargets.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        if pull {
            self.pull_options = options.clone();
        } else {
            self.project_options = options.clone();
        }
        if !custom_requested && !first.is_empty() && !targets.is_empty() {
            self.try_execute_command(&format!(
                "{} Sources={} Targets={}",
                if pull {
                    options.pull_command()
                } else {
                    options.command()
                },
                project_ids(&first),
                project_ids(&targets)
            ));
            self.command_input.clear();
            return true;
        }
        if !custom_requested || first.is_empty() || targets.is_empty() {
            self.document.clear_selection();
        }
        let finish_targets =
            (custom_requested && !first.is_empty() && !targets.is_empty()).then_some(targets);
        self.intersection_prompt = Some(TwoSetsPrompt {
            first: (!first.is_empty()).then_some(first),
            original_selection,
            output_layer: "Current",
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            projection: Some(ProjectSession {
                operation: if pull {
                    PlaneOperation::Pull
                } else {
                    PlaneOperation::Project
                },
                options,
                custom_requested,
                finish_targets,
                planes: Vec::new(),
                plane_getter: None,
            }),
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        if custom_requested {
            self.begin_project_direction();
        } else {
            self.log_intersection_prompt();
        }
        true
    }
    pub(super) fn log_project_prompt(&mut self, prompt: &TwoSetsPrompt) {
        self.push_log(format!(
            "{}: {}",
            prompt.projection.as_ref().unwrap().command(),
            if self.picking_project_direction() {
                self.active_command
                    .unwrap()
                    .prompt()
                    .strip_prefix("Project: ")
                    .unwrap_or(self.active_command.unwrap().prompt())
            } else {
                prompt.hint()
            }
        ));
    }
    pub(super) fn continue_project_prompt(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if self.continue_project_plane(&prompt, input) {
            return true;
        }
        if !prompt.projection.as_ref().unwrap().is_iplane()
            && !self.picking_project_direction()
            && prompt.first.is_none()
            && self.try_continue_uv_subcurve(&prompt, input)
        {
            return true;
        }
        if self.picking_project_direction() {
            if input.trim_start_matches('_').eq_ignore_ascii_case("Undo") {
                self.active_command = Some(InteractiveCommand::ProjectDirection { start: None });
                self.point_filter = None;
                self.point_constraint = None;
                self.drafting_plane = None;
                self.command_input.clear();
                self.log_intersection_prompt();
                return true;
            }
            if input
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Cancel")
            {
                self.cancel_interactive_command(true);
                return true;
            }
            if self.try_continue_point_constraint(input)
                || self.try_continue_point_filter(input)
                || self.try_continue_point_input(input)
            {
                return true;
            }
            if input.is_empty()
                || matches!(
                    input.trim_start_matches('_').to_ascii_lowercase().as_str(),
                    "selall" | "selnone"
                )
            {
                self.command_input.clear();
                self.log_intersection_prompt();
                return true;
            }
        }
        if input.is_empty() {
            let selected = self
                .document
                .selected_objects()
                .filter(|o| prompt.filter().accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if selected.is_empty()
                && if prompt.first.is_some() {
                    prompt.projection.as_ref().unwrap().planes.is_empty()
                } else {
                    prompt.uv_subcurves.ranges.is_empty()
                }
            {
                self.push_log(prompt.hint().into());
                return true;
            }
            if let Some(first) = &prompt.first {
                let command = project_command(&prompt, first, &selected);
                let retry = if prompt.projection.as_ref().unwrap().is_pull() {
                    "Adjust the targets; Enter retries"
                } else {
                    "Adjust the targets or direction; Enter retries"
                };
                if !self.try_execute_command(&command) {
                    self.intersection_prompt = Some(prompt);
                    self.push_log(retry.into());
                }
            } else {
                prompt.first = Some(selected);
                self.document.clear_selection();
                let intersection = prompt.projection.as_ref().unwrap().is_iplane();
                self.intersection_prompt = Some(prompt);
                if intersection {
                    self.begin_iplane_getter();
                } else {
                    self.log_intersection_prompt();
                }
            }
        } else if input.contains('=')
            && !self.commands.recognizes(
                input
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches(['_', '-']),
            )
        {
            let mut session = prompt.projection.clone().unwrap();
            if session.is_iplane() {
                self.push_log("IPlane: choose a plane or press Esc to cancel".into());
            } else if !session.is_pull() && custom_option(input) {
                self.clear_project_plane_state();
                session.plane_getter = None;
                session.custom_requested = true;
                prompt.projection = Some(session);
                self.intersection_prompt = Some(prompt);
                self.begin_project_direction();
            } else if session.options.argument_for(input, session.is_pull()) {
                let direction_changed = input
                    .trim_start_matches('_')
                    .split_once('=')
                    .is_some_and(|(key, _)| key.eq_ignore_ascii_case("Direction"));
                if direction_changed {
                    self.clear_project_plane_state();
                    session.plane_getter = None;
                    session.custom_requested = false;
                    self.clear_project_direction();
                }
                if session.is_pull() {
                    self.pull_options = session.options.clone();
                } else {
                    self.project_options = session.options.clone();
                }
                prompt.projection = Some(session);
                self.intersection_prompt = Some(prompt.clone());
                if direction_changed {
                    self.finish_deferred_project(prompt);
                } else {
                    self.log_intersection_prompt();
                }
            } else {
                self.push_log(if session.is_pull() {"Pull options: DeleteInput=Yes|No, OutputLayer=Current|Input|TargetObject, Loose=No"}else{"Project options: Direction=CPlaneZ|View|Custom|x,y,z, DeleteInput=Yes|No, OutputLayer=Current|Input|TargetObject"}.into());
            }
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("SelNone")
        {
            self.document.clear_selection();
            if prompt.first.is_none() {
                self.intersection_prompt
                    .as_mut()
                    .unwrap()
                    .uv_subcurves
                    .ranges
                    .clear();
            }
        } else if input.trim_start_matches('_').eq_ignore_ascii_case("SelAll") {
            let ids = self
                .document
                .selectable_objects()
                .filter(|o| prompt.filter().accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_project_objects(ids, SelectionMode::Add);
        } else if self.commands.recognizes(
            input
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches(['_', '-']),
        ) {
            self.cancel_intersection_prompt(true);
            return false;
        } else if let Ok(ids) = input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>()
        {
            self.select_project_objects(ids, SelectionMode::Add);
        } else if self.picking_project_direction() {
            self.log_intersection_prompt();
        } else {
            self.push_log("Pick objects, type object IDs, or press Enter to continue".into());
        }
        self.command_input.clear();
        true
    }
    pub(super) fn picking_project_direction(&self) -> bool {
        matches!(
            self.active_command,
            Some(InteractiveCommand::ProjectDirection { .. })
        )
    }
    fn clear_project_direction(&mut self) {
        if self.picking_project_direction() {
            self.active_command = None;
            self.drafting_plane = None;
            self.point_constraint = None;
            self.point_filter = None;
            self.snaps.model_override = None;
        }
    }
    fn begin_project_direction(&mut self) {
        self.active_command = Some(InteractiveCommand::ProjectDirection { start: None });
        self.drafting_plane = None;
        self.point_constraint = None;
        self.point_filter = None;
        self.command_input.clear();
        self.log_intersection_prompt();
    }
    pub(super) fn accept_project_direction_point(
        &mut self,
        start: Option<Point3>,
        point: Point3,
    ) -> bool {
        let Some(mut prompt) = self
            .intersection_prompt
            .clone()
            .filter(|p| p.projection.is_some())
        else {
            return false;
        };
        if let Some(start) = start {
            let direction = match start.direction_to(point) {
                Ok(d) => d.as_vector(),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            };
            let session = prompt.projection.as_mut().unwrap();
            let [x, y, z] = direction.to_array();
            session.options.direction = format!("{x},{y},{z}");
            session.custom_requested = false;
            self.project_options = session.options.clone();
            self.clear_project_direction();
            self.intersection_prompt = Some(prompt.clone());
            self.finish_deferred_project(prompt);
        } else {
            self.active_command = Some(InteractiveCommand::ProjectDirection { start: Some(point) });
            self.log_intersection_prompt();
        }
        true
    }

    fn finish_deferred_project(&mut self, mut prompt: TwoSetsPrompt) {
        if let Some(targets) = prompt.projection.as_mut().unwrap().finish_targets.take() {
            let command = project_command(&prompt, prompt.first.as_ref().unwrap(), &targets);
            if !self.try_execute_command(&command) {
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
        } else {
            self.intersection_prompt = Some(prompt);
            self.log_intersection_prompt();
        }
    }

    pub(super) fn select_project_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        if self.picking_project_direction() {
            return;
        }
        let ids = ids.into_iter().collect::<Vec<_>>();
        if self.pick_project_plane_reference(&ids) {
            return;
        }
        if self.pick_uv_subcurve_source(ids.iter().copied()) {
            return;
        }
        let Some(prompt) = &self.intersection_prompt else {
            return;
        };
        let requested = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|o| requested.contains(&o.id()) && prompt.filter().accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        match self.document.select_objects_direct(ids, mode) {
            Ok(n) => self.push_log(format!("Selected {n} object(s); Enter continues")),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
}
fn project_command(prompt: &TwoSetsPrompt, sources: &[ObjectId], targets: &[ObjectId]) -> String {
    let sources = if sources.is_empty() {
        String::new()
    } else {
        format!(" Sources={}", project_ids(sources))
    };
    let targets = if targets.is_empty() {
        String::new()
    } else {
        format!(" Targets={}", project_ids(targets))
    };
    let planes = prompt
        .projection
        .as_ref()
        .unwrap()
        .planes
        .iter()
        .map(|p| format!(" InfinitePlane={p}"))
        .collect::<String>();
    format!(
        "{}{}{}{}{}",
        prompt.projection.as_ref().unwrap().command(),
        sources,
        targets,
        prompt.uv_subcurves.arguments(),
        planes,
    )
}
pub(super) fn project_ids(ids: &[ObjectId]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
