//! Two successive object selections for the intersection command.

use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};

#[derive(Clone, Debug)]
pub(super) struct TwoSetsPrompt {
    pub(super) first: Option<Vec<ObjectId>>,
    pub(super) output_layer: &'static str,
    pub(super) original_selection: Vec<ObjectId>,
    pub(super) boolean: Option<BooleanOptions>,
    pub(super) uv_mapping: Option<super::apply_curves::UvMappingKind>,
    pub(super) uv_face: Option<usize>,
    pub(super) uv_subcurves: super::uv_subcurve_input::SubcurveInputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BooleanPromptKind {
    Intersection,
    Difference,
}
impl BooleanPromptKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Intersection => "BooleanIntersection",
            Self::Difference => "BooleanDifference",
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct BooleanOptions {
    pub(super) kind: BooleanPromptKind,
    pub(super) delete_input: bool,
    pub(super) delete_cutters: bool,
    pub(super) preselected_first: bool,
}
impl BooleanOptions {
    pub(super) fn command_line(&self, include_hidden: bool) -> String {
        let mut command = format!(
            "{} DeleteInput={}",
            self.kind.name(),
            if self.delete_input { "Yes" } else { "No" }
        );
        if self.kind == BooleanPromptKind::Difference && (include_hidden || self.delete_input) {
            command.push_str(&format!(
                " DeleteCutters={}",
                if self.delete_cutters { "Yes" } else { "No" }
            ));
        }
        command
    }
}

impl TwoSetsPrompt {
    pub(super) fn name(&self) -> &'static str {
        if let Some(kind) = self.uv_mapping {
            kind.name()
        } else {
            self.boolean
                .as_ref()
                .map_or("IntersectTwoSets", |o| o.kind.name())
        }
    }
    pub(super) fn filter(&self) -> ObjectSelectionFilter {
        if let Some(kind) = self.uv_mapping {
            if self.uv_subcurves.pending.is_some() {
                return ObjectSelectionFilter::Curves;
            }
            kind.filter(self.first.is_some())
        } else if self.boolean.is_some() {
            ObjectSelectionFilter::SurfaceComponents
        } else {
            ObjectSelectionFilter::Parametric
        }
    }
    pub(super) fn hint(&self) -> &'static str {
        if let Some(hint) = self.uv_subcurves.hint() {
            return hint;
        }
        if let Some(kind) = self.uv_mapping {
            kind.hint(self.first.is_some())
        } else if self
            .boolean
            .as_ref()
            .is_some_and(|o| o.kind == BooleanPromptKind::Difference)
        {
            if self.first.is_some() {
                "Select cutters; Enter subtracts, Esc cancels"
            } else {
                "Select targets; Enter continues, Esc cancels"
            }
        } else if self.boolean.is_some() && self.first.is_some() {
            "Select second set; empty Enter intersects the first set, Esc cancels"
        } else if self.first.is_some() {
            "Select second set; Enter intersects, Esc cancels"
        } else {
            "Select first set; Enter continues, Esc cancels"
        }
    }
}

fn output_layer_option(input: &str) -> Option<&'static str> {
    let (name, value) = input.trim_start_matches('_').split_once('=')?;
    if !name.eq_ignore_ascii_case("OutputLayer") {
        return None;
    }
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("Current") {
        Some("Current")
    } else if value.eq_ignore_ascii_case("FirstSet") {
        Some("FirstSet")
    } else if value.eq_ignore_ascii_case("SecondSet") {
        Some("SecondSet")
    } else {
        None
    }
}

