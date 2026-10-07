//! ApplyCrv source selection followed by one underlying surface reference.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};

impl VibocerosApp {
    pub(super) fn try_start_apply_curves_prompt(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.len() != 1
            || !matches!(
                words[0]
                    .trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "applycrv" | "applycurves"
            )
        {
            return false;
        }
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let first = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::ApplyCurves.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.intersection_prompt = Some(TwoSetsPrompt {
            first: (!first.is_empty()).then_some(first),
            original_selection,
            output_layer: "Current",
            boolean: None,
            apply_curves: true,
        });
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }

    pub(super) fn continue_apply_curves_prompt(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if input.is_empty() {
            if prompt.first.is_none() {
                let ids = self
                    .document
                    .selected_objects()
                    .filter(|o| prompt.filter().accepts_object(o))
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                if ids.is_empty() {
                    self.push_log("Select World-XY curves or points; Esc cancels".into());
                } else {
                    prompt.first = Some(ids);
                    self.document.clear_selection();
                    self.intersection_prompt = Some(prompt);
                    self.log_intersection_prompt();
                }
            } else {
                let targets = self
                    .document
                    .selected_objects()
                    .filter(|o| prompt.filter().accepts_object(o))
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                if let [target] = targets.as_slice() {
                    self.finish_apply_curves(prompt, *target);
                } else {
                    self.push_log("Select one target surface; Esc cancels".into());
                }
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
                    .filter(|o| prompt.filter().accepts_object(o))
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                self.select_apply_curves_objects(ids, SelectionMode::Add);
            }
        } else if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_intersection_prompt(true);
            return false;
        } else if let Ok(ids) = input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>()
        {
            self.select_apply_curves_objects(ids, SelectionMode::Add);
        } else {
            self.push_log("Pick objects, type object IDs, or press Enter to continue".into());
        }
        self.command_input.clear();
        true
    }

    pub(super) fn select_apply_curves_objects(
        &mut self,
        requested: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let Some(prompt) = self.intersection_prompt.clone() else {
            return;
        };
        let requested = requested
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let ids = self
            .document
            .selectable_objects()
            .filter(|o| requested.contains(&o.id()) && prompt.filter().accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if prompt.first.is_some() {
            if let [target] = ids.as_slice() {
                self.finish_apply_curves(prompt, *target);
            } else if !ids.is_empty() {
                self.push_log("Select one target surface".into());
            }
        } else {
            let mode = if mode == SelectionMode::Replace {
                SelectionMode::Add
            } else {
                mode
            };
            match self.document.select_objects(ids, mode) {
                Ok(_) => {
                    let selected = self
                        .document
                        .selected_objects()
                        .filter(|o| prompt.filter().accepts_object(o))
                        .map(|o| o.id())
                        .collect::<Vec<_>>();
                    let n = selected.len();
                    let _ = self.document.select_command_results(selected);
                    self.push_log(format!("Selected {n} source(s); Enter continues"));
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
    }

    fn finish_apply_curves(&mut self, prompt: TwoSetsPrompt, target: ObjectId) {
        let first = prompt.first.clone().unwrap_or_default();
        if let Err(e) = self.document.select_command_results(first) {
            self.push_log(format!("Error: {e}"));
            return;
        }
        if !self.try_execute_command(&format!("ApplyCrv Surface={target}")) {
            self.document.clear_selection();
            self.intersection_prompt = Some(prompt);
            self.push_log("Select another target surface or Esc to cancel".into());
        }
    }
}
