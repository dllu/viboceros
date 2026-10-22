//! Cleanup and completion of interactive command sessions.
use super::*;

impl VibocerosApp {
    pub(super) fn cancel_interactive_command(&mut self, announce: bool) {
        self.release_arc_extension_pick();
        self.release_circle_around_picks();
        self.release_ellipse_picks();
        self.commands.clear_curve_deviation();
        self.set_point_sources = None;
        self.remap_cplane_session = None;
        self.surface_extrusion_sources = None;
        self.surface_path_prompt = None;
        self.cancel_shell_prompt(false);
        if self.merge_face_prompt.take().is_some() && announce {
            self.push_log("MergeCoplanarFace canceled".into());
        }
        self.finish_edge_zoom_options(announce);
        self.cancel_block_add_input();
        self.block_session = None;
        self.blend_surface_prompt = None;
        self.fillet_surface_prompt = None;
        self.finish_select_uv();
        self.refinement_prompt = None;
        self.reparameterize_prompt = None;
        self.cancel_tween_surfaces();
        self.cancel_planar_boolean();
        self.cancel_boolean_two();
        if self.subcurve_prompt.take().is_some() {
            self.document.clear_selection();
        }
        self.cancel_surface_array();
        self.cancel_curve_array();
        let transform_applied = self.finish_orient_surface_session()
            | self.finish_orient_points_session()
            | self.finish_face_create()
            | self.finish_mesh_polyline()
            | self.finish_surface_interpolation()
            | self.finish_short_path()
            | self.finish_surface_offset()
            | self.finish_orient_edge()
            | self.finish_orient_curve_session()
            | self.finish_array_curve_surface_session()
            | self.finish_maelstrom_session()
            | self.finish_transform_session()
            | self
                .translation_session
                .take()
                .is_some_and(|session| session.applied);
        self.cancel_crease();
        self.finish_edge_fillet(false);
        if self.unjoin_prompt.is_some() {
            self.finish_unjoin_command(false);
        }
        self.flow_session = None;
        self.surface_flow_session = None;
        self.catenary_session = None;
        self.ellipse_session = None;
        self.circle_around_session = None;
        self.tangent_session = None;
        self.arc_extension_session = None;
        self.arc_tilted_session = None;
        self.arc_deform_session = None;
        self.arc_length_session = None;
        self.circle_deform_session = None;
        self.spiral_session = None;
        self.hyperbola_session = None;
        self.parabola_session = None;
        self.conic_session = None;
        self.rectangle_session = None;
        self.subd_primitive_session = None;
        self.subd_axial_session = None;
        self.subd_sphere_session = None;
        self.subd_torus_session = None;
        self.subd_ellipsoid_session = None;
        self.twist_session = None;
        self.finish_bend_session(false);
        self.finish_stretch_session(false);
        self.finish_taper_session(false);
        self.set_view_prompt = None;
        if std::mem::take(&mut self.remember_copy_prompt) && announce {
            self.push_log("Cancelled RememberCopyOptions".into());
        }
        self.cancel_end_analysis_pick(false);
        self.snaps.model_override = None;
        self.point_filter = None;
        self.point_constraint = None;
        self.finish_points_session();
        self.finish_evaluate_uv_session();
        self.cancel_object_prompt(announce);
        self.cancel_group_prompt(announce);
        self.cancel_intersection_prompt(announce);
        self.finish_edge_command(announce);
        self.finish_hole_command(announce);
        if self.picking_extract_faces() {
            self.component_selection.clear();
        }
        let command = self.active_command.take();
        if matches!(
            command,
            Some(InteractiveCommand::Align {
                postselected: true,
                ..
            })
        ) || matches!(command, Some(InteractiveCommand::ScaleByPlane(prompt))
            if prompt.options.plane == viboceros_command::scale_by_plane::PlaneChoice::Object)
        {
            self.document.clear_selection();
        }
        self.drafting_plane = None;
        if command.is_some() {
            self.command_input.clear();
        }
        self.curve_points.clear();
        if let Some(command) = command
            && announce
            && command != InteractiveCommand::Points
            && !matches!(
                command,
                InteractiveCommand::ProjectDirection { .. }
                    | InteractiveCommand::ProjectPlane { .. }
            )
        {
            let action =
                if transform_applied || matches!(command, InteractiveCommand::EvaluateUv { .. }) {
                    "Finished"
                } else {
                    "Cancelled"
                };
            self.push_log(format!("{action} {}", command.name()));
        }
    }
}