impl VibocerosApp {
    pub(super) fn try_start_intersection_prompt(&mut self, input: &str) -> bool {
        if self.try_start_apply_curves_prompt(input) {
            return true;
        }
        if self.try_start_boolean_solids_prompt(input) {
            return true;
        }
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("IntersectTwoSets")
        }) {
            return false;
        }
        let arguments = words.collect::<Vec<_>>();
        let output_layer = match arguments.as_slice() {
            [] => "Current",
            [option] => {
                let Some(value) = output_layer_option(option) else {
                    return false;
                };
                value
            }
            _ => return false,
        };
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let first = self
            .document
            .selected_objects()
            .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
            .map(|object| object.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.intersection_prompt = Some(TwoSetsPrompt {
            first: (!first.is_empty()).then_some(first),
            output_layer,
            original_selection,
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }

    pub(super) fn log_intersection_prompt(&mut self) {
        if let Some(prompt) = &self.intersection_prompt {
            if prompt.uv_mapping.is_some() {
                self.push_log(format!("{}: {}", prompt.name(), prompt.hint()));
                return;
            }
            if let Some(options) = &prompt.boolean {
                self.push_log(format!(
                    "{}: {}",
                    options.command_line(false),
                    prompt.hint()
                ));
                return;
            }
            self.push_log(format!(
                "IntersectTwoSets: {}; OutputLayer={}",
                prompt.hint(),
                prompt.output_layer
            ));
        }
    }

    pub(super) fn try_continue_intersection_prompt(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.intersection_prompt.clone() else {
            return false;
        };
        if prompt.uv_mapping.is_some() {
            return self.continue_apply_curves_prompt(prompt, input);
        }
        if prompt.boolean.is_some() {
            return self.continue_boolean_solids_prompt(prompt, input);
        }
        if input.is_empty() {
            let selected = self
                .document
                .selected_objects()
                .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
                .map(|object| object.id())
                .collect::<Vec<_>>();
            if selected.is_empty() {
                self.push_log("Select at least one curve, surface, or B-rep; Esc cancels".into());
                return true;
            }
            if let Some(first) = &prompt.first {
                let first = first
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let second = selected
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let command = format!(
                    "IntersectTwoSets {first} {second} OutputLayer={}",
                    prompt.output_layer
                );
                if !self.try_execute_command(&command) {
                    self.intersection_prompt = Some(prompt);
                    self.push_log("Adjust the second set or output layer; Enter retries".into());
                }
            } else {
                prompt.first = Some(selected);
                self.document.clear_selection();
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
            self.command_input.clear();
            return true;
        }
        if input
            .trim_start_matches('_')
            .split_once('=')
            .is_some_and(|(name, _)| name.eq_ignore_ascii_case("OutputLayer"))
        {
            if let Some(value) = output_layer_option(input) {
                prompt.output_layer = value;
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            } else {
                self.push_log("OutputLayer must be Current, FirstSet, or SecondSet".into());
            }
            self.command_input.clear();
            return true;
        }
        let normalized = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if matches!(normalized.as_str(), "selall" | "selnone") {
            if normalized == "selnone" {
                self.document.clear_selection();
            } else {
                let ids = self
                    .document
                    .selectable_objects()
                    .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
                    .map(|object| object.id())
                    .collect::<Vec<_>>();
                self.select_intersection_prompt_objects(ids, SelectionMode::Add);
            }
            self.command_input.clear();
            return true;
        }
        if self.commands.recognizes(
            input
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches(['_', '-']),
        ) {
            self.cancel_intersection_prompt(true);
            return false;
        }
        let ids = input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>();
        if let Ok(ids) = ids {
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
            self.command_input.clear();
        } else {
            self.push_log("Pick objects, type object IDs, or press Enter to continue".into());
        }
        true
    }

    pub(super) fn select_intersection_prompt_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        if let Some(prompt) = &self.intersection_prompt {
            if prompt.uv_mapping.is_some() {
                self.select_apply_curves_objects(ids, mode);
                return;
            }
            if prompt.boolean.is_some() {
                self.select_boolean_solids_objects(ids, mode);
                return;
            }
        } else {
            return;
        }
        let requested = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|object| requested.contains(&object.id()))
            .filter(|object| ObjectSelectionFilter::Parametric.accepts_object(object))
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
    }

    pub(super) fn cancel_intersection_prompt(&mut self, announce: bool) {
        if let Some(prompt) = self.intersection_prompt.take() {
            self.command_input.clear();
            let name = prompt.name();
            if let Some(options) = prompt.boolean {
                let _ = self
                    .document
                    .select_command_results(prompt.first.unwrap_or_default());
                if announce {
                    self.push_log(format!("Cancelled {}", options.kind.name()));
                }
                return;
            }
            if announce {
                let _ = self
                    .document
                    .select_objects_direct(prompt.original_selection, SelectionMode::Replace);
                self.push_log(format!("Cancelled {name}"));
            }
        }
    }
}
