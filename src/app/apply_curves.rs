//! ApplyCrv source selection followed by one underlying surface reference.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UvMappingKind {
    Apply,
    Create,
}
impl UvMappingKind {
    pub(super) fn name(self) -> &'static str {
        if self == Self::Apply {
            "ApplyCrv"
        } else {
            "CreateUVCrv"
        }
    }
    pub(super) fn filter(self, second: bool) -> ObjectSelectionFilter {
        if second == (self == Self::Apply) {
            ObjectSelectionFilter::Surfaces
        } else {
            ObjectSelectionFilter::ApplyCurves
        }
    }
    pub(super) fn hint(self, second: bool) -> &'static str {
        match (self, second) {
            (Self::Apply, false) => {
                "Select World-XY curves and points; Enter continues, Esc cancels"
            }
            (Self::Apply, true) => "Select target surface; Esc cancels",
            (Self::Create, false) => "Select one surface; Esc cancels",
            (Self::Create, true) => {
                "Select optional curves and points; Enter creates UV objects, Esc cancels"
            }
        }
    }
}

impl VibocerosApp {
    fn continue_create_uv_curves_prompt(&mut self, prompt: TwoSetsPrompt, input: &str) -> bool {
        if input.is_empty() {
            if let Some(surface) = prompt.first.as_ref().and_then(|x| x.first()) {
                if !self.try_execute_command(&format!("CreateUVCrv Surface={surface}")) {
                    self.intersection_prompt = Some(prompt);
                }
            } else {
                self.push_log("Select one surface; Esc cancels".into());
            }
        } else if matches!(
            input.trim_start_matches('_').to_ascii_lowercase().as_str(),
            "selall" | "selnone"
        ) {
            if input
                .trim_start_matches('_')
                .eq_ignore_ascii_case("SelNone")
            {
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
            self.push_log(
                "Pick objects, type object IDs, or press Enter to create UV objects".into(),
            );
        }
        self.command_input.clear();
        true
    }
    pub(super) fn try_start_apply_curves_prompt(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.len() != 1
            || !matches!(
                words[0]
                    .trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "applycrv" | "applycurves" | "createuvcrv"
            )
        {
            return false;
        }
        let kind = if words[0]
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("CreateUVCrv")
        {
            UvMappingKind::Create
        } else {
            UvMappingKind::Apply
        };
        let original_selection = self.document.selected_object_ids().collect::<Vec<_>>();
        let mut first = self
            .document
            .selected_objects()
            .filter(|o| kind.filter(false).accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if kind == UvMappingKind::Create && first.len() != 1 {
            first.clear();
        }
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.intersection_prompt = Some(TwoSetsPrompt {
            first: (!first.is_empty()).then_some(first),
            original_selection,
            output_layer: "Current",
            boolean: None,
            uv_mapping: Some(kind),
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
        if prompt.uv_mapping == Some(UvMappingKind::Create) {
            return self.continue_create_uv_curves_prompt(prompt, input);
        }
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
        if prompt.first.is_some() == (prompt.uv_mapping == Some(UvMappingKind::Apply)) {
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

    fn finish_apply_curves(&mut self, mut prompt: TwoSetsPrompt, target: ObjectId) {
        if prompt.uv_mapping == Some(UvMappingKind::Create) {
            prompt.first = Some(vec![target]);
            self.document.clear_selection();
            self.intersection_prompt = Some(prompt);
            self.log_intersection_prompt();
            return;
        }
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
