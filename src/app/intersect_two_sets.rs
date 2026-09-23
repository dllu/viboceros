//! Two successive object selections for the intersection command.

use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};

#[derive(Clone, Debug)]
pub(super) struct TwoSetsPrompt {
    pub(super) first: Option<Vec<ObjectId>>,
    pub(super) output_layer: &'static str,
    original_selection: Vec<ObjectId>,
}

impl TwoSetsPrompt {
    pub(super) fn hint(&self) -> &'static str {
        if self.first.is_some() {
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
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }

    fn log_intersection_prompt(&mut self) {
        if let Some(prompt) = &self.intersection_prompt {
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
        if self.intersection_prompt.is_none() {
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
            if announce {
                let _ = self
                    .document
                    .select_objects_direct(prompt.original_selection, SelectionMode::Replace);
                self.push_log("Cancelled IntersectTwoSets".into());
            }
        }
    }
}
