//! Single ordered picks for the two planar surface commands.
use super::*;
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::ObjectId;
#[derive(Debug)]
pub(super) struct Prompt {
    pub(super) command: &'static str,
    first: Option<ObjectId>,
}
impl VibocerosApp {
    pub(super) fn start_planar_boolean(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.len() != 1 {
            return false;
        }
        let command = match words[0]
            .trim_start_matches(['_', '-'])
            .to_ascii_lowercase()
            .as_str()
        {
            "planardifference" => "PlanarDifference",
            "planarintersection" => "PlanarIntersection",
            _ => return false,
        };
        let selected = self
            .document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.planar_boolean_prompt = Some(Prompt {
            command,
            first: None,
        });
        self.push_log(format!("> {input}"));
        if selected.len() == 2 && command == "PlanarIntersection" {
            self.accept_planar_boolean([selected[0], selected[1]]);
        } else {
            if selected.len() == 1 && command == "PlanarIntersection" {
                self.planar_boolean_prompt.as_mut().unwrap().first = Some(selected[0]);
            }
            self.document.clear_selection();
            self.log_planar_boolean();
        }
        true
    }
    fn log_planar_boolean(&mut self) {
        if let Some(p) = &self.planar_boolean_prompt {
            self.push_log(format!(
                "{}: {}; Esc cancels",
                p.command,
                if p.first.is_some() {
                    "select the second coplanar surface"
                } else {
                    "select the first planar surface"
                }
            ));
        }
    }
    pub(super) fn cancel_planar_boolean(&mut self) {
        if self.planar_boolean_prompt.take().is_some() {
            self.document.clear_selection();
        }
    }
    pub(super) fn continue_planar_boolean(&mut self, input: &str) -> bool {
        if self.planar_boolean_prompt.is_none() {
            return false;
        }
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_planar_boolean();
            return true;
        }
        if input.is_empty() {
            self.log_planar_boolean();
            return true;
        }
        if let Ok(id) = input.parse::<ObjectId>() {
            self.pick_planar_boolean(Some(id));
            self.command_input.clear();
            return true;
        }
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_planar_boolean();
            return false;
        }
        self.push_log("Select a planar surface or enter its object ID".into());
        self.command_input.clear();
        true
    }
    pub(super) fn pick_planar_boolean(&mut self, id: Option<ObjectId>) -> bool {
        let Some(p) = self.planar_boolean_prompt.as_ref() else {
            return false;
        };
        let Some(id) = id.filter(|id| {
            self.document.is_object_selectable(*id)
                && self
                    .document
                    .object(*id)
                    .is_some_and(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
        }) else {
            return true;
        };
        if let Some(first) = p.first {
            if id != first {
                self.accept_planar_boolean([first, id]);
            }
        } else {
            self.planar_boolean_prompt.as_mut().unwrap().first = Some(id);
            self.document.clear_selection();
            self.log_planar_boolean();
        }
        true
    }
    fn accept_planar_boolean(&mut self, ids: [ObjectId; 2]) {
        let command = self.planar_boolean_prompt.take().unwrap().command;
        let input = format!("{command} Sources={},{}", ids[0], ids[1]);
        match self.commands.execute(&mut self.document, &input) {
            Ok(message) => self.push_log(message),
            Err(error) => self.push_log(format!("Error: {error}")),
        };
        self.command_input.clear();
    }
}
