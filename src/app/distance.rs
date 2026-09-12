//! Measurement-local input revision, separate from document undo/redo.
use super::*;

impl VibocerosApp {
    pub(super) fn try_continue_distance(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Distance {
            start: Some(_),
            previous_last,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some()
            || self.object_prompt.is_some()
            || !input
                .trim()
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Undo")
        {
            return false;
        }
        let next = InteractiveCommand::Distance {
            start: None,
            previous_last,
        };
        self.active_command = Some(next);
        self.last_point = previous_last;
        self.drafting_plane = None;
        self.command_input.clear();
        self.push_log(next.prompt().to_owned());
        true
    }
}
