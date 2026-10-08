//! Two-phase BooleanIntersection, BooleanDifference and BooleanSplit picking.
use super::intersect_two_sets::{BooleanOptions, BooleanPromptKind, TwoSetsPrompt};
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};

impl VibocerosApp {
    pub(super) fn try_start_boolean_solids_prompt(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        let Some(name) = words.first() else {
            return false;
        };
        let name = name.trim_start_matches(['_', '-']);
        let kind = if name.eq_ignore_ascii_case("BooleanIntersection") {
            BooleanPromptKind::Intersection
        } else if name.eq_ignore_ascii_case("BooleanDifference") {
            BooleanPromptKind::Difference
        } else if name.eq_ignore_ascii_case("BooleanSplit") {
            BooleanPromptKind::Split
        } else {
            return false;
        };
        // Explicit sets are available to scripts through the command backend.
        if words.iter().skip(1).any(|w| {
            w.trim_start_matches('_')
                .split('=')
                .next()
                .is_some_and(|n| {
                    n.eq_ignore_ascii_case("FirstSet") || n.eq_ignore_ascii_case("SecondSet")
                })
        }) {
            return false;
        }
        let description = match self.commands.object_selection_prompt(input) {
            Ok(Some(p)) => p,
            _ => return false,
        };
        if kind != BooleanPromptKind::Intersection
            && let Err(error) = self.commands.accept_object_selection_input(input)
        {
            self.push_log(format!("Error: {error}"));
            return true;
        }
        let first = self
            .document
            .objects()
            .filter(|o| {
                self.document.is_selected(o.id())
                    && ObjectSelectionFilter::SurfaceComponents.accepts_object(o)
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        let preselected_first = !first.is_empty();
        let _ = self.document.select_command_results(first.iter().copied());
        self.intersection_prompt = Some(TwoSetsPrompt {
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            first: preselected_first.then_some(first),
            output_layer: "Current",
            original_selection: vec![],
            boolean: Some(BooleanOptions {
                kind,
                delete_cutters: description
                    .options
                    .iter()
                    .find(|o| o.name == "DeleteCutters")
                    .is_none_or(|o| o.value),
                delete_input: description.options[0].value,
                preselected_first,
            }),
        });
        if kind == BooleanPromptKind::Split && preselected_first {
            self.document.clear_selection();
        }
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.log_intersection_prompt();
        true
    }

    pub(super) fn continue_boolean_solids_prompt(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        let normalized = input.trim_start_matches(['_', '-']);
        if normalized.eq_ignore_ascii_case("Cancel") {
            self.cancel_intersection_prompt(true);
            return true;
        }
        if input.is_empty() {
            let picked = self
                .document
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
                .map(|o| o.id())
                .filter(|id| {
                    prompt.boolean.as_ref().unwrap().kind == BooleanPromptKind::Split
                        || !prompt
                            .first
                            .as_ref()
                            .is_some_and(|first| first.contains(id))
                })
                .collect::<Vec<_>>();
            if let Some(first) = &prompt.first {
                if picked.is_empty()
                    && (first.len() < 2
                        || prompt.boolean.as_ref().unwrap().kind != BooleanPromptKind::Intersection)
                {
                    self.push_log(
                        "Select at least one object in the second set; Esc cancels".into(),
                    );
                    self.command_input.clear();
                    return true;
                }
                let options = prompt.boolean.as_ref().unwrap();
                let ids = |set: &[ObjectId]| {
                    set.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let display = options.command_line(false);
                let mut command = format!("{} FirstSet={}", options.command_line(true), ids(first));
                if !picked.is_empty() {
                    command.push_str(&format!(" SecondSet={}", ids(&picked)));
                }
                self.intersection_prompt = None;
                self.push_log(format!("> {display}"));
                let context = viboceros_command::CommandContext {
                    construction_plane: self.viewports[self.active_viewport].construction_plane(),
                };
                let result = if options.preselected_first {
                    self.commands
                        .execute_in_context(&mut self.document, &command, context)
                } else {
                    self.commands
                        .execute_postselected(&mut self.document, &command, context)
                };
                match result {
                    Ok(message) => self.push_log(message),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            } else if picked.is_empty() {
                self.push_log("Select at least one surface or polysurface; Esc cancels".into());
            } else {
                prompt.first = Some(picked);
                if prompt.boolean.as_ref().unwrap().kind == BooleanPromptKind::Split {
                    self.document.clear_selection();
                }
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
            self.command_input.clear();
            return true;
        }
        let option = normalized.split(['=', ' ']).next().unwrap_or("");
        let cutters = option.eq_ignore_ascii_case("DeleteCutters");
        if option.eq_ignore_ascii_case("DeleteInput")
            || (cutters && prompt.boolean.as_ref().unwrap().kind == BooleanPromptKind::Difference)
        {
            if cutters && !prompt.boolean.as_ref().unwrap().delete_input {
                self.push_log("DeleteCutters is available when DeleteInput=Yes".into());
                self.command_input.clear();
                return true;
            }
            let words = input.split_whitespace().collect::<Vec<_>>();
            let value = match words.as_slice() {
                [word] => word.split_once('=').map(|(_, v)| v),
                [_, v] => Some(*v),
                _ => None,
            }
            .map(|v| v.trim_start_matches('_'));
            let value = match value {
                Some(v) if v.eq_ignore_ascii_case("Yes") => true,
                Some(v) if v.eq_ignore_ascii_case("No") => false,
                _ => {
                    self.push_log(format!("{option} must be Yes or No"));
                    self.command_input.clear();
                    return true;
                }
            };
            let options = prompt.boolean.as_mut().unwrap();
            if cutters {
                options.delete_cutters = value;
            } else {
                options.delete_input = value;
            }
            if options.kind != BooleanPromptKind::Intersection
                && let Err(error) = self
                    .commands
                    .accept_object_selection_input(&options.command_line(true))
            {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
            self.intersection_prompt = Some(prompt);
            self.log_intersection_prompt();
            self.command_input.clear();
            return true;
        }
        if normalized.eq_ignore_ascii_case("SelNone") {
            let _ = self.document.select_command_results(
                if prompt.boolean.as_ref().unwrap().kind == BooleanPromptKind::Split {
                    vec![]
                } else {
                    prompt.first.clone().unwrap_or_default()
                },
            );
            self.command_input.clear();
            return true;
        }
        if normalized.eq_ignore_ascii_case("SelAll") {
            let ids = self
                .document
                .selectable_objects()
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_boolean_solids_objects(ids, SelectionMode::Add);
            self.command_input.clear();
            return true;
        }
        if self
            .commands
            .recognizes(normalized.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_intersection_prompt(true);
            return false;
        }
        match input
            .split(',')
            .map(str::parse::<ObjectId>)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(ids) => self.select_boolean_solids_objects(ids, SelectionMode::Add),
            Err(_) => {
                self.push_log("Pick objects, type object IDs, or press Enter to continue".into())
            }
        }
        self.command_input.clear();
        true
    }

    pub(super) fn select_boolean_solids_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let Some(prompt) = &self.intersection_prompt else {
            return;
        };
        let first = prompt.first.clone().unwrap_or_default();
        let shared_sets = prompt.boolean.as_ref().unwrap().kind == BooleanPromptKind::Split;
        let eligible = self
            .document
            .selectable_objects()
            .filter(|o| ObjectSelectionFilter::SurfaceComponents.accepts_object(o))
            .map(|o| o.id())
            .collect::<std::collections::BTreeSet<_>>();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        // Add separately: bulk document selection sorts IDs, but metadata follows pick order.
        for id in ids {
            if eligible.contains(&id)
                && (shared_sets || !first.contains(&id))
                && let Err(error) = self.document.select_objects_direct([id], mode)
            {
                self.push_log(format!("Error: {error}"));
            }
        }
    }
}
