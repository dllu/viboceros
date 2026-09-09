//! Named-target group commands collect input without opening model history.
use super::*;
use viboceros_document::{ObjectId, SelectionMode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GroupPrompt {
    Sources { target: Option<String> },
    Target,
}

impl GroupPrompt {
    pub(super) fn hint(&self) -> &'static str {
        match self {
            Self::Sources { .. } => "Select objects to add; Enter continues, Esc cancels",
            Self::Target => "Pick a grouped object or type the target group name; Esc cancels",
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_start_group_prompt(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("AddToGroup")
        }) {
            return false;
        }
        let target = words.collect::<Vec<_>>().join(" ");
        if !target.is_empty() && self.document.selected_object_count() > 0 {
            return false;
        }
        if !target.is_empty() && self.document.group_by_name(&target).is_none() {
            self.push_log(format!("Error: group '{target}' does not exist"));
            return true;
        }
        self.cancel_interactive_command(false);
        self.group_prompt = Some(if self.document.selected_object_count() == 0 {
            GroupPrompt::Sources {
                target: (!target.is_empty()).then_some(target),
            }
        } else {
            GroupPrompt::Target
        });
        self.command_input.clear();
        self.log_group_prompt();
        true
    }

    fn log_group_prompt(&mut self) {
        if let Some(prompt) = &self.group_prompt {
            self.push_log(format!("AddToGroup: {}", prompt.hint()));
        }
    }

    pub(super) fn try_continue_group_prompt(&mut self, input: &str) -> bool {
        let Some(prompt) = self.group_prompt.clone() else {
            return false;
        };
        let normalized = input.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if self.document.group_by_name(input).is_none()
            && matches!(normalized.as_str(), "selall" | "selnone")
        {
            if matches!(prompt, GroupPrompt::Sources { .. }) {
                if normalized == "selall" {
                    self.document.select_all();
                } else {
                    self.document.clear_selection();
                }
                self.command_input.clear();
            } else {
                self.push_log("Source selection is fixed; enter a target name or cancel".into());
            }
            return true;
        }
        if !input.is_empty()
            && self.document.group_by_name(input).is_none()
            && input
                .split_whitespace()
                .next()
                .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_group_prompt(true);
            return false;
        }
        let target = match &prompt {
            GroupPrompt::Sources { target } => {
                if !input.is_empty() || self.document.selected_object_count() == 0 {
                    self.log_group_prompt();
                    return true;
                }
                let Some(target) = target else {
                    self.group_prompt = Some(GroupPrompt::Target);
                    self.command_input.clear();
                    self.log_group_prompt();
                    return true;
                };
                target.clone()
            }
            GroupPrompt::Target => {
                if input.is_empty() {
                    self.log_group_prompt();
                    return true;
                }
                input.to_owned()
            }
        };
        self.group_prompt = None;
        if self.try_execute_command(&format!("AddToGroup {target}")) {
            self.command_input.clear();
        } else {
            // A removed/invalid target must remain correctable without losing
            // the source selection or silently completing the command.
            self.group_prompt = Some(GroupPrompt::Target);
            self.command_input = target;
        }
        true
    }

    pub(super) fn select_group_prompt_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        if !matches!(self.group_prompt, Some(GroupPrompt::Sources { .. })) {
            return;
        }
        let mode = if mode == SelectionMode::Replace {
            SelectionMode::Add
        } else {
            mode
        };
        if let Err(error) = self.document.select_objects(ids, mode) {
            self.push_log(format!("Error: {error}"));
        }
    }

    pub(super) fn cancel_group_prompt(&mut self, announce: bool) {
        if self.group_prompt.take().is_some() {
            self.command_input.clear();
            if announce {
                self.push_log("Cancelled AddToGroup".into());
            }
        }
    }

    pub(super) fn pick_group_prompt_target(&mut self, id: Option<ObjectId>) {
        let Some(id) = id else {
            return;
        };
        let group = self
            .document
            .object(id)
            .filter(|_| self.document.is_object_selectable(id))
            .and_then(|object| object.top_group());
        let Some(group) = group else {
            self.push_log("Pick a selectable object that belongs to a group".into());
            return;
        };
        match self
            .commands
            .execute_add_to_group(&mut self.document, group)
        {
            Ok(message) => {
                self.cancel_group_prompt(false);
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
