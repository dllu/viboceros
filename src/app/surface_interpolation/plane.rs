//! Nested plane input returns to the existing interpolation point draft.
use super::*;
use crate::app::intersect_two_sets::TwoSetsPrompt;
use crate::app::project::{PlaneOperation, ProjectOptions, ProjectSession};

impl VibocerosApp {
    pub(in crate::app) fn discard_surface_interpolation_plane(&mut self) -> bool {
        if !self.picking_surface_interpolation_plane() {
            return false;
        }
        self.clear_project_plane_state();
        self.intersection_prompt = None;
        self.drafting_plane = None;
        self.active_command = Some(InteractiveCommand::InterpolateCurveOnSurface { anchor: None });
        self.command_input.clear();
        self.surface_interpolation_prompt();
        true
    }
    pub(in crate::app) fn picking_surface_interpolation_plane(&self) -> bool {
        self.intersection_prompt
            .as_ref()
            .and_then(|p| p.projection.as_ref())
            .is_some_and(|p| p.is_interpolation())
    }
    pub(super) fn begin_surface_interpolation_plane(&mut self) {
        self.intersection_prompt = Some(TwoSetsPrompt {
            curve_extraction: None,
            curve_deviation: None,
            deviation: None,
            first: Some(Vec::new()),
            output_layer: "Current",
            original_selection: self.document.selected_object_ids().collect(),
            boolean: None,
            uv_mapping: None,
            uv_face: None,
            uv_subcurves: Default::default(),
            projection: Some(ProjectSession {
                operation: PlaneOperation::InterpolateCurveOnSurface,
                options: ProjectOptions::default(),
                custom_requested: false,
                finish_targets: None,
                planes: Vec::new(),
                plane_getter: None,
            }),
        });
        self.begin_iplane_getter();
    }
    pub(in crate::app) fn finish_surface_interpolation_plane(&mut self, definition: &str) {
        let result = Support::capture_plane(
            &self.document,
            definition,
            self.viewports[self.active_viewport].construction_plane(),
        );
        self.finish_surface_interpolation_support(result);
    }
    pub(in crate::app) fn finish_surface_interpolation_frame(&mut self, frame: Frame3) {
        let result = Support::capture_frame(&self.document, frame);
        self.finish_surface_interpolation_support(result);
    }
    fn finish_surface_interpolation_support(
        &mut self,
        result: Result<Support, viboceros_command::CommandError>,
    ) {
        match result {
            Ok(target) => {
                self.clear_project_plane_state();
                self.intersection_prompt = None;
                self.drafting_plane = target.frame();
                self.surface_interpolation_session.as_mut().unwrap().target = Some(target);
                self.active_command =
                    Some(InteractiveCommand::InterpolateCurveOnSurface { anchor: None });
                self.component_selection.clear_choices();
                self.command_input.clear();
                self.surface_interpolation_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
