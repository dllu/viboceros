//! Shared component getter for mesh/SubD face merging and result selection replay.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::merge_faces::{MergeSelection, Outcome, Parsed, Results, Target};
impl VibocerosApp {
    pub(super) fn picking_merge_faces(&self) -> bool {
        matches!(self.crease_prompt, Some(crease::Prompt::Merge { .. }))
    }
    fn merge_targets(&self) -> Result<Vec<Target>, viboceros_command::CommandError> {
        Ok(self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| viboceros_command::CommandError::MergeFacesStale)?
            .into_iter()
            .filter(|p| Target::eligible(p.kind))
            .map(|p| Target {
                object: p.object,
                kind: p.kind,
                index: p.index,
            })
            .collect())
    }
    pub(super) fn try_start_merge_faces(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("MergeFaces")
        }) {
            return false;
        }
        let parsed = match Parsed::parse(
            &words.collect::<Vec<_>>(),
            self.commands.merge_faces_results(),
        ) {
            Ok(parsed) => parsed,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.commands.remember_merge_faces_results(parsed.results);
        let targets = match if parsed.component_getter() {
            self.merge_targets()
        } else {
            parsed.targets(&self.document)
        } {
            Ok(targets) => targets,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        if targets.is_empty() && parsed.component_getter() {
            self.document.clear_selection();
            self.component_selection.clear();
            self.crease_prompt = Some(crease::Prompt::Merge {
                results: parsed.results,
                tolerance: self.document.tolerance(),
                results_pending: false,
            });
            self.push_log(format!(
                "Select mesh and SubD faces, edges, and vertices (Results={})",
                parsed.results.name()
            ));
        } else {
            match MergeSelection::prepare(&self.document, targets, parsed.results)
                .and_then(|plan| plan.commit(&mut self.document))
            {
                Ok(out) => self.accept_merge_faces(out),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn continue_merge_faces(&mut self, input: &str) -> bool {
        let Some(crease::Prompt::Merge {
            results,
            tolerance,
            results_pending,
        }) = self.crease_prompt.as_ref()
        else {
            return false;
        };
        let (results, tolerance, results_pending) = (*results, *tolerance, *results_pending);
        let token = input.trim().trim_start_matches(['_', '-']);
        if results_pending
            && (token.is_empty()
                || token.eq_ignore_ascii_case("Enter")
                || Results::parse(token).is_some())
        {
            let value = Results::parse(token).unwrap_or(results);
            self.set_merge_results(value, tolerance, false);
            self.command_input.clear();
            return true;
        }
        if token.is_empty() || token.eq_ignore_ascii_case("Enter") {
            let targets = self.merge_targets();
            if targets.as_ref().is_ok_and(|t| t.is_empty()) {
                self.cancel_crease();
                self.command_input.clear();
                return true;
            }
            let result = if self.document.tolerance() != tolerance {
                Err(viboceros_command::CommandError::MergeFacesStale)
            } else {
                targets
                    .and_then(|targets| MergeSelection::prepare(&self.document, targets, results))
                    .and_then(|plan| plan.commit(&mut self.document))
            };
            match result {
                Ok(out) => {
                    self.crease_prompt = None;
                    self.accept_merge_faces(out);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else if token.eq_ignore_ascii_case("Cancel") {
            self.cancel_crease();
        } else if token.eq_ignore_ascii_case("SelNone") {
            self.component_selection.clear();
            self.document.clear_selection();
        } else if token
            .split(['=', ' '])
            .next()
            .is_some_and(|n| n.eq_ignore_ascii_case("Results"))
        {
            let value = token
                .split_once('=')
                .or_else(|| token.split_once(' '))
                .and_then(|(_, v)| Results::parse(v.trim()));
            if let Some(value) = value {
                self.set_merge_results(value, tolerance, false);
            } else if token.eq_ignore_ascii_case("Results") {
                self.set_merge_results(results, tolerance, true);
            } else {
                self.push_log("Results=Selected|NotSelected".into());
            }
        } else if token
            .split_whitespace()
            .next()
            .is_some_and(|n| self.commands.recognizes(n))
        {
            self.cancel_crease();
            return false;
        } else {
            self.push_log(
                "Select mesh or SubD components; Enter applies (Results=Selected|NotSelected)"
                    .into(),
            );
        }
        self.command_input.clear();
        true
    }
    fn set_merge_results(&mut self, results: Results, tolerance: Tolerance, results_pending: bool) {
        self.commands.remember_merge_faces_results(results);
        self.crease_prompt = Some(crease::Prompt::Merge {
            results,
            tolerance,
            results_pending,
        });
        self.push_log(if results_pending {
            format!("Results <{}> (Selected NotSelected)", results.name())
        } else {
            format!("Results={}", results.name())
        });
    }
    fn accept_merge_faces(&mut self, out: Outcome) {
        let pick = |t: Target| ComponentPick {
            object: t.object,
            kind: t.kind,
            index: t.index,
        };
        if !out.changed_objects.is_empty() {
            self.component_selection.remember_merge_result(
                &self.document,
                out.released_objects.clone(),
                out.replay_components.into_iter().map(pick).collect(),
            );
        }
        if let Err(e) = self.component_selection.accept_merge_components(
            &self.document,
            &out.released_objects,
            out.selected_components.into_iter().map(pick).collect(),
        ) {
            self.push_log(format!("Error: {e}"));
        }
        self.push_log(out.message);
    }
}

#[cfg(test)]
mod allocation_tests;
#[cfg(test)]
mod face_properties_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod custom_tests;
