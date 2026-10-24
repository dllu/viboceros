//! CrvDeviation shares the nonmutating nested SubCrv getter with curve mapping.
use super::super::{
    intersect_two_sets::TwoSetsPrompt,
    uv_subcurve_input::{PendingSubcurve, SubcurveInputs},
};
use super::*;
use viboceros_document::{ObjectId, SelectionMode};
impl VibocerosApp {
    pub(in crate::app) fn try_start_curve_deviation_subcurve_prompt(
        &mut self,
        input: &str,
    ) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("CrvDeviation")
        }) {
            return false;
        }
        let mut bare = 0;
        let base = words
            .into_iter()
            .filter(|w| {
                if w.trim_start_matches('_').eq_ignore_ascii_case("SubCrv") {
                    bare += 1;
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let args = base.split_whitespace().skip(1).collect::<Vec<_>>();
        if bare != 1 || !viboceros_command::curve_deviation::accepts_subcurve_picker_options(&args)
        {
            return false;
        }
        self.begin_curve_deviation_sources(base, input);
        true
    }
    pub(in crate::app) fn promote_curve_deviation_subcurve(&mut self, input: &str) -> bool {
        if !input.trim_start_matches('_').eq_ignore_ascii_case("SubCrv")
            || self
                .object_prompt
                .as_ref()
                .is_none_or(|p| p.description.command != "CrvDeviation")
        {
            return false;
        }
        let p = self.object_prompt.take().unwrap();
        let base = p
            .command_override
            .unwrap_or_else(|| p.description.command_line());
        self.begin_curve_deviation_sources(base, "SubCrv");
        true
    }
    fn begin_curve_deviation_sources(&mut self, base: String, log: &str) {
        let original_selection = self.document.selected_object_ids().collect();
        self.cancel_interactive_command(false);
        self.intersection_prompt = Some(TwoSetsPrompt {
            curve_extraction: None,
            curve_deviation: Some(base),
            deviation: None,
            first: None,
            output_layer: "Current",
            original_selection,
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: SubcurveInputs {
                ranges: vec![],
                pending: Some(PendingSubcurve::default()),
                ..Default::default()
            },
            projection: None,
        });
        self.command_input.clear();
        self.push_log(format!("> {log}"));
        self.log_intersection_prompt();
    }
    pub(in crate::app) fn continue_curve_deviation_sources(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        if self.try_continue_uv_subcurve(&prompt, input) {
            return true;
        }
        if input.is_empty() {
            let owners = prompt
                .uv_subcurves
                .ranges
                .iter()
                .map(|r| r.object)
                .collect::<std::collections::BTreeSet<_>>();
            let count = prompt.uv_subcurves.ranges.len()
                + self
                    .document
                    .selected_objects()
                    .filter(|o| o.geometry().curve_ref().is_some() && !owners.contains(&o.id()))
                    .count();
            if count != 2 {
                self.push_log(format!(
                    "Select exactly two curve inputs; currently {count}"
                ));
                self.command_input.clear();
                return true;
            }
            let command = format!(
                "{}{}",
                prompt.curve_deviation.as_ref().unwrap(),
                prompt.uv_subcurves.arguments()
            );
            self.intersection_prompt = None;
            if self.try_execute_command(&command) {
                self.document.clear_selection();
            } else {
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
        } else if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("SelNone")
        {
            self.document.clear_selection();
            prompt.uv_subcurves.ranges.clear();
            self.intersection_prompt = Some(prompt);
        } else if input.trim_start_matches('_').eq_ignore_ascii_case("SelAll") {
            let ids = self
                .document
                .selectable_objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
        } else if input
            .split_once('=')
            .is_some_and(|(k, _)| k.trim_start_matches('_').eq_ignore_ascii_case("KeepMarks"))
        {
            let base = prompt
                .curve_deviation
                .as_ref()
                .unwrap()
                .split_whitespace()
                .filter(|w| {
                    !w.trim_start_matches('_')
                        .to_ascii_lowercase()
                        .starts_with("keepmarks=")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let candidate = format!("{base} {input}");
            match self.commands.object_selection_prompt(&candidate) {
                Ok(Some(_)) => {
                    prompt.curve_deviation = Some(candidate);
                    self.intersection_prompt = Some(prompt);
                    self.log_intersection_prompt();
                }
                _ => self.push_log("KeepMarks expects Yes or No".into()),
            }
        } else if self.commands.recognizes(
            input
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches(['_', '-']),
        ) {
            self.cancel_interactive_command(true);
            return false;
        } else if let Ok(ids) = input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>()
        {
            self.select_intersection_prompt_objects(ids, SelectionMode::Add);
        } else {
            self.push_log(prompt.hint().into());
        }
        self.command_input.clear();
        true
    }
}
