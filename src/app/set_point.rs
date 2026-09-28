//! Interactive target and option editing for the typed SetPt command.

use super::*;

impl VibocerosApp {
    pub(super) fn try_continue_set_point_option(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::SetPoint { options }) = self.active_command else {
            return false;
        };
        if !input.contains('=') {
            return false;
        }
        let mut updated = options;
        if let Err(error) = updated.update(input) {
            self.push_log(format!("Error: {error}"));
            return true;
        }
        if !updated.axes.contains(&true) {
            self.push_log("Error: SetPt needs at least one enabled axis".into());
            return true;
        }
        let command = InteractiveCommand::SetPoint { options: updated };
        self.active_command = Some(command);
        self.push_log(format!("SetPt options: {}", updated.command_options()));
        self.push_log(command.prompt().to_owned());
        self.command_input.clear();
        true
    }

    pub(super) fn try_finish_set_point(&mut self, input: &str) -> bool {
        if !input.is_empty()
            || !matches!(
                self.active_command,
                Some(InteractiveCommand::SetPoint { options }) if options.copy
            )
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.push_log("Finished SetPt".into());
        self.command_input.clear();
        true
    }
}
