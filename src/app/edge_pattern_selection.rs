//! Standalone loop/ring selection: bright seeds, dim loop previews, no model history.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::{
    CommandError,
    component_selection::ComponentAction,
    edge_pattern_selection::{Pattern, Session, eligible},
};

pub(super) const HINT: &str =
    "Select mesh or SubD edges; Ctrl removes seeds; Enter accepts, Esc cancels";
#[derive(Debug, Default)]
pub(super) struct Prompt {
    session: Session,
}
impl Prompt {
    pub(super) fn command(&self) -> &'static str {
        self.session.pattern().command()
    }
    pub(super) fn preview(&self) -> &[ComponentPick] {
        self.session.preview()
    }
}
impl VibocerosApp {
    pub(super) fn start_edge_pattern(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        let pattern = match words.next().map(|s| s.trim_start_matches(['_', '-'])) {
            Some(s) if s.eq_ignore_ascii_case("SelEdgeLoop") => Pattern::Loop,
            Some(s) if s.eq_ignore_ascii_case("SelEdgeRing") => Pattern::Ring,
            _ => return false,
        };
        if words.next().is_some() {
            return false;
        }
        // Preserve eligible preselection across cancellation of another getter.
        let picks = match self.component_selection.checked_picks(&self.document) {
            Ok(p) => p.into_iter().filter(|p| eligible(*p)).collect::<Vec<_>>(),
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.command_input.clear();
        if !picks.is_empty() {
            let result = self
                .commands
                .component_selection()
                .replace(&self.document, picks);
            match result {
                Ok(()) => self.execute_command(pattern.command()),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else {
            self.component_selection.clear();
            self.edge_pattern_prompt = Some(Prompt {
                session: Session::new(pattern),
            });
            self.push_log(format!("{}: {HINT}", pattern.command()));
        }
        true
    }
    pub(super) fn pick_edge_pattern(&mut self, picks: Vec<ComponentPick>, action: ComponentAction) {
        let picks = picks
            .into_iter()
            .filter(|p| eligible(*p) && !self.document.is_selected(p.object))
            .collect::<Vec<_>>();
        let Some(p) = &mut self.edge_pattern_prompt else {
            return;
        };
        let result = p
            .session
            .pick(&self.document, &picks, action)
            .and_then(|()| {
                self.commands
                    .component_selection()
                    .replace(&self.document, p.session.seeds())
                    .map_err(|_| CommandError::EdgeLoopStale)
            });
        self.component_selection.clear_choices();
        if let Err(e) = result {
            self.push_log(format!("Error: {e}"));
        }
    }
    pub(super) fn finish_edge_pattern(&mut self, accept: bool) {
        let Some(p) = self.edge_pattern_prompt.take() else {
            return;
        };
        self.component_selection.clear();
        if accept {
            match p.session.finish(&self.document) {
                Ok(picks) => {
                    let n = picks.len();
                    match self
                        .commands
                        .component_selection()
                        .replace(&self.document, picks)
                    {
                        Ok(()) => self.push_log(format!("Selected {n} edge(s)")),
                        Err(e) => self.push_log(format!("Error: {e}")),
                    }
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        } else {
            self.push_log(format!("{} canceled", p.command()));
        }
        self.command_input.clear();
    }
    pub(super) fn validate_edge_pattern(&mut self) {
        if self
            .edge_pattern_prompt
            .as_ref()
            .is_some_and(|p| !p.session.current(&self.document))
        {
            self.finish_edge_pattern(false);
            self.push_log("Error: edge selection source changed; select again".into());
        }
    }
    pub(super) fn continue_edge_pattern(&mut self, input: &str) -> bool {
        if self.edge_pattern_prompt.is_none() {
            return false;
        }
        if self.component_selection.has_choices() && self.try_continue_component_choice(input) {
            return true;
        }
        let key = input.trim().trim_start_matches(['_', '-']);
        if key.is_empty() || key.eq_ignore_ascii_case("Enter") {
            self.finish_edge_pattern(true);
            return true;
        }
        if key == "!" || key.eq_ignore_ascii_case("Cancel") {
            self.finish_edge_pattern(false);
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|s| self.commands.recognizes(s))
        {
            self.finish_edge_pattern(false);
            return false;
        }
        self.command_input.clear();
        self.push_log(HINT.into());
        true
    }
}
