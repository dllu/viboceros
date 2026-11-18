//! Surface face source selection followed by shared point input for the apex.
use super::*;
use viboceros_command::extrude_surface_apex::{ApexPoint, SurfaceApexInput, SurfaceApexOptions};
impl VibocerosApp {
    pub(super) fn try_start_surface_apex_input(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ExtrudeSrfToPoint")
        }) {
            return false;
        }
        let Ok(parsed) = SurfaceApexInput::parse_with_options(
            &words.collect::<Vec<_>>(),
            self.commands.surface_apex_options(),
        ) else {
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
        if let Some(apex) = parsed.apex {
            let plane = self.viewports[self.active_viewport].construction_plane();
            if self.accept_drafting_point(apex.resolve(plane)) {
                return true;
            }
        }
        if let Some(sources) = &mut self.surface_extrusion_sources {
            sources.apex_draft = Some(crate::viewport::surface_apex::SurfaceApexDraft::capture(
                &self.document,
                &sources.targets,
            ));
        }
        self.log_surface_apex(parsed.options);
        true
    }
    pub(super) fn try_continue_surface_apex(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeSurfaceToPoint { mut options }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.set_view_prompt.is_some() {
            return false;
        }
        if let Some(apex) = ApexPoint::origin_token(input) {
            let plane = self.viewports[self.active_viewport].construction_plane();
            self.accept_filtered_drafting_point(apex.resolve(plane), true);
            self.command_input.clear();
            return true;
        }
        if viboceros_drafting::PointInput::parse_length_with_units(input, self.document.units())
            .is_some()
        {
            self.push_log(
                "Enter an apex point, 0 for the CPlane origin, or w0 for world origin".into(),
            );
            self.command_input.clear();
            return true;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty()
            || !words
                .iter()
                .all(|w| w.contains('=') || SurfaceApexOptions::recognizes(w))
        {
            return false;
        }
        match options.update(&words) {
            Ok(()) => {
                self.active_command = Some(InteractiveCommand::ExtrudeSurfaceToPoint { options });
                self.log_surface_apex(options);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }

    fn log_surface_apex(&mut self, options: SurfaceApexOptions) {
        self.push_log(format!(
            "Point to extrude to ({})",
            options.command_options()
        ));
    }
}
