//! Edge picking and radius confirmation for the staged native fillet command.
use super::*;
use viboceros_command::{ComponentSelectionKind, FilletEdgeSelection};
#[derive(Clone, Copy, Debug)]
pub(super) struct Prompt {
    radius: f64,
    tolerance: Tolerance,
}
impl VibocerosApp {
    pub(super) fn try_start_edge_fillet(&mut self, input: &str) -> bool {
        if !input.split_whitespace().next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("FilletEdge")
        }) {
            return false;
        }
        let descriptor = match self.commands.component_selection_prompt(input) {
            Ok(Some(prompt)) => prompt,
            Ok(None) => return false,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.edge_fillet_prompt = Some(Prompt {
            radius: descriptor.numbers[0].value,
            tolerance: self.document.tolerance(),
        });
        self.command_input.clear();
        self.document.clear_selection();
        self.push_log(format!(
            "FilletEdge: select edges; Radius={}; Enter applies, Esc cancels",
            descriptor.numbers[0].value
        ));
        true
    }
    pub(super) fn try_continue_edge_fillet(&mut self, input: &str) -> bool {
        if self.edge_fillet_prompt.is_none() {
            return false;
        }
        let input = input.trim();
        if input.is_empty() {
            self.finish_edge_fillet(true);
            return true;
        }
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.finish_edge_fillet(false);
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.finish_edge_fillet(false);
            return false;
        }
        match self
            .commands
            .component_selection_prompt(&format!("FilletEdge {input}"))
        {
            Ok(Some(descriptor)) => {
                self.edge_fillet_prompt.as_mut().unwrap().radius = descriptor.numbers[0].value;
                self.push_log(format!(
                    "FilletEdge Radius={}; select edges or Enter applies",
                    descriptor.numbers[0].value
                ));
            }
            Ok(None) => self.push_log("Select edges, set Radius=value, or press Enter".into()),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn finish_edge_fillet(&mut self, apply: bool) {
        let Some(prompt) = self.edge_fillet_prompt.take() else {
            return;
        };
        let picks = self.component_selection.checked_picks(&self.document);
        self.component_selection.clear();
        self.command_input.clear();
        if !apply {
            self.push_log("Cancelled FilletEdge".into());
            return;
        }
        if prompt.tolerance != self.document.tolerance() {
            self.push_log("Error: tolerance changed; select edges again".into());
            return;
        }
        let result = picks
            .map_err(|e| e.to_string())
            .and_then(|picks| {
                FilletEdgeSelection::prepare(
                    &self.document,
                    picks
                        .into_iter()
                        .filter(|p| p.kind == ComponentSelectionKind::BrepEdge)
                        .map(|p| (p.object, p.index)),
                    prompt.radius,
                )
                .map_err(|e| e.to_string())
            })
            .and_then(|selection| {
                selection
                    .commit(&mut self.document)
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(count) => self.push_log(format!("Filleted edges on {count} object(s)")),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
