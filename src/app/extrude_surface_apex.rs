//! Surface face source selection followed by shared point input for the apex.
use super::*;
use viboceros_command::extrude_surface_apex::SurfaceApexInput;
impl VibocerosApp {
    pub(super) fn try_start_surface_apex_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrfToPoint")
        }) {
            return false;
        }
        let Ok(parsed) = SurfaceApexInput::parse(&words.collect::<Vec<_>>()) else {
            return false;
        };
        let sources = match self.prepare_surface_extrusion_sources(parsed.faces) {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.surface_extrusion_sources = Some(sources);
        self.active_command = Some(InteractiveCommand::ExtrudeSurfaceToPoint {
            options: parsed.options,
        });
        self.command_input.clear();
        if let Some(point) = parsed.apex {
            self.accept_drafting_point(point);
        } else {
            self.push_log(self.active_command.unwrap().prompt().into());
        }
        true
    }
    pub(super) fn try_continue_surface_apex(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeSurfaceToPoint { mut options }) = self.active_command
        else {
            return false;
        };
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() || !words.iter().all(|w| w.contains('=')) {
            return false;
        }
        match options.update(&words) {
            Ok(()) => {
                self.active_command = Some(InteractiveCommand::ExtrudeSurfaceToPoint { options });
                self.push_log(options.command_options());
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
}
