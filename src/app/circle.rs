//! Options available at the first Circle point prompt.
use super::*;

impl VibocerosApp {
    pub(super) fn try_continue_circle(&mut self, input: &str) -> bool {
        if self.active_command != Some(InteractiveCommand::Circle { center: None })
            || self.plane_prompt.is_some()
            || self.object_prompt.is_some()
        {
            return false;
        }
        let option = input.trim().trim_start_matches('_');
        let next = if option.eq_ignore_ascii_case("2Point") {
            InteractiveCommand::CircleTwoPoint { first: None }
        } else if option.eq_ignore_ascii_case("3Point") {
            InteractiveCommand::CircleThreePoint { points: [None; 2] }
        } else {
            return false;
        };
        self.command_input.clear();
        self.active_command = Some(next);
        self.push_log(next.prompt().to_owned());
        true
    }
}
