//! Switch the initial angle point prompt to object selection.
use super::*;

impl VibocerosApp {
    pub(super) fn try_continue_angle(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Angle { points }) = self.active_command else {
            return false;
        };
        if self.plane_prompt.is_some()
            || self.object_prompt.is_some()
            || !input
                .trim()
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("TwoObjects")
        {
            return false;
        }
        self.command_input.clear();
        if points.iter().any(Option::is_some) {
            self.push_log("Error: TwoObjects is available before picking the first point".into());
            return true;
        }
        // Use the same preselection and postselection path as a complete
        // `Angle TwoObjects` invocation. End point input before dispatching.
        self.cancel_interactive_command(false);
        if !self.try_start_object_prompt("Angle TwoObjects") {
            self.execute_command("Angle TwoObjects");
        }
        true
    }
}
