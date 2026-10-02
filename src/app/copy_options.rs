//! Application preference prompt; answers create no model transaction.
use super::*;

impl VibocerosApp {
    pub(super) fn try_start_remember_copy_options(&mut self, input: &str) -> bool {
        let mut tokens = input.split_whitespace();
        if !tokens.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("RememberCopyOptions")
        }) || tokens.next().is_some()
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.remember_copy_prompt = true;
        self.push_log(format!("> {input}"));
        self.push_log(format!(
            "Remember copy options <{}>: Yes or No (Enter accepts, Esc cancels)",
            if self.commands.remember_copy_options() {
                "Yes"
            } else {
                "No"
            }
        ));
        self.command_input.clear();
        true
    }

    pub(super) fn try_continue_remember_copy_options(&mut self, input: &str) -> bool {
        if !self.remember_copy_prompt {
            return false;
        }
        let answer = input.trim().trim_start_matches('_');
        if answer.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if answer.is_empty() || answer.eq_ignore_ascii_case("Enter") {
            self.remember_copy_prompt = false;
            self.execute_command("RememberCopyOptions");
        } else if ["Yes", "No"]
            .iter()
            .any(|value| answer.eq_ignore_ascii_case(value))
        {
            self.remember_copy_prompt = false;
            self.execute_command(&format!("RememberCopyOptions {answer}"));
        } else if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            self.cancel_interactive_command(false);
            return false;
        } else {
            self.push_log("Enter Yes or No; Enter accepts, Esc cancels".into());
        }
        self.command_input.clear();
        true
    }
}
