//! Object picking is a prompt phase, not point drafting or a model transaction.
use super::*;
use viboceros_document::{ObjectId, SelectionMode};

impl VibocerosApp {
    pub(super) fn try_start_object_prompt(&mut self, input: &str) -> bool {
        let prompt = match self.commands.object_selection_prompt(input) {
            Ok(Some(prompt)) => prompt,
            Ok(None) => return false,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        if self
            .document
            .selected_objects()
            .any(|o| prompt.filter.accepts(o.geometry()))
        {
            return false;
        }
        self.cancel_interactive_command(false);
        if let Err(error) = self.commands.accept_object_selection_options(&prompt) {
            self.push_log(format!("Error: {error}"));
            return true;
        }
        self.document.clear_selection();
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.push_log(format!(
            "{}: select meshes; Enter finishes, Esc cancels. {}",
            prompt.command,
            prompt.command_line()
        ));
        self.object_prompt = Some(prompt);
        true
    }

    pub(super) fn try_continue_object_prompt(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.object_prompt.clone() else {
            return false;
        };
        if input.is_empty() {
            if !self
                .document
                .selected_objects()
                .any(|o| prompt.filter.accepts(o.geometry()))
            {
                self.push_log("Select at least one mesh; Enter finishes, Esc cancels".into());
                return true;
            }
            let command = prompt.command_line();
            self.push_log(format!("> {command}"));
            match self.commands.execute_postselected(
                &mut self.document,
                &command,
                viboceros_command::CommandContext {
                    construction_plane: self.viewports[self.active_viewport].construction_plane(),
                },
            ) {
                Ok(message) => {
                    self.object_prompt = None;
                    self.push_log(message);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        let normalized = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if normalized == "selall" {
            let ids = self
                .document
                .objects()
                .filter(|o| {
                    self.document.is_object_selectable(o.id())
                        && prompt.filter.accepts(o.geometry())
                })
                .map(|o| o.id())
                .collect::<Vec<_>>();
            self.select_prompt_objects(ids, SelectionMode::Add);
            self.command_input.clear();
            return true;
        }
        if normalized == "selnone" {
            self.document.clear_selection();
            self.command_input.clear();
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_object_prompt(true);
            return false;
        }
        match prompt
            .update_options(input)
            .and_then(|()| self.commands.accept_object_selection_options(&prompt))
        {
            Ok(()) => {
                self.push_log(prompt.command_line());
                self.object_prompt = Some(prompt);
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn cancel_object_prompt(&mut self, announce: bool) {
        if let Some(prompt) = self.object_prompt.take() {
            self.document.clear_selection();
            self.command_input.clear();
            if announce {
                self.push_log(format!("Cancelled {}", prompt.command));
            }
        }
    }

    pub(super) fn select_prompt_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        let Some(prompt) = &self.object_prompt else {
            return;
        };
        let ids = ids
            .into_iter()
            .filter(|id| {
                self.document.object(*id).is_some_and(|o| {
                    self.document.is_object_selectable(*id) && prompt.filter.accepts(o.geometry())
                })
            })
            .collect::<Vec<_>>();
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        match self.document.select_objects_direct(ids, mode) {
            Ok(count) => self.push_log(format!("Selected {count} mesh(es); Enter finishes")),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
