//! The GetObject phase precedes point input for command-first affine transforms.
use super::*;
use object_selection::{ObjectPromptPhase, PendingObjectCommand};
use viboceros_command::{ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow};

impl VibocerosApp {
    pub(super) fn start_transform_source_prompt(&mut self, command: &'static str) {
        self.object_prompt = Some(PendingObjectCommand {
            description: ObjectSelectionPrompt {
                command,
                filter: ObjectSelectionFilter::Any,
                options: Vec::new(),
                menus: Vec::new(),
                choices: Vec::new(),
                workflow: ObjectSelectionWorkflow::PointInputAfterSelection,
            },
            phase: ObjectPromptPhase::Selecting,
            postselected: true,
            subcurve_measurement: false,
            measurement_display_units: None,
            command_override: None,
            excluded_object: None,
            selection_before: None,
            cloud_removal: None,
            cloud_action_target: None,
            special_selection: None,
        });
        self.command_input.clear();
        self.push_log(format!(
            "Select objects for {command}; Enter continues, Esc cancels"
        ));
    }

    pub(super) fn try_continue_transform_source_prompt(&mut self, input: &str) -> bool {
        let Some(pending) = self.object_prompt.as_ref().filter(|pending| {
            pending.description.workflow == ObjectSelectionWorkflow::PointInputAfterSelection
        }) else {
            return false;
        };
        let command = pending.description.command;
        let word = input.trim_start_matches('_');
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_object_prompt(true);
            return true;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let sources = self.document.selected_object_ids().collect::<Vec<_>>();
            if sources.is_empty() {
                self.cancel_object_prompt(true);
            } else {
                // Accept GetObject picks without invoking cancellation's
                // selection cleanup. Point-phase Escape retains them.
                self.object_prompt = None;
                self.try_start_interactive_command_with_sources(command, Some(sources));
            }
            self.command_input.clear();
            return true;
        }
        if transform_prompt::copy_option(&input.split_whitespace().collect::<Vec<_>>(), false)
            .is_some()
            || viboceros_command::mirror::MirrorPlaneOption::from_token(word).is_some()
        {
            self.push_log("Select objects first; Enter continues to transform options".into());
            self.command_input.clear();
            return true;
        }
        let tokens = input.split_whitespace().collect::<Vec<_>>();
        if tokens.first().is_some_and(|token| {
            token
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SelID")
        }) {
            let id = match tokens.as_slice() {
                [_, id] => id.parse::<ObjectId>().ok(),
                _ => None,
            };
            match id {
                Some(id) => match self
                    .document
                    .select_objects_direct([id], SelectionMode::Add)
                {
                    Ok(count) => {
                        self.push_log(format!("Selected {count} object(s); Enter continues"))
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                },
                None => self.push_log("Error: SelID requires one object ID".into()),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
}
