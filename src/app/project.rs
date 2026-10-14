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
}
impl Default for ProjectOptions {
    fn default() -> Self {
        Self {
            direction: "CPlaneZ".into(),
            delete_input: false,
            output_layer: "Current",
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
    fn command(&self) -> String {
        format!(
            "Project Direction={} DeleteInput={} OutputLayer={}",
            self.direction,
            if self.delete_input { "Yes" } else { "No" },
            self.output_layer
        )
    }
}
impl VibocerosApp {
    pub(super) fn try_start_project_prompt(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Project")
        }) {
            return false;
        }
        let mut options = self.project_options.clone();
        let mut seen = std::collections::BTreeSet::new();
        for argument in words {
            let Some((key, _)) = argument.split_once('=') else {
                return false;
            };
            if !seen.insert(key.trim_start_matches('_').to_ascii_lowercase())
                || !options.argument(argument)
            {
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
        self.project_options = options.clone();
        if !first.is_empty() && !targets.is_empty() {
            self.try_execute_command(&format!(
                "{} Sources={} Targets={}",
                options.command(),
                project_ids(&first),
                project_ids(&targets)
            ));
            self.command_input.clear();
            return true;
        }
        self.document.clear_selection();
        self.intersection_prompt = Some(TwoSetsPrompt {
            first: (!first.is_empty()).then_some(first),
            original_selection,
            output_layer: "Current",
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            projection: Some(options),
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }
    pub(super) fn log_project_prompt(&mut self, prompt: &TwoSetsPrompt) {
        self.push_log(format!(
            "{}: {}",
            prompt.projection.as_ref().unwrap().command(),
            prompt.hint()
        ));
    }
    pub(super) fn continue_project_prompt(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if input.is_empty() {
            let selected = self
                .document
                .selected_objects()
                .filter(|o| prompt.filter().accepts_object(o))
                .map(|o| o.id())
                .collect::<Vec<_>>();
            if selected.is_empty() {
                self.push_log(prompt.hint().into());
                return true;
            }
            if let Some(first) = &prompt.first {
                let command = format!(
                    "{} Sources={} Targets={}",
                    prompt.projection.as_ref().unwrap().command(),
                    project_ids(first),
                    project_ids(&selected)
                );
                if !self.try_execute_command(&command) {
                    self.intersection_prompt = Some(prompt);
                    self.push_log("Adjust the targets or direction; Enter retries".into());
                }
            } else {
                prompt.first = Some(selected);
                self.document.clear_selection();
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
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
            let mut options = prompt.projection.clone().unwrap();
            if options.argument(input) {
                self.project_options = options.clone();
                prompt.projection = Some(options);
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            } else {
                self.push_log("Project options: Direction=CPlaneZ|View|x,y,z, DeleteInput=Yes|No, OutputLayer=Current|Input|TargetObject".into());
            }
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("SelNone")
        {
            self.document.clear_selection();
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
        } else {
            self.push_log("Pick objects, type object IDs, or press Enter to continue".into());
        }
        self.command_input.clear();
        true
    }
    pub(super) fn select_project_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
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
fn project_ids(ids: &[ObjectId]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
