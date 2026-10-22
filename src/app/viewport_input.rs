//! Viewport actions dispatched to drafting and selection handlers.
use super::*;

impl VibocerosApp {
    pub(super) fn handle_viewport_action(&mut self, mut output: ViewportOutput) -> bool {
        if self.remap_cplane_session.is_some()
            && self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
        {
            if output.source_viewport_click {
                self.accept_remap_cplane_view(self.active_viewport);
            }
            return true;
        }
        if output.selection_pointer.is_some() {
            self.capture_flow_pointer(output.selection_pointer);
            if let Some(s) = &mut self.orient_edge_session {
                s.pointer = output.selection_pointer;
            }
            if let Some(s) = &mut self.curve_array_session {
                s.pointer = output.selection_pointer;
            }
            if let Some(s) = &mut self.array_curve_surface_session {
                s.pointer = output.selection_pointer;
            }
            if let Some(s) = &mut self.arc_extension_session {
                s.pointer = output.selection_pointer;
            }
            if let Some(s) = &mut self.tangent_session {
                s.pointer = output.selection_pointer;
            }
        }
        if let Some(id) = output.object_double_click.take()
            && self.try_open_block_edit_double_click(id)
        {
            return true;
        }
        if let Some(action) = output.surface_corner_click.take()
            && self.edit_tween_corner(action)
        {
            return true;
        }
        if output.source_viewport_click && !output.toggle_maximized && self.cycle_boolean_two() {
            return true;
        }
        if let Some(point) = output.drafting_hover.take() {
            if let Some(s) = self.arc_extension_session.as_mut() {
                let _ = s.input.observe_cursor(point);
            }
            if let Some(s) = &mut self.arc_tilted_session {
                let _ = s.input.observe_cursor(point);
            }
            if let Some(s) = &mut self.ellipse_session {
                s.cursor = Some(point);
            }
            if let Some(s) = &mut self.spiral_session {
                s.cursor = Some(point);
            }
            self.update_subd_sphere_hover(point);
            self.update_subd_torus_hover(point);
            self.update_subd_ellipsoid_hover(point);
            self.update_subd_axial_hover(point);
            self.update_subd_primitive_hover(point);
            self.update_face_create_hover(point);
            self.update_mesh_polyline_hover(point);
            self.update_surface_interpolation_hover(point);
            self.update_short_path_hover(point);
            self.update_surface_offset_hover(point);
            self.update_subcurve_hover(point);
            self.update_soft_edit_hover(point);
        }
        if self.picking_scale_by_plane_view() {
            if output.source_viewport_click {
                self.accept_scale_by_plane_view(self.active_viewport);
            }
            return true;
        }
        if self.select_uv_viewport_hit(&output) {
            return true;
        }
        if let Some(selection) = output.control_point_selection.take() {
            let fit_prompt = self.object_prompt.as_ref().is_some_and(|prompt| {
                matches!(
                    prompt.description.command,
                    "Circle FitPoints"
                        | "SubDCylinder FitPoints"
                        | "SubDCone FitPoints"
                        | "SubDTruncatedCone FitPoints"
                        | "SubDTorus FitPoints"
                        | "SubDSphere FitPoints"
                        | "PlaneThroughPt"
                        | "LineThroughPt"
                        | "Sphere FitPoints"
                        | "Smooth"
                        | "HidePt"
                        | "Weight"
                ) || (prompt.description.workflow
                    == viboceros_command::ObjectSelectionWorkflow::PointInputAfterSelection
                    && transform_sources::allows_grips(prompt.description.command))
            });
            let mode = if fit_prompt && selection.mode == SelectionMode::Replace {
                SelectionMode::Add
            } else {
                selection.mode
            };
            // Validate grip identities before clearing or selecting either set.
            if let Some(pick) = selection
                .picks
                .iter()
                .find(|pick| !self.document.is_control_point_selectable(**pick))
            {
                self.push_log(format!(
                    "Error: control point {} on object {} is unavailable",
                    pick.index, pick.object
                ));
                return true;
            }
            if output.selection_window.as_ref().is_some_and(|window| {
                window
                    .object_ids
                    .iter()
                    .any(|&id| !self.document.is_object_selectable(id))
            }) {
                self.push_log("Error: window selection source is unavailable".into());
                return true;
            }
            if mode == SelectionMode::Replace {
                self.document.clear_selection();
            }
            if let Some(mut window) = output.selection_window.take() {
                window.mode = if mode == SelectionMode::Replace {
                    SelectionMode::Add
                } else {
                    mode
                };
                self.apply_selection_window(window);
            }
            match self.document.select_control_points(
                selection.picks,
                if mode == SelectionMode::Replace {
                    SelectionMode::Add
                } else {
                    mode
                },
            ) {
                Ok(count) => self.push_log(format!("{count} control point(s) selected")),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        if self.refinement_viewport_hit(&output) {
            return true;
        }
        if self.reparameterize_viewport_hit(&output) {
            return true;
        }
        if output.source_viewport_click && self.copy_cplane_source.is_some() {
            self.accept_copy_cplane_source(self.active_viewport);
        } else if output.zoom_target_cancelled {
            self.zoom_target = None;
            self.push_log("Zoom Target canceled".into());
        } else if let Some((target, viewport)) = output.zoom_target_pick {
            self.accept_zoom_target_pick(target, viewport);
        } else if let Some(result) = output.zoom_target_result {
            self.finish_zoom_target(result);
        } else if output.zoom_window_cancelled {
            self.zoom_window_pending = false;
            self.push_log("Zoom window canceled".into());
        } else if let Some(result) = output.zoom_window_result {
            self.zoom_window_pending = false;
            self.push_log(match result {
                Ok(true) => "Zoomed to window".into(),
                Ok(false) => "Zoom window left view unchanged".into(),
                Err(error) => format!("Error: {error}"),
            });
        } else if let Some((center, viewport)) = output.circular_center_pick {
            if let Some(CircularSelectionState::PickCenter(mode)) = self.circular_selection {
                self.circular_selection = Some(CircularSelectionState::PickRadius {
                    mode,
                    center,
                    viewport,
                });
                self.push_log("Select a radius point in the same viewport; Esc to cancel".into());
            }
        } else if let Some((path, viewport, mode)) = output.lasso_stroke {
            self.accept_lasso_stroke(path, viewport, mode);
        } else if let Some((point, viewport, mode)) = output.lasso_point {
            self.accept_lasso_point(point, viewport, mode);
        } else if output.enter_pressed {
            if self.lasso_selection.is_some() {
                self.finish_lasso_selection();
                return true;
            }
            if self.fence_selection.is_some() && self.end_analysis_pick.is_none() {
                self.finish_fence_selection();
            } else {
                self.run_command();
            }
        } else if let Some(click) = output.component_click {
            self.accept_component_click(click);
        } else if let Some(window) = output.component_window {
            self.accept_component_window(window);
        } else if let Some(picks) = output.mesh_boundary_edge_click {
            if let Some(index) = picks.first() {
                self.pick_face_edge(*index);
            }
            return true;
        } else if let Some(picks) = output.edge_click {
            if self.pick_face_edge_click(&picks) {
                return true;
            }
            if self.picking_subcurve_edge()
                || (self.picking_orient_curve()
                    || self.picking_orient_edge()
                    || self.picking_surface_offset_curve())
                || self.picking_array_curve()
                || self.picking_curve_array()
                || self.picking_flow_guide()
                || self.picking_spiral_rail()
                || self.picking_conic_curve()
                || self.picking_rectangle_curve()
                || self.picking_ellipse_curve()
                || self.picking_arc_extension_curve()
                || self.picking_tangent_curve()
                || self.picking_circle_around_curve()
                || self.picking_splop_reference_curve()
            {
                self.accept_component_click(crate::viewport::ComponentClick {
                    picks: picks
                        .into_iter()
                        .map(|pick| crate::viewport::ComponentPick {
                            object: pick.object,
                            index: pick.edge,
                            kind: viboceros_command::ComponentSelectionKind::BrepEdge,
                        })
                        .collect(),
                    preselection: false,
                    modifiers: egui::Modifiers::NONE,
                });
            } else if self.blend_surface_prompt.is_some() {
                self.accept_blend_edges(picks);
            } else if self.hole_prompt.is_some() {
                self.accept_hole_edges(picks);
            } else {
                self.accept_edge_click(picks);
            }
        } else if let Some(parameter) = output.edge_parameter {
            if self.accept_surface_offset_parameter(parameter)
                || self.accept_conic_parameter(parameter)
                || self.accept_rectangle_parameter(parameter)
                || self.accept_ellipse_parameter(parameter)
                || self.accept_splop_reference_parameter(parameter)
                || self.accept_circle_around_parameter(parameter)
                || self.accept_orient_curve_parameter(parameter)
                || self.accept_array_curve_surface_parameter(parameter)
                || self.accept_curve_array_parameter(parameter)
            {
            } else if self.move_normal_curve().is_some() {
                self.accept_move_normal_curve_parameter(parameter);
            } else {
                self.accept_split_parameter(parameter);
            }
        } else if let Some((object, face)) = output.face_click {
            self.accept_component_face_hit(object, face, output.face_hit_point);
        } else if let Some(point) = output.picked_point {
            if self.plane_prompt.is_some() {
                self.accept_plane_prompt_point(point);
            } else {
                self.accept_filtered_drafting_snap(point, output.picked_snap);
            }
        } else if let Some(selection) = output.point_cloud_selection {
            self.select_cloud_points(&selection.indices, selection.mode);
        } else if let Some(choice) = output.selection_choice {
            self.selection_menu = Some(SelectionMenu {
                choice,
                highlighted: 0,
            });
        } else if let Some((point, viewport, mode)) = output.fence_point {
            self.add_fence_point(point, viewport, mode);
        } else if let Some(click) = output.selection_click {
            self.apply_selection_click(click);
        } else if let Some(selection) = output.selection_window {
            if self.circular_selection.take().is_some() {
                self.apply_selection_region(selection, true);
            } else {
                self.apply_selection_window(selection);
            }
        } else {
            return false;
        }
        true
    }

    pub(super) fn control_point_picking_available(&self) -> bool {
        self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && (self.component_preselection_available()
                || self.object_prompt.as_ref().is_some_and(|prompt| {
                    (matches!(
                        prompt.description.command,
                        "Circle FitPoints"
                            | "SubDCylinder FitPoints"
                            | "SubDCone FitPoints"
                            | "SubDTruncatedCone FitPoints"
                            | "SubDTorus FitPoints"
                            | "SubDSphere FitPoints"
                            | "PlaneThroughPt"
                            | "LineThroughPt"
                            | "Sphere FitPoints"
                            | "Smooth"
                            | "HidePt"
                            | "Weight"
                    ) || (prompt.description.workflow
                        == viboceros_command::ObjectSelectionWorkflow::PointInputAfterSelection
                        && transform_sources::allows_grips(prompt.description.command)))
                        && prompt.phase == object_selection::ObjectPromptPhase::Selecting
                }))
    }
}
