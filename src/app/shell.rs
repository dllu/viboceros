//! Solid face picking with frozen component identities and repairable options.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_command::shell::{ShellInput, ShellOptions};
#[derive(Clone, Copy)]
pub(super) struct ShellPrompt {
    pub(super) options: ShellOptions,
    pub(super) tolerance: Tolerance,
    pub(super) awaiting_thickness: bool,
}
impl VibocerosApp {
    pub(super) fn shellable_picks(&self, picks: Vec<ComponentPick>) -> Vec<ComponentPick> {
        picks.into_iter().filter(|p| {
            p.kind == viboceros_command::ComponentSelectionKind::BrepFace
                && self.document.is_object_selectable(p.object)
                && self.document.object(p.object).is_some_and(|o| {
                    matches!(o.geometry(), Geometry::Brep(b) if b.is_solid() && b.faces().len() > 1 && p.index < b.faces().len())
                })
        }).collect()
    }
    pub(super) fn try_start_shell_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Shell")
        }) {
            return false;
        }
        let Ok(parsed) = ShellInput::parse(&words.collect::<Vec<_>>()) else {
            return false;
        };
        if parsed.faces.is_some() {
            return false;
        }
        let picks = match self.component_selection.checked_picks(&self.document) {
            Ok(p) => p,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        if self.shellable_picks(picks.clone()).len() != picks.len() {
            self.push_log(
                "Error: Shell requires face components of closed manifold polysurfaces".into(),
            );
            return true;
        }
        self.cancel_interactive_command(false);
        self.shell_prompt = Some(ShellPrompt {
            options: parsed.options,
            tolerance: self.document.tolerance(),
            awaiting_thickness: false,
        });
        self.command_input.clear();
        self.push_log(format!(
            "Shell: select faces to remove ({}); Enter applies, Esc cancels",
            parsed.options.command_options()
        ));
        true
    }
    pub(super) fn try_continue_shell(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.shell_prompt else {
            return false;
        };
        let input = input.trim();
        if self.component_selection.has_choices()
            && !prompt.awaiting_thickness
            && !input.is_empty()
            && input.chars().all(|c| c.is_ascii_digit())
        {
            return false;
        }
        if input.is_empty() {
            self.finish_shell_prompt();
            self.command_input.clear();
            return true;
        }
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_shell_prompt(true);
            self.command_input.clear();
            return true;
        }
        if input
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Thickness")
        {
            prompt.awaiting_thickness = true;
            self.component_selection.clear_choices();
            self.shell_prompt = Some(prompt);
            self.command_input.clear();
            self.push_log("Shell thickness (model units or unit suffix)".into());
            return true;
        }
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            return false;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        let mut normalized = Vec::new();
        if words.len() == 1 && !input.contains('=') {
            if let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            ) {
                match value {
                    Ok(n) => normalized.push(format!("Thickness={n}")),
                    Err(e) => {
                        self.push_log(format!("Error: {e}"));
                        self.command_input.clear();
                        return true;
                    }
                }
            } else {
                self.push_log(
                    "Error: enter a thickness or Thickness=distance / DeleteInput=Yes|No".into(),
                );
                self.command_input.clear();
                return true;
            }
        } else if words.iter().all(|w| w.contains('=')) {
            for word in words {
                if let Some((key, value)) = word.trim_start_matches('_').split_once('=')
                    && key.eq_ignore_ascii_case("Thickness")
                    && let Some(n) = viboceros_drafting::PointInput::parse_length_with_units(
                        value,
                        self.document.units(),
                    )
                {
                    match n {
                        Ok(n) => normalized.push(format!("Thickness={n}")),
                        Err(e) => {
                            self.push_log(format!("Error: {e}"));
                            self.command_input.clear();
                            return true;
                        }
                    }
                } else {
                    normalized.push(word.to_string());
                }
            }
        } else {
            self.push_log("Error: enter Thickness=distance / DeleteInput=Yes|No".into());
            self.command_input.clear();
            return true;
        }
        match prompt
            .options
            .update(&normalized.iter().map(String::as_str).collect::<Vec<_>>())
        {
            Ok(()) => {
                prompt.awaiting_thickness = false;
                self.shell_prompt = Some(prompt);
                self.push_log(prompt.options.command_options());
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn cancel_shell_prompt(&mut self, announce: bool) {
        if self.shell_prompt.take().is_some() {
            self.component_selection.clear();
            if announce {
                self.push_log("Shell canceled".into());
            }
        }
    }
    fn finish_shell_prompt(&mut self) {
        let Some(prompt) = self.shell_prompt.take() else {
            return;
        };
        if prompt.tolerance != self.document.tolerance() {
            self.shell_prompt = Some(prompt);
            self.push_log("Error: tolerance changed; select the shell faces again".into());
            return;
        }
        let picks = match self.component_selection.checked_picks(&self.document) {
            Ok(p) if !p.is_empty() => p,
            Ok(_) => {
                self.shell_prompt = Some(prompt);
                self.push_log("Error: select at least one face to remove".into());
                return;
            }
            Err(e) => {
                self.shell_prompt = Some(prompt);
                self.push_log(format!("Error: {e}"));
                return;
            }
        };
        let refs = picks
            .iter()
            .map(|p| format!("{}:{}", p.object, p.index))
            .collect::<Vec<_>>()
            .join(",");
        let command = format!("Shell {} FaceRefs={refs}", prompt.options.command_options());
        if self.try_execute_command(&command) {
            self.component_selection.clear();
        } else {
            self.shell_prompt = Some(prompt);
        }
    }
}
