//! Frozen surface targets for shared tapered distance and direction input.
use super::*;
use viboceros_command::extrude_surface_tapered::SurfaceTaperedInput;
impl VibocerosApp {
    pub(super) fn try_start_surface_tapered_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrfTapered")
        }) {
            return false;
        }
        let Ok(parsed) = SurfaceTaperedInput::parse(&words.collect::<Vec<_>>()) else {
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
        self.active_command = Some(InteractiveCommand::ExtrudeSurfaceTapered {
            base: None,
            options: parsed.options,
        });
        self.command_input.clear();
        self.push_log(self.active_command.unwrap().prompt().into());
        true
    }
}
