//! Answers and transitions between the first and second Boolean selection.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn continue_boolean_solids_prompt(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        let kind = prompt.boolean.as_ref().unwrap().kind;
        let normalized = input.trim_start_matches(['_', '-']);
        if kind.is_mesh_trim() {
            return self.continue_mesh_trim(prompt, input);
        }
        if normalized.eq_ignore_ascii_case("Cancel") {
            self.cancel_intersection_prompt(true);
            return true;
        }
        if kind.is_mesh_partition() {
            if prompt.plane_getter().is_some()
                || matches!(
                    normalized.to_ascii_lowercase().as_str(),
                    "ip" | "iplane" | "infiniteplane"
                )
            {
                return self.continue_project_plane(&prompt, input);
            }
            if !input.is_empty() && self.continue_mesh_partition_option(prompt.clone(), input) {
                return true;
            }
        }
        if input.is_empty() {
            let picked = self
                .document
                .selected_objects()
                .filter(|o| kind.filter_for(prompt.first.is_some()).accepts_object(o))
                .map(|o| o.id())
                .filter(|id| {
                    prompt.boolean.as_ref().unwrap().shares_sets()
                        || !prompt
                            .first
                            .as_ref()
                            .is_some_and(|first| first.contains(id))
                })
                .collect::<Vec<_>>();
            if let Some(first) = &prompt.first {
                if picked.is_empty()
                    && prompt
                        .boolean
                        .as_ref()
                        .and_then(|o| o.partition.as_ref())
                        .is_none_or(|p| p.planes.is_empty())
                    && (first.len() < 2 || !kind.allows_empty_second())
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
                self.push_log("Select at least one eligible object; Esc cancels".into());
            } else {
                prompt.first = Some(picked);
                if kind.shares_sets() {
                    self.document.clear_selection();
                }
                self.intersection_prompt = Some(prompt);
                self.log_intersection_prompt();
            }
            self.command_input.clear();
            return true;
        }
        let option = normalized.split(['=', ' ']).next().unwrap_or("");
        if kind.mesh_tolerance().is_some()
            && (option.eq_ignore_ascii_case("Tolerance")
                || option.eq_ignore_ascii_case("Auto")
                || option.eq_ignore_ascii_case("Document")
                || option.eq_ignore_ascii_case("Zero"))
        {
            let value = normalized
                .split_once('=')
                .map(|(_, v)| v)
                .or_else(|| input.split_whitespace().nth(1))
                .unwrap_or(option)
                .trim_start_matches('_');
            let value = if value.eq_ignore_ascii_case("Auto") {
                Some("Auto")
            } else if value.eq_ignore_ascii_case("Document") {
                Some("Document")
            } else if value.eq_ignore_ascii_case("Zero") {
                Some("Zero")
            } else {
                None
            };
            let Some(value) = value else {
                self.push_log("Tolerance: Auto, Document or Zero".into());
                self.command_input.clear();
                return true;
            };
            let options = prompt.boolean.as_mut().unwrap();
            options.kind = options.kind.with_mesh_tolerance(value);
            if let Err(error) = self
                .commands
                .accept_object_selection_input(&options.command_line(true))
            {
                self.push_log(format!("Error: {error}"));
            }
            self.intersection_prompt = Some(prompt);
            self.command_input.clear();
            self.log_intersection_prompt();
            return true;
        }
        let cutters = option.eq_ignore_ascii_case("DeleteCutters");
        if option.eq_ignore_ascii_case("DeleteInput")
            && kind.mesh_tolerance().is_some()
            && (!kind.is_difference() || prompt.first.is_none())
        {
            self.push_log("DeleteInput is available while selecting difference cutters".into());
            self.command_input.clear();
            return true;
        }
        if option.eq_ignore_ascii_case("DeleteInput")
            || (cutters && kind == BooleanPromptKind::Difference)
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
            if !options.kind.is_intersection()
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
                if prompt.boolean.as_ref().unwrap().shares_sets() {
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
}
