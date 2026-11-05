//! Shared options remain editable until the apex is picked.
use super::*;
use viboceros_command::extrude_apex::Options;

pub(super) fn start(arguments: &[&str]) -> Option<InteractiveCommand> {
    let mut options = Options::default();
    options.update(arguments).ok()?;
    Some(InteractiveCommand::ExtrudeCurveToPoint { options })
}
impl VibocerosApp {
    pub(super) fn continue_curve_apex_options(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeCurveToPoint { mut options }) = self.active_command
        else {
            return false;
        };
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() || !words.iter().all(|w| w.contains('=')) {
            return false;
        }
        match options.update(&words) {
            Ok(()) => {
                self.active_command = Some(InteractiveCommand::ExtrudeCurveToPoint { options });
                self.push_log(options.command_options());
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
}
