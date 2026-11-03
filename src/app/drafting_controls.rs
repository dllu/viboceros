//! Shared viewport point-input state for UI rendering and headless interaction checks.
use super::*;

impl VibocerosApp {
    pub(super) fn viewport_drafting_input(
        &self,
        end_analysis_picking: bool,
        shift_inverts_ortho: bool,
    ) -> DraftingInput {
        DraftingInput {
            active: !end_analysis_picking
                && !self.picking_block_edit_sources()
                && !self.adding_to_block()
                && !self.replacing_block()
                && !self.picking_scale_by_plane_object()
                && !self.picking_scale_by_plane_view()
                && (self.remap_cplane_session.is_none() || self.plane_prompt.is_some())
                && !self.picking_extract_faces()
                && self.offset_subd_session.is_none()
                && self.extrude_subd_session.is_none()
                && !self.picking_spiral_rail()
                && !self.picking_conic_curve()
                && !self.picking_rectangle_curve()
                && !self.picking_ellipse_curve()
                && !self.picking_arc_extension_curve()
                && self
                    .tangent_session
                    .as_ref()
                    .is_none_or(tangent::Session::wants_point)
                && !self.picking_circle_around_curve()
                && !self.picking_splop_reference_curve()
                && !(self.picking_orient_curve()
                    || self.picking_orient_edge()
                    || self.picking_surface_offset_curve())
                && !self.picking_array_curve()
                && !self.picking_draft_pull_curve()
                && self
                    .curve_array_session
                    .as_ref()
                    .is_none_or(curve_array::Session::wants_point)
                && (self.surface_flow_session.is_none() || self.drawing_surface_flow())
                && (self.flow_session.is_none()
                    || self.drawing_flow_line()
                    || self.drawing_flow_axis()
                    || self.drawing_flow_radius())
                && !self.picking_face_append()
                && !self.picking_face_edge()
                && !self.picking_mesh_polyline()
                && !self.picking_surface_interpolation_face()
                && !self.picking_short_path_face()
                && !self.picking_surface_offset_face()
                && !self.picking_surface_offset_points()
                && !self.picking_orient_surface()
                && !self.picking_array_surface()
                && self
                    .array_surface_session
                    .as_ref()
                    .is_none_or(array_surface::Session::wants_point)
                && self
                    .array_curve_surface_session
                    .as_ref()
                    .is_none_or(array_curve_surface::Session::wants_point)
                && !self.selecting_move_normal_reference()
                && (self.set_view_prompt.is_none() || self.plane_prompt.is_some())
                && self.plane_prompt.as_ref().map_or_else(
                    || {
                        (self.active_command.is_some()
                            || self
                                .refinement_prompt
                                .as_ref()
                                .is_some_and(refinement::Prompt::wants_point)
                            || self
                                .select_uv_prompt
                                .as_ref()
                                .is_some_and(|p| !p.selecting()))
                            && !self.picking_alignment_curve()
                            && !matches!(
                                self.active_command,
                                Some(
                                    InteractiveCommand::SelVolumePipe { source: None, .. }
                                        | InteractiveCommand::Pipe { source: None, .. }
                                        | InteractiveCommand::SelVolumeObject { .. }
                                        | InteractiveCommand::MirrorObject
                                )
                            )
                    },
                    construction_plane::PlanePrompt::requests_point,
                ),
            osnap: self.effective_snap_modes(),
            mesh_edges: self.snaps.mesh_edges,
            smart_track: self.smart_track,
            grid_snap: self.grid_snap,
            ortho: self.ortho,
            ortho_snap_to_cplane_z: self.ortho_snap_to_cplane_z,
            shift_inverts_ortho,
            planar: self.planar,
            ortho_angle_degrees: self.ortho_angle.degrees(),
            anchor: if let Some(prompt) = &self.plane_prompt {
                prompt.anchor()
            } else if self
                .active_command
                .is_some_and(InteractiveCommand::collects_curve_points)
            {
                self.curve_points.last().copied()
            } else {
                self.arc_length_session
                    .and_then(|s| s.first)
                    .or_else(|| self.maelstrom_circle_anchor())
                    .or_else(|| self.active_command.and_then(InteractiveCommand::anchor))
            },
            reference: if self.plane_prompt.is_some() {
                None
            } else {
                self.active_command.and_then(InteractiveCommand::reference)
            },
        }
    }
}
