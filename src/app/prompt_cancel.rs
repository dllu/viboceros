//! Escape-key handling for prompts and transient selections.
use super::*;

impl VibocerosApp {
    pub(super) fn cancel_current_prompt_or_selection(&mut self) {
        if self.cancel_face_corner() {
            return;
        }
        if self.discard_surface_offset_subcurve() {
            return;
        }
        if self.viewport_tab_rename.take().is_some() {
            // Escape dismisses the rename editor without canceling a modeling prompt.
        } else if self.selection_menu.take().is_some() {
            // Escape dismisses the choice without changing the selection.
        } else if self.dismiss_reparameterize_menu() {
            // Keep the reference selection active after dismissing its choices.
        } else if self
            .orient_edge_session
            .as_ref()
            .is_some_and(|s| s.has_subcurve())
        {
            self.continue_edge_subcurve("Cancel");
        } else if self.merge_face_prompt.take().is_some() {
            self.push_log("MergeCoplanarFace canceled".into());
        } else if self.shell_prompt.is_some() {
            self.cancel_shell_prompt(true);
        } else if self.surface_path_prompt.take().is_some() {
            self.push_log("ExtrudeSrfAlongCrv canceled".into());
        } else if self.copy_cplane_source.take().is_some() {
            self.push_log("CopyCPlane source pick canceled".into());
        } else if self.select_uv_prompt.is_some() {
            self.finish_select_uv();
        } else if self.refinement_prompt.is_some() {
            let name = self.refinement_prompt.as_ref().unwrap().command_name();
            self.cancel_refinement_input();
            if self.refinement_prompt.is_none() {
                self.push_log(format!("{name} canceled"));
            }
        } else if self.reparameterize_prompt.take().is_some() {
            self.push_log("Reparameterize canceled".into());
        } else if self.fillet_surface_prompt.is_some() {
            self.fillet_surface_prompt = None;
        } else if self.blend_surface_prompt.is_some() {
            self.blend_surface_prompt = None;
        } else if self.tween_surfaces_prompt.is_some() {
            self.cancel_tween_surfaces();
        } else if self.planar_boolean_prompt.is_some() {
            self.cancel_planar_boolean();
        } else if self.boolean_two_prompt.is_some() {
            self.cancel_boolean_two();
        } else if self.commands.curve_deviation_view(&self.document).is_some() {
            self.commands.clear_curve_deviation();
            self.push_log("CrvDeviation canceled".into());
        } else if self.remember_copy_prompt || self.edge_zoom_prompt.is_some() {
            self.cancel_interactive_command(true);
        } else if self.end_analysis_pick.is_some() && self.set_view_prompt.is_none() {
            self.cancel_end_analysis_pick(true);
        } else if self.zoom_target.take().is_some() {
            self.push_log("Zoom Target canceled".into());
        } else if self.zoom_factor_pending.take().is_some() {
            self.push_log("Zoom Factor canceled".into());
        } else if self.snap_size_pending.take().is_some() {
            self.push_log("SnapSize canceled".into());
        } else if self.zoom_window_pending {
            self.zoom_window_pending = false;
            self.push_log("Zoom window canceled".into());
        } else if self.set_view_prompt.is_some() {
            if self.plane_prompt.is_some() {
                self.cancel_plane_prompt();
            } else {
                self.cancel_set_view_prompt();
            }
        } else if self.selection_window_override.take().is_some() {
            self.push_log("Selection window canceled".into());
        } else if self.circular_selection.take().is_some() {
            self.push_log("Circular selection canceled".into());
        } else if self.boundary_selection.take().is_some() {
            self.push_log("Boundary selection canceled".into());
        } else if self.fence_selection.take().is_some() {
            self.push_log("Fence selection canceled".into());
        } else if self.lasso_selection.take().is_some() {
            self.push_log("Lasso selection canceled".into());
        } else if self.answer_object_prompt_escape() {
            // A command-owned warning consumed this Escape key.
        } else if self.plane_prompt.is_some() {
            self.cancel_plane_prompt();
        } else if self.active_command.is_some()
            || self.subcurve_prompt.is_some()
            || self.object_prompt.is_some()
            || self.group_prompt.is_some()
            || self.intersection_prompt.is_some()
            || self.edge_prompt.is_some()
            || self.hole_prompt.is_some()
            || self.unjoin_prompt.is_some()
            || self.edge_fillet_prompt.is_some()
            || self.crease_prompt.is_some()
        {
            self.cancel_interactive_command(true);
        } else if self.component_selection.has_choices() {
            self.component_selection.clear_choices();
        } else {
            let components = self.component_selection.valid_picks(&self.document).len();
            self.component_selection.clear();
            let count = self.document.clear_selection() + components;
            if count > 0 {
                self.push_log(format!("Deselected {count} object(s)"));
            } else {
                let owners = self.document.disable_control_points();
                if owners > 0 {
                    self.push_log(format!("Turned off grips on {owners} object(s)"));
                }
            }
        }
    }
}
