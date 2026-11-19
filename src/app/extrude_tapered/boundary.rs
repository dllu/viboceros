//! Tapered extrusion cutter selection through the shared surface face picker.
use super::*;
use viboceros_command::surface_reference::BoundaryRef;
use viboceros_command::{
    CommandError,
    extrude_tapered::{TaperedBoundaryOutcome, TaperedBoundaryPreview},
};

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
        if !surface && stage == Stage::Boundary {
            let session = self.tapered_extrusion_session.as_mut().unwrap();
            let constraint = session.constraint(options);
            session.boundary_preview = session.hover.or_else(|| {
                let ray = self.viewports[self.active_viewport].pointer_ray()?;
                let point = ray.closest_point_on_axis(constraint.anchor, constraint.direction?)?;
                signed_height(constraint, point)
            });
        }
        self.set_tapered_stage(surface, stage, options);
        if let BoundaryInput::Reference(reference) = input {
            self.pick_tapered_boundary(reference, None);
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
                Ok(reference) => self.pick_tapered_boundary(reference, None),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    pub(in crate::app) fn pick_tapered_boundary(
        &mut self,
        reference: BoundaryRef,
        hit: Option<Point3>,
    ) {
        let Some(InteractiveCommand::ExtrudeTapered {
            surface,
            stage: Stage::Boundary,
            options,
        }) = self.active_command
        else {
            return;
        };
        if !surface {
            self.finish_curve_tapered_boundary(reference, options, hit);
            return;
        }
        let before = self.document.objects().len();
        if !self.finish_tapered_input(true, &format!("ToBoundary={reference}"), options)
            && self.document.objects().len() > before
        {
            // A failed cut can commit a fallback sweep. Complete that pick
            // while retaining the command error and its replayable result.
            self.cancel_interactive_command(false);
        }
    }

    fn finish_curve_tapered_boundary(
        &mut self,
        reference: BoundaryRef,
        options: TaperedExtrusionOptions,
        hit: Option<Point3>,
    ) {
        let Some(session) = self.tapered_extrusion_session.as_ref() else {
            return;
        };
        if !session.sources.is_current(&self.document) {
            self.push_log("Error: extrusion source changed; select the objects again".into());
            return;
        }
        let plane = session.plane;
        let preview = TaperedBoundaryPreview {
            distance: session.boundary_preview,
            pick_offset: hit
                .and_then(|point| session.constraint(options).anchor.vector_to(point).ok()),
        };
        self.selection_menu = None;
        self.push_log(format!(
            "> ExtrudeCrvTapered ToBoundary={reference} {}",
            options.command_options(false),
        ));
        let result = self.commands.execute_curve_tapered_boundary_pick(
            &mut self.document,
            reference,
            options,
            plane,
            preview,
        );
        if result.is_ok() {
            self.cancel_interactive_command(false);
        }
        let error = match result {
            Ok(TaperedBoundaryOutcome::Cut(count)) => {
                self.push_log(format!("Tapered {count} curve object(s) to the boundary"));
                return;
            }
            Ok(TaperedBoundaryOutcome::Fallback) => CommandError::SurfaceExtrusionBoundaryRejected,
            Err(error) => error,
        };
        self.command_file.fail_dispatch();
        self.push_log(format!("Error: {error}"));
    }
}
