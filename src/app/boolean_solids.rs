//! Startup for solid and mesh Booleans with two successive object selections.
use super::intersect_two_sets::TwoSetsPrompt;
use super::*;
use viboceros_document::{ObjectId, SelectionMode};

mod input;
mod mesh_partition;
mod mesh_trim;
pub(super) use mesh_partition::PartitionOptions;
mod options;
mod selection;
pub(super) use options::{BooleanOptions, BooleanPromptKind};

impl VibocerosApp {
    pub(super) fn try_start_boolean_solids_prompt(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        let Some(name) = words.first() else {
            return false;
        };
        let name = name.trim_start_matches(['_', '-']);
        let Some(kind) = BooleanPromptKind::from_name(name) else {
            return false;
        };
        // Explicit sets are available to scripts through the command backend.
        if words.iter().skip(1).any(|w| {
            w.trim_start_matches('_')
                .split('=')
                .next()
                .is_some_and(|n| {
                    n.eq_ignore_ascii_case("FirstSet")
                        || n.eq_ignore_ascii_case("SecondSet")
                        || n.eq_ignore_ascii_case("Target")
                })
        }) {
            return false;
        }
        let description = match self.commands.object_selection_prompt(input) {
            Ok(Some(p)) => p,
            _ => return false,
        };
        if (!kind.is_intersection() || kind.mesh_tolerance().is_some())
            && let Err(error) = self.commands.accept_object_selection_input(input)
        {
            self.push_log(format!("Error: {error}"));
            return true;
        }
        let first = self
            .document
            .objects()
            .filter(|o| self.document.is_selected(o.id()) && kind.filter().accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let preselected_first = !first.is_empty();
        let _ = self.document.select_command_results(first.iter().copied());
        self.intersection_prompt = Some(TwoSetsPrompt {
            curve_extraction: None,
            curve_deviation: None,
            deviation: None,
            projection: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            first: preselected_first.then_some(first),
            output_layer: "Current",
            original_selection: vec![],
            boolean: Some(BooleanOptions {
                kind: kind.with_mesh_tolerance(
                    description
                        .choices
                        .iter()
                        .find(|o| o.name == "Tolerance")
                        .map_or("Auto", |o| o.value),
                ),
                delete_cutters: description
                    .options
                    .iter()
                    .find(|o| o.name == "DeleteCutters")
                    .is_none_or(|o| o.value),
                delete_input: description
                    .options
                    .iter()
                    .find(|o| o.name == "DeleteInput")
                    .is_none_or(|o| o.value),
                partition: (kind.is_mesh_partition() || kind.is_mesh_trim())
                    .then(|| PartitionOptions::from_prompt(&description)),
                preselected_first,
            }),
        });
        if kind.is_mesh_trim() && preselected_first {
            self.document.clear_selection();
            self.begin_mesh_trim_session();
        }
        if kind.shares_sets() && preselected_first {
            self.document.clear_selection();
        }
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }
}
