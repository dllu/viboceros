//! Application preference prompt; answers create no model transaction.
use super::*;

impl VibocerosApp {
    pub(super) fn try_start_use_extrusions(&mut self, input: &str) -> bool {
        let mut tokens = input.split_whitespace();
        if !tokens.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("UseExtrusions")
        }) || tokens.next().is_some()
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.use_extrusions_prompt = true;
        self.push_log(format!("> {input}"));
        self.push_log(format!(
            "Planar extrusion output object type <{}>: Extrusion or Polysurface (Enter accepts, Esc cancels)",
            if self.commands.use_extrusions() {
                "Extrusion"
            } else {
                "Polysurface"
            }
        ));
        self.command_input.clear();
        true
    }

    pub(super) fn try_continue_use_extrusions(&mut self, input: &str) -> bool {
        if !self.use_extrusions_prompt {
            return false;
        }
        let answer = input.trim().trim_start_matches('_');
        if answer.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if answer.is_empty() || answer.eq_ignore_ascii_case("Enter") {
            self.use_extrusions_prompt = false;
            self.execute_command("UseExtrusions");
        } else if ["Extrusion", "Polysurface"]
            .iter()
            .any(|value| answer.eq_ignore_ascii_case(value))
        {
            self.use_extrusions_prompt = false;
            self.execute_command(&format!("UseExtrusions {answer}"));
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_interactive_command(false);
            return false;
        } else {
            self.push_log("Enter Extrusion or Polysurface; Enter accepts, Esc cancels".into());
        }
        self.command_input.clear();
        true
    }
}
