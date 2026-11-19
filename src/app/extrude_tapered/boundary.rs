//! Tapered extrusion cutter selection through the shared surface face picker.
use super::*;
use viboceros_command::surface_reference::BoundaryRef;

impl VibocerosApp {
    pub(in crate::app) fn picking_tapered_boundary(&self) -> bool {
        self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && matches!(
                self.active_command,
                Some(InteractiveCommand::ExtrudeTapered {
                    stage: Stage::Boundary,
                    ..
                })
            )
    }

    pub(super) fn open_tapered_boundary_or_distance(
        &mut self,
        surface: bool,
        input: BoundaryInput,
        options: TaperedExtrusionOptions,
    ) {
        let stage = if input == BoundaryInput::None {
            Stage::Distance
        } else {
            Stage::Boundary
        };
        self.set_tapered_stage(surface, stage, options);
        if let BoundaryInput::Reference(reference) = input {
            self.pick_tapered_boundary(reference);
        }
    }

    pub(super) fn continue_tapered_boundary(
        &mut self,
        surface: bool,
        input: &str,
        options: TaperedExtrusionOptions,
    ) {
        if surface && input.eq_ignore_ascii_case("Direction") {
            self.set_tapered_stage(true, Stage::DirectionStart { to_boundary: true }, options);
        } else if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            self.cancel_current_prompt_or_selection();
        } else {
            let reference = input
                .split_once(char::is_whitespace)
                .filter(|(name, _)| {
                    name.trim_start_matches(['_', '\''])
                        .eq_ignore_ascii_case("SelID")
                })
                .map_or(input, |(_, id)| id.trim());
            match BoundaryRef::parse(reference) {
                Ok(reference) => self.pick_tapered_boundary(reference),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    pub(in crate::app) fn pick_tapered_boundary(&mut self, reference: BoundaryRef) {
        let Some(InteractiveCommand::ExtrudeTapered {
            surface,
            stage: Stage::Boundary,
            options,
        }) = self.active_command
        else {
            return;
        };
        let before = self.document.objects().len();
        if !self.finish_tapered_input(surface, &format!("ToBoundary={reference}"), options)
            && self.document.objects().len() > before
        {
            // A failed cut can commit a fallback sweep. Complete that pick
            // while retaining the command error and its replayable result.
            self.cancel_interactive_command(false);
        }
    }
}
