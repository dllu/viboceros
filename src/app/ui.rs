//! Application frame, panels, keyboard input, and viewport layout.
use super::*;

impl eframe::App for VibocerosApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        preferences::save_zoom_scale(storage, self.zoom_scale);
        preferences::save_zoom_extents_borders(storage, self.zoom_extents_borders);
        preferences::save_named_view_policy(storage, self.named_view_policy);
        preferences::save_viewport_tabs_visible(storage, self.viewport_tabs_visible);
        preferences::save_viewport_tab_alignment(storage, self.viewport_tab_alignment);
        preferences::save_four_view_projection(storage, self.four_view_projection);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_edge_analysis(ui);
        self.show_curvature_graph(ui);
        self.show_point_deviation(ui);
        self.show_curve_deviation(ui);
        self.validate_orient_guide();
        self.validate_array_curve_surface();
        self.validate_mesh_polyline();
        self.validate_surface_interpolation();
        self.suspend_surface_interpolation_close(ui.input(|i| i.modifiers.alt));
        self.validate_short_path();
        self.validate_surface_offset();
        self.validate_orient_edge();
        self.validate_curve_array();
        self.validate_surface_array();
        self.validate_orient_surface();
        self.validate_flow_guides();
        self.validate_surface_flow();
        self.validate_spiral_rail();
        self.validate_conic_curve();
        self.validate_rectangle_curve();
        self.validate_ellipse_curve();
        self.validate_arc_extension_source();
        self.validate_tangent_sources();
        self.validate_circle_around_curve();
        self.validate_blend_surface();
        self.validate_fillet_surface();
        self.validate_select_uv();
        self.validate_refinement();
        self.validate_reparameterize();
        self.validate_tween_surfaces();
        self.validate_boolean_two();
        self.validate_rebuild_preview();
        self.validate_weight_preview();
        self.validate_soft_crease();
        self.handle_interface_shortcuts(ui);
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.cancel_current_prompt_or_selection();
        }
        if self.selection_menu.is_none()
            && self
                .reparameterize_prompt
                .as_ref()
                .is_none_or(|p| !p.has_menu())
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::Enter))
        {
            self.run_command();
        }
        if self.active_command.is_none()
            && self.selection_menu.is_none()
            && self.object_prompt.is_none()
            && self.boolean_two_prompt.is_none()
            && self.planar_boolean_prompt.is_none()
            && self.tween_surfaces_prompt.is_none()
            && self.blend_surface_prompt.is_none()
            && self.fillet_surface_prompt.is_none()
            && self.refinement_prompt.is_none()
            && self.select_uv_prompt.is_none()
            && self.reparameterize_prompt.is_none()
            && self.group_prompt.is_none()
            && self.intersection_prompt.is_none()
            && self.edge_prompt.is_none()
            && self.hole_prompt.is_none()
            && self.unjoin_prompt.is_none()
            && self.shell_prompt.is_none()
            && self.merge_face_prompt.is_none()
            && self.edge_fillet_prompt.is_none()
            && self.crease_prompt.is_none()
            && self.soft_crease_prompt.is_none()
            && self.plane_prompt.is_none()
            && self.set_view_prompt.is_none()
            && !self.remember_copy_prompt
            && self.edge_zoom_prompt.is_none()
            && (self.document.selected_object_count() > 0
                || self.document.selected_control_points().next().is_some())
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
        {
            self.execute_command("Delete");
        }
        self.capture_global_command_typing(ui);
        self.show_toolbar(ui);
        self.show_grid_settings(ui);
        self.show_weight_dialog(ui.ctx());
        self.show_layers(ui);
        self.show_block_manager(ui.ctx());
        self.show_block_edit(ui.ctx());
        self.show_replace_block_chooser(ui.ctx());
        self.show_command_line(ui);
        let _ = self.show_viewport_tabs(ui);
        let model_input_active = self.set_view_prompt.is_none() && !self.remember_copy_prompt;
        let end_analysis_picking = model_input_active && self.end_analysis_pick.is_some();
        let drafting = DraftingInput {
            active: !end_analysis_picking
                && !self.picking_block_edit_sources()
                && !self.adding_to_block()
                && !self.replacing_block()
                && !self.picking_scale_by_plane_object()
                && !self.picking_scale_by_plane_view()
                && (self.remap_cplane_session.is_none() || self.plane_prompt.is_some())
                && !self.picking_extract_faces()
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
            shift_inverts_ortho: ui.input(|input| input.modifiers.shift),
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
        };
        let mut viewport_outputs = (0..self.viewports.len())
            .map(|_| ViewportOutput::default())
            .collect::<Vec<_>>();
        let active_viewport = self.active_viewport;
        let maximized_viewport = self.maximized_viewport;
        let zoom_window_pending = self.zoom_window_pending && !end_analysis_picking;
        let selection_window_override = (model_input_active && !end_analysis_picking)
            .then_some(self.selection_window_override)
            .flatten();
        let circular_selection = if end_analysis_picking || !model_input_active {
            None
        } else {
            self.circular_selection
        };
        let zoom_target = if end_analysis_picking {
            None
        } else {
            self.zoom_target
        };
        let object_filter = if self.selecting_insert_source() {
            Some(viboceros_command::ObjectSelectionFilter::MeshAndSubD)
        } else if self.insert_prompt().is_some() {
            None
        } else if self.picking_face_append() {
            Some(viboceros_command::ObjectSelectionFilter::FaceAppend)
        } else if self.picking_mesh_polyline() {
            Some(viboceros_command::ObjectSelectionFilter::Mesh)
        } else if end_analysis_picking || self.surface_path_prompt.is_some() {
            Some(viboceros_command::ObjectSelectionFilter::Curves)
        } else if self.picking_surface_offset_points() {
            Some(viboceros_command::ObjectSelectionFilter::Points)
        } else {
            self.viewport_object_filter()
        };
        let selection_preview = self.selection_preview_filter();
        let mut selection_preview_ids = self
            .object_prompt
            .as_ref()
            .and_then(|prompt| prompt.special_selection.as_ref())
            .map(|ids| ids.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        selection_preview_ids.extend(self.block_edit_picked_sources());
        selection_preview_ids.extend(self.surface_offset_selected_points());
        selection_preview_ids.extend(
            self.refinement_prompt
                .as_ref()
                .and_then(refinement::Prompt::object),
        );
        selection_preview_ids.extend(
            self.reparameterize_prompt
                .as_ref()
                .and_then(|p| p.preview_id()),
        );
        if let Some(id) = self
            .selection_menu
            .as_ref()
            .and_then(|menu| menu.choice.object_ids.get(menu.highlighted))
        {
            selection_preview_ids.push(*id);
        }
        let cloud_removal = self
            .object_prompt
            .as_ref()
            .and_then(|prompt| prompt.cloud_removal.as_ref());
        let point_cloud_remove_target = if end_analysis_picking || !model_input_active {
            None
        } else {
            cloud_removal.map(|removal| removal.target)
        };
        let point_cloud_highlights = cloud_removal
            .map(|removal| removal.indices.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let preview_curve = if model_input_active {
            self.curve_draft_preview()
        } else {
            None
        };
        let subcurve_preview = if model_input_active {
            self.subcurve_draft_preview()
        } else {
            None
        };
        let orient_markers = self
            .orient_points_session
            .as_ref()
            .map(orient_points::Session::markers);
        let division_preview = model_input_active
            .then(|| self.divide_preview_points())
            .flatten();
        let subcurve_hover = self.subcurve_preview_hover();
        let mirror_preview = self
            .transform_session
            .as_ref()
            .and_then(|session| session.mirror_preview(self.active_command))
            .filter(|_| model_input_active && self.plane_prompt.is_none());
        let translation_preview = self
            .translation_session
            .as_ref()
            .and_then(|session| session.preview())
            .filter(|_| model_input_active && self.plane_prompt.is_none());
        let affine_preview = self
            .transform_session
            .as_ref()
            .and_then(|session| {
                session.affine_preview(
                    self.active_command,
                    self.drafting_plane.unwrap_or_else(|| {
                        self.viewports[self.active_viewport].construction_plane()
                    }),
                    self.document.tolerance(),
                )
            })
            .or_else(|| {
                self.set_point_sources
                    .as_ref()
                    .and_then(|sources| sources.affine_preview(self.active_command))
            })
            .or_else(|| {
                self.orient_edge_session
                    .as_ref()
                    .and_then(orient_edge::Session::preview)
            })
            .or_else(|| {
                self.orient_curve_session
                    .as_ref()
                    .and_then(orient_curve::Session::preview)
            })
            .or_else(|| {
                self.array_curve_surface_session
                    .as_ref()
                    .and_then(array_curve_surface::Session::preview)
            })
            .or_else(|| {
                self.orient_points_session
                    .as_ref()
                    .and_then(orient_points::Session::preview)
            })
            .or_else(|| {
                self.orient_surface_session
                    .as_ref()
                    .and_then(orient_surface::Session::preview)
            })
            .filter(|_| model_input_active && self.plane_prompt.is_none());
        let instance_preview = self
            .array_surface_session
            .as_ref()
            .and_then(array_surface::Session::preview)
            .or_else(|| {
                self.curve_array_session
                    .as_ref()
                    .and_then(curve_array::Session::preview)
            })
            .filter(|_| model_input_active);
        let plane_object_pick = self
            .plane_prompt
            .as_ref()
            .is_some_and(construction_plane::PlanePrompt::requests_object);
        let plane_surface_pick = self
            .plane_prompt
            .as_ref()
            .is_some_and(construction_plane::PlanePrompt::requests_surface);
        let plane_curve_pick = self
            .plane_prompt
            .as_ref()
            .is_some_and(construction_plane::PlanePrompt::requests_curve);
        let face_pick = if self.merge_face_prompt.is_some() {
            Some(FacePickMode::MeshAndBrepAny)
        } else if self.picking_fillet_surface()
            || self.picking_surface_interpolation_face()
            || self.picking_short_path_face()
            || self.picking_surface_offset_face()
            || self.picking_orient_surface()
            || self.picking_array_surface()
            || self.picking_surface_array()
            || self.picking_flow_surface()
            || self.picking_surface_flow()
            || self.selecting_move_normal_reference()
            || self.picking_uv_reference()
            || self.picking_mirror_object()
            || self.picking_scale_by_plane_object()
        {
            Some(FacePickMode::SurfaceAndBrepAny)
        } else if plane_object_pick && !plane_curve_pick {
            Some(if plane_surface_pick {
                FacePickMode::SurfaceAndBrepAny
            } else {
                FacePickMode::MeshAndBrepAny
            })
        } else if self
            .hole_prompt
            .as_ref()
            .is_some_and(untrim_holes::HolePrompt::picking_faces)
            || self.picking_extract_faces()
            || self.shell_prompt.is_some()
        {
            Some(FacePickMode::SurfaceAndBrepAny)
        } else if matches!(
            self.active_command,
            Some(
                InteractiveCommand::ExtractMeshFaces { .. }
                    | InteractiveCommand::ExtractConnectedMeshFaces { .. }
                    | InteractiveCommand::ExtractMeshPart { .. }
            )
        ) {
            Some(FacePickMode::Mesh)
        } else if self.active_command == Some(InteractiveCommand::DeleteFaces) {
            Some(FacePickMode::MeshAndBrep)
        } else if matches!(
            self.active_command,
            Some(
                InteractiveCommand::ExtractSrf { .. }
                    | InteractiveCommand::DupFaceBorder { .. }
                    | InteractiveCommand::ExtractIsocurve { .. }
                    | InteractiveCommand::DomainFace
                    | InteractiveCommand::SrfSeam { .. }
                    | InteractiveCommand::SplitSurfaceIsocurve { .. }
                    | InteractiveCommand::ExtendSrf { .. }
                    | InteractiveCommand::EvaluateUv { .. }
            )
        ) {
            Some(FacePickMode::SurfaceAndBrep)
        } else {
            None
        }
        .filter(|_| plane_object_pick || model_input_active && self.plane_prompt.is_none());
        let edge_pick = (self
            .edge_prompt
            .as_ref()
            .is_some_and(edge_commands::EdgePrompt::picking_edge)
            || self
                .hole_prompt
                .as_ref()
                .is_some_and(untrim_holes::HolePrompt::picking_edges)
            || self
                .blend_surface_prompt
                .as_ref()
                .is_some_and(blend_surface::Prompt::selecting)
            || self.picking_face_edge()
            || self.picking_subcurve_edge()
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
            || self.picking_splop_reference_curve())
            && self.plane_prompt.is_none()
            && model_input_active
            && !end_analysis_picking;
        let split_selection = self
            .edge_prompt
            .as_ref()
            .and_then(edge_commands::EdgePrompt::split_selection)
            .filter(|_| model_input_active && self.plane_prompt.is_none() && !end_analysis_picking);
        let normal_curve = self
            .translation_session
            .as_ref()
            .and_then(|session| session.normal.as_ref())
            .filter(|normal| {
                normal.direction.is_none()
                    && self.point_filter.is_none()
                    && self.point_constraint.is_none()
            })
            .and_then(|normal| normal.curve.as_ref());
        let edge_curve = split_selection
            .map(viboceros_command::SplitEdgeSelection::curve)
            .or(normal_curve)
            .or_else(|| {
                self.refinement_prompt
                    .as_ref()
                    .and_then(refinement::Prompt::cursor_curve)
            })
            .or_else(|| {
                self.conic_session
                    .as_ref()
                    .and_then(conic::Session::cursor_curve)
            })
            .or_else(|| {
                self.ellipse_session
                    .as_ref()
                    .and_then(ellipse::Session::cursor_curve)
            })
            .or_else(|| {
                self.circle_around_session
                    .as_ref()
                    .and_then(circle_around::Session::cursor_curve)
            })
            .or_else(|| {
                self.rectangle_session
                    .as_ref()
                    .or_else(|| {
                        self.subd_primitive_session
                            .as_ref()
                            .filter(|s| s.base.is_none())
                            .map(|s| &s.rectangle)
                    })
                    .or_else(|| self.surface_flow_session.as_ref()?.plane.as_ref())
                    .and_then(rectangle::InputSession::cursor_curve)
            })
            .or_else(|| {
                self.orient_edge_session
                    .as_ref()
                    .filter(|_| self.point_filter.is_none() && self.point_constraint.is_none())
                    .and_then(orient_edge::Session::cursor_curve)
            })
            .or_else(|| {
                self.surface_offset_session
                    .as_ref()
                    .filter(|_| self.point_filter.is_none() && self.point_constraint.is_none())
                    .and_then(surface_curve_offset::Session::cursor_curve)
            })
            .or_else(|| {
                self.orient_curve_session
                    .as_ref()
                    .filter(|_| self.point_filter.is_none() && self.point_constraint.is_none())
                    .and_then(orient_curve::Session::cursor_curve)
            })
            .or_else(|| {
                self.array_curve_surface_session
                    .as_ref()
                    .filter(|_| self.point_filter.is_none() && self.point_constraint.is_none())
                    .and_then(array_curve_surface::Session::cursor_curve)
            })
            .or_else(|| {
                self.curve_array_session
                    .as_ref()
                    .filter(|_| self.point_filter.is_none() && self.point_constraint.is_none())
                    .and_then(curve_array::Session::cursor_curve)
            });
        let edge_curve = edge_curve.or_else(|| {
            self.orient_surface_session
                .as_ref()
                .and_then(orient_surface::Session::sphere_cursor_curve)
        });
        let edge_parameters =
            split_selection.map_or(&[][..], viboceros_command::SplitEdgeSelection::parameters);
        let edge_distance_parameters =
            split_selection.and_then(viboceros_command::SplitEdgeSelection::distance_parameters);
        let edge_endpoints = match &self.edge_prompt {
            Some(edge_commands::EdgePrompt::Choice(selection)) if edge_pick => {
                Some(selection.endpoints())
            }
            _ => None,
        };
        let edge_analysis_view = self
            .commands
            .edge_analysis_view(&self.document)
            .ok()
            .flatten();
        let curvature_graph_view = self
            .commands
            .curvature_graph_view(&self.document)
            .ok()
            .flatten();
        let deviation_view = self
            .commands
            .point_deviation_view(&self.document)
            .ok()
            .flatten();
        let curve_deviation_view = self.commands.curve_deviation_view(&self.document);
        let edge_zoom_view = self.commands.edge_zoom_view(&self.document).ok().flatten();
        let mut edge_highlights = self
            .edge_prompt
            .as_ref()
            .map_or_else(Vec::new, edge_commands::EdgePrompt::highlights);
        if let Some(prompt) = &self.hole_prompt {
            edge_highlights.extend(prompt.highlights());
        }
        if let Some(prompt) = &self.blend_surface_prompt {
            edge_highlights.extend(prompt.highlights());
        }
        if self.unjoin_prompt.is_none()
            && self.shell_prompt.is_none()
            && self.merge_face_prompt.is_none()
            && self.edge_fillet_prompt.is_none()
            && self.crease_prompt.is_none()
            && self.soft_crease_prompt.is_none()
            && self.shrink_prompt_mode().is_none()
            && !self.to_subd_pending()
            && !self.picking_subdivide()
            && !self.picking_extract_faces()
        {
            self.component_selection.valid_picks(&self.document);
        }
        let mut component_highlights = self.component_selection.highlights(&self.document);
        if let Some(p) = &self.fillet_surface_prompt {
            component_highlights.extend(p.highlights());
        }
        component_highlights.extend(
            self.merge_face_highlight()
                .filter(|p| p.kind != viboceros_command::ComponentSelectionKind::MeshFace),
        );
        let parametric_pick = model_input_active
            && (self.picking_reparameterize()
                || self
                    .refinement_prompt
                    .as_ref()
                    .is_some_and(refinement::Prompt::selecting)
                || self
                    .select_uv_prompt
                    .as_ref()
                    .is_some_and(|p| p.selecting()));
        let parametric_highlights = self
            .reparameterize_prompt
            .as_ref()
            .map(|p| p.highlights())
            .unwrap_or_default();
        let component_highlight_segments = self.merge_face_mesh_wires();
        let component_preselection = self.component_preselection_available();
        let control_point_pick = model_input_active && self.control_point_picking_available();
        let component_pick = self
            .hole_prompt
            .as_ref()
            .and_then(|prompt| {
                if !model_input_active || self.plane_prompt.is_some() || end_analysis_picking {
                    None
                } else if prompt.picking_edges() {
                    Some(crate::viewport::ComponentPickFilter::Edges)
                } else if prompt.picking_faces() {
                    Some(crate::viewport::ComponentPickFilter::Faces)
                } else {
                    None
                }
            })
            .or_else(|| {
                ((self.unjoin_prompt.is_some() || self.edge_fillet_prompt.is_some())
                    && model_input_active
                    && self.plane_prompt.is_none()
                    && !end_analysis_picking)
                    .then_some(crate::viewport::ComponentPickFilter::Edges)
            });
        let component_pick = component_pick.or_else(|| {
            (((self.crease_prompt.is_some() && self.insert_prompt().is_none())
                || self.picking_soft_crease_edges())
                && model_input_active
                && self.plane_prompt.is_none()
                && !end_analysis_picking)
                .then_some(crate::viewport::ComponentPickFilter::Crease)
        });
        let component_pick = component_pick.or_else(|| {
            ((self.picking_extract_faces() || self.shell_prompt.is_some())
                && model_input_active
                && self.plane_prompt.is_none()
                && !end_analysis_picking)
                .then_some(crate::viewport::ComponentPickFilter::Faces)
        });
        let fence_selection = if end_analysis_picking || !model_input_active {
            None
        } else {
            self.fence_selection.as_ref()
        };
        let lasso_selection = if end_analysis_picking || !model_input_active {
            None
        } else {
            self.lasso_selection.as_ref()
        };
        let fence_curve_pick = fence_selection.is_some_and(|state| state.curve_pick);
        let curve_region_pick = model_input_active
            && (fence_curve_pick
                || self.boundary_selection.is_some()
                || matches!(
                    self.active_command,
                    Some(
                        InteractiveCommand::SelVolumePipe { source: None, .. }
                            | InteractiveCommand::Pipe { source: None, .. }
                    )
                ));
        let volume_object_pick = model_input_active
            && matches!(
                self.active_command,
                Some(InteractiveCommand::SelVolumeObject { .. })
            );
        let end_markers = self
            .end_analysis
            .as_ref()
            .and_then(|analysis| {
                collect_end_markers(
                    &self.document,
                    analysis.sources.iter().copied(),
                    analysis.options,
                )
                .ok()
            })
            .unwrap_or_default();
        let current_end_marker = self.end_analysis.as_ref().and_then(|analysis| {
            (!end_markers.is_empty()).then_some(analysis.current % end_markers.len())
        });
        let end_marker_color = self.end_analysis.as_ref().and_then(|analysis| {
            analysis
                .use_single_marker_color
                .then_some(analysis.marker_color)
        });
        let tween_corner_controls = self.tween_corner_controls().to_vec();
        let normal_surface = model_input_active
            .then(|| {
                self.orient_surface_location()
                    .or_else(|| {
                        self.surface_interpolation_session
                            .as_ref()?
                            .cursor_surface()
                    })
                    .or_else(|| self.short_path_session.as_ref()?.cursor_surface())
                    .or_else(|| self.surface_offset_session.as_ref()?.cursor_surface())
                    .or_else(|| self.select_uv_prompt.as_ref().and_then(|p| p.surface()))
                    .or_else(|| self.move_normal_surface())
                    .or_else(|| {
                        self.refinement_prompt
                            .as_ref()
                            .and_then(refinement::Prompt::surface)
                    })
            })
            .flatten();
        let selecting_normal = model_input_active && self.selecting_move_normal_reference();
        let curve_or_edge_pick = edge_pick
            && (self.picking_subcurve_edge()
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
                || self.picking_splop_reference_curve());
        let document = surface_rebuild::viewport_document(
            &self.document,
            self.boolean_two_prompt.as_ref().and_then(|p| p.scene()),
            self.blend_surface_prompt
                .as_ref()
                .and_then(|p| p.scene())
                .or_else(|| self.tween_surfaces_prompt.as_ref().and_then(|p| p.scene()))
                .or_else(|| self.fillet_surface_prompt.as_ref().and_then(|p| p.scene())),
            self.rebuild_preview
                .as_ref()
                .and_then(|p| p.scene())
                .or_else(|| self.refinement_prompt.as_ref().and_then(|p| p.scene()))
                .or_else(|| self.weight_prompt.as_ref().and_then(|p| p.scene()))
                .or_else(|| self.soft_crease_prompt.as_ref().and_then(|p| p.scene()))
                .or_else(|| self.crease_prompt.as_ref().and_then(|p| p.scene())),
        );
        let twist_preview = self
            .twist_session
            .as_ref()
            .and_then(|s| s.preview(self.active_command));
        let bend_preview = self.bend_session.as_ref().and_then(|s| {
            s.preview(
                self.active_command,
                self.commands.transform_scalar_default("Bend"),
            )
        });
        let stretch_preview = self
            .stretch_session
            .as_ref()
            .and_then(|s| s.preview(self.active_command));
        let taper_preview = self.taper_session.as_ref().and_then(|s| {
            s.preview(
                self.active_command,
                self.viewports[self.active_viewport].construction_plane(),
            )
        });
        let maelstrom_preview = self.maelstrom_session.as_ref().and_then(|s| {
            s.preview(
                self.active_command,
                self.viewports[self.active_viewport].construction_plane(),
            )
        });
        let surface_session = self
            .orient_surface_session
            .as_ref()
            .filter(|_| model_input_active);
        let surface_normals = self
            .viewports
            .iter()
            .map(|v| v.construction_plane().z_axis().as_vector())
            .collect::<Vec<_>>();
        let angle_plane = matches!(
            self.active_command,
            Some(
                InteractiveCommand::Twist {
                    points: [Some(_), Some(_), _],
                    ..
                } | InteractiveCommand::Maelstrom {
                    initial: Some(_),
                    ..
                }
            )
        )
        .then_some(self.drafting_plane)
        .flatten()
        .or_else(|| {
            if let Some(InteractiveCommand::ScaleByPlane(prompt)) = self.active_command {
                prompt
                    .plane
                    .zip(prompt.origin)
                    .map(|(plane, origin)| plane.with_origin(origin))
            } else {
                None
            }
        })
        .or_else(|| {
            maelstrom_preview
                .and_then(|p| p.circle_getter)
                .and_then(viboceros_command::circle_input::CircleInput::mouse_plane)
        });
        let angle_plane = angle_plane
            .or_else(|| {
                self.subd_ellipsoid_session
                    .as_ref()
                    .and(self.drafting_plane)
            })
            .or_else(|| self.subd_torus_session.as_ref().and(self.drafting_plane))
            .or_else(|| self.subd_sphere_session.as_ref().and(self.drafting_plane))
            .or_else(|| self.subd_axial_session.as_ref().and(self.drafting_plane))
            .or_else(|| {
                self.circle_around_session
                    .as_ref()?
                    .input
                    .map(viboceros_command::circle_around_input::Input::frame)
            })
            .or_else(|| surface_session.and_then(orient_surface::Session::angle_plane))
            .or_else(|| self.rectangle_getter().and_then(|s| s.input.mouse_plane()))
            .or_else(|| {
                self.conic_session
                    .as_ref()
                    .and_then(|s| s.input.mouse_plane())
            });
        let translation_constraint = model_input_active
            .then(|| {
                self.translation_constraint()
                    .or_else(|| {
                        self.refinement_prompt
                            .as_ref()
                            .and_then(refinement::Prompt::destination_constraint)
                    })
                    .or_else(|| self.conic_session.as_ref()?.input.point_constraint())
            })
            .flatten();
        let curve_points = self
            .plane_prompt
            .as_ref()
            .map_or(self.curve_points.as_slice(), |prompt| {
                prompt.points.as_slice()
            });
        let point_filter = self
            .plane_prompt
            .is_none()
            .then_some(self.point_filter.filter(|_| model_input_active))
            .flatten();
        let point_constraint = self
            .plane_prompt
            .is_none()
            .then_some(self.point_constraint.filter(|_| model_input_active))
            .flatten();
        let selection_excluded_object = self
            .planar_boolean_prompt
            .as_ref()
            .and_then(|p| p.first)
            .or_else(|| {
                self.fillet_surface_prompt
                    .as_ref()
                    .and_then(|p| p.excluded())
            });
        let selection_allowed_objects = self
            .object_prompt
            .as_ref()
            .and_then(|p| self.curvature_graph_selection_ids(p.description.command))
            .or_else(|| {
                self.reparameterize_prompt
                    .as_ref()
                    .filter(|p| p.selecting())
                    .and_then(|p| p.source_id())
                    .map(|id| [id].into_iter().collect())
            })
            .or_else(|| {
                self.refinement_prompt
                    .as_ref()
                    .filter(|p| p.selecting())
                    .map(|p| {
                        self.document
                            .selectable_objects()
                            .filter(|o| p.accepts(o.geometry()))
                            .map(|o| o.id())
                            .collect()
                    })
            });
        let selection_allowed_objects = if self
            .select_uv_prompt
            .as_ref()
            .is_some_and(|p| p.selecting())
        {
            Some(
                self.document
                    .selectable_objects()
                    .filter(|o| viboceros_command::select_uv_rows::accepts(o.geometry()))
                    .map(|o| o.id())
                    .collect(),
            )
        } else {
            selection_allowed_objects
        };
        let viewport_positions = &self.viewport_positions;
        let catenary_input = self
            .catenary_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let spiral_input = self
            .spiral_session
            .as_ref()
            .map(|s| s.input.clone())
            .filter(|_| model_input_active);
        let hyperbola_input = self
            .hyperbola_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let circle_deform_input = self
            .circle_deform_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let tangent_input = self
            .tangent_session
            .as_ref()
            .filter(|_| model_input_active)
            .map(|s| crate::viewport::TangentPreview {
                sphere: s.sphere(),
                contacts: &s.input,
                output: s.output,
                arc: s.support.as_ref(),
                arc_output: self.arc_deform_session.map(|s| s.output()),
            });
        let circle_around_output = self
            .circle_around_session
            .as_ref()
            .and_then(|s| s.output)
            .filter(|_| model_input_active);
        let arc_output = self
            .arc_deform_session
            .map(|s| s.output())
            .filter(|_| model_input_active);
        let arc_preview = self
            .arc_preview()
            .map(|p| {
                (
                    p,
                    self.drafting_plane.unwrap_or_else(|| {
                        self.viewports[self.active_viewport].construction_plane()
                    }),
                )
            })
            .filter(|_| model_input_active);
        let arc_extension_input = self
            .arc_extension_session
            .as_ref()
            .map(|s| &s.input)
            .filter(|_| model_input_active);
        let arc_tilted_input = self
            .arc_tilted_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let circle_around_input = self
            .circle_around_session
            .as_ref()
            .and_then(|s| s.input)
            .filter(|_| model_input_active);
        let ellipse_input = self
            .ellipse_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let parabola_input = self
            .parabola_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let conic_input = self
            .conic_session
            .as_ref()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let conic_reference = self
            .conic_session
            .as_ref()
            .and_then(|s| s.reference.as_ref())
            .filter(|_| model_input_active);
        let rectangle_input = self
            .rectangle_getter()
            .map(|s| s.input)
            .filter(|_| model_input_active);
        let mesh_boundary_edges = self
            .face_create_session
            .as_ref()
            .filter(|s| edge_pick && s.boundary_pick())
            .map(|s| s.boundary_segments());
        let insert_edges = match self.crease_prompt.as_ref() {
            Some(crease::Prompt::Insert(p)) => p.session.as_ref().map(|s| s.source_edges()),
            _ => None,
        };
        let insert_mesh = match self.crease_prompt.as_ref() {
            Some(crease::Prompt::Insert(p)) => {
                p.session.as_ref().and_then(|s| s.source_pick_mesh())
            }
            _ => None,
        };
        let viewports = &mut self.viewports;
        egui::CentralPanel::default().show(ui, |ui| {
            let available = ui.available_rect_before_wrap();
            let indices = (0..viewports.len())
                .filter(|index| *index != active_viewport)
                .chain(std::iter::once(active_viewport));
            for index in indices {
                if maximized_viewport.is_none() || maximized_viewport == Some(index) {
                    let position = if maximized_viewport.is_some() {
                        [0.0, 1.0, 0.0, 1.0]
                    } else {
                        viewport_positions[index]
                    };
                    let rect = viewport_rect(available, position);
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(index)
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    child.set_clip_rect(rect);
                    viewport_outputs[index] = viewports[index].show(
                        &mut child,
                        document,
                        ViewportInput {
                            insert_edges,
                            insert_mesh,
                            soft_crease: self.soft_crease_prompt.as_ref().and_then(|p| p.overlay()),
                            drafting,
                            angle_plane,
                            point_filter,
                            point_constraint,
                            translation_constraint,
                            normal_surface,
                            mesh_constraint: self
                                .mesh_polyline_session
                                .as_ref()
                                .and_then(polyline_on_mesh::Session::index),
                            surface_curve_close_point: self
                                .surface_interpolation_session
                                .as_ref()
                                .and_then(surface_interpolation::Session::close_point)
                                .or_else(|| self.mesh_polyline_session.as_ref()?.close_point()),
                            zoom_window: zoom_window_pending,
                            rect_selection_mode: selection_window_override,
                            circular_selection: match circular_selection {
                                Some(CircularSelectionState::PickCenter(_)) => {
                                    Some(CircularSelectionInput::PickCenter)
                                }
                                Some(CircularSelectionState::PickRadius {
                                    mode,
                                    center,
                                    viewport,
                                }) if viewport == index => {
                                    Some(CircularSelectionInput::PickRadius { mode, center })
                                }
                                Some(CircularSelectionState::PickRadius { .. }) => {
                                    Some(CircularSelectionInput::Waiting)
                                }
                                None => None,
                            },
                            fence_selection: match fence_selection {
                                Some(state) if state.curve_pick => None,
                                Some(state) if state.viewport.is_none() => {
                                    Some(FenceSelectionInput::PickFirst)
                                }
                                Some(state) if state.viewport == Some(index) => {
                                    Some(FenceSelectionInput::Continue(&state.points))
                                }
                                Some(_) => Some(FenceSelectionInput::Waiting),
                                None => None,
                            },
                            lasso_selection: match lasso_selection {
                                Some(state)
                                    if state.viewport.is_none()
                                        || state.viewport == Some(index) =>
                                {
                                    Some(LassoSelectionInput::Capture {
                                        points: &state.points,
                                        mode: state.mode,
                                    })
                                }
                                Some(_) => Some(LassoSelectionInput::Waiting),
                                None => None,
                            },
                            zoom_target: match zoom_target {
                                Some(ZoomTargetState::PickTarget) => {
                                    Some(ZoomTargetInput::PickTarget)
                                }
                                Some(ZoomTargetState::PickWindow { target, viewport })
                                    if viewport == index =>
                                {
                                    Some(ZoomTargetInput::PickWindow(target))
                                }
                                Some(ZoomTargetState::PickWindow { .. }) => {
                                    Some(ZoomTargetInput::Waiting)
                                }
                                None => None,
                            },
                            object_filter: if selecting_normal {
                                Some(viboceros_command::ObjectSelectionFilter::Parametric)
                            } else if plane_object_pick {
                                Some(viboceros_command::ObjectSelectionFilter::Any)
                            } else if end_analysis_picking || curve_region_pick {
                                Some(viboceros_command::ObjectSelectionFilter::Curves)
                            } else if volume_object_pick {
                                Some(viboceros_command::ObjectSelectionFilter::Any)
                            } else {
                                object_filter
                            },
                            selection_excluded_object,
                            selection_allowed_objects: selection_allowed_objects.as_ref(),
                            selection_preview: if self.surface_path_prompt.is_some()
                                || plane_object_pick
                                || end_analysis_picking
                                || curve_region_pick
                                || volume_object_pick
                            {
                                None
                            } else {
                                selection_preview
                            },
                            selection_preview_ids: &selection_preview_ids,
                            point_cloud_remove_target,
                            point_cloud_highlights: &point_cloud_highlights,
                            isocurve_guides: self
                                .refinement_prompt
                                .as_ref()
                                .map_or(&[], refinement::Prompt::guides),
                            select_uv_rows: self.select_uv_prompt.as_ref().and_then(|p| p.picker()),
                            control_removal: self
                                .refinement_prompt
                                .as_ref()
                                .and_then(refinement::Prompt::control_removal),
                            preview_curve: preview_curve.as_deref().or_else(|| {
                                subcurve_preview
                                    .as_ref()
                                    .filter(|p| p.show_curve)
                                    .map(|p| &p.geometry.curve)
                            }),
                            preview_curves: self
                                .face_create_session
                                .as_ref()
                                .map(face_create::Session::preview_curves)
                                .or_else(|| {
                                    self.subd_ellipsoid_session
                                        .as_ref()
                                        .map(subd_ellipsoid::Session::preview_curves)
                                })
                                .or_else(|| {
                                    self.subd_torus_session
                                        .as_ref()
                                        .map(subd_torus::Session::preview_curves)
                                })
                                .or_else(|| {
                                    self.subd_sphere_session
                                        .as_ref()
                                        .map(subd_sphere::Session::preview_curves)
                                })
                                .or_else(|| {
                                    self.subd_axial_session
                                        .as_ref()
                                        .map(subd_axial::Session::preview_curves)
                                })
                                .or_else(|| {
                                    self.subd_primitive_session
                                        .as_ref()
                                        .map(subd_primitives::Session::preview_curves)
                                })
                                .or_else(|| {
                                    self.surface_offset_session
                                        .as_ref()
                                        .map(surface_curve_offset::Session::preview_curves)
                                        .or_else(|| {
                                            self.short_path_session
                                                .as_ref()
                                                .map(short_path::Session::preview_curves)
                                        })
                                        .or_else(|| {
                                            self.surface_interpolation_session
                                                .as_ref()
                                                .map(surface_interpolation::Session::preview_curves)
                                        })
                                        .or_else(|| {
                                            self.mesh_polyline_session
                                                .as_ref()
                                                .map(polyline_on_mesh::Session::preview_curves)
                                        })
                                })
                                .unwrap_or_default(),
                            preview_points: orient_markers
                                .as_deref()
                                .or_else(|| {
                                    self.surface_interpolation_session
                                        .as_ref()
                                        .map(surface_interpolation::Session::points)
                                })
                                .or_else(|| {
                                    self.mesh_polyline_session
                                        .as_ref()
                                        .map(polyline_on_mesh::Session::points)
                                })
                                .or(division_preview.as_deref())
                                .or_else(|| {
                                    self.refinement_prompt
                                        .as_ref()
                                        .and_then(refinement::Prompt::markers)
                                })
                                .unwrap_or_else(|| {
                                    subcurve_preview
                                        .as_ref()
                                        .map_or(&[], |p| &p.geometry.endpoints)
                                }),
                            mirror_preview,
                            translation_preview,
                            affine_preview,
                            instance_preview,
                            catenary_input,
                            spiral_input: spiral_input.as_ref(),
                            hyperbola_input,
                            parabola_input,
                            ellipse_input,
                            tangent: tangent_input,
                            arc_tilted_input,
                            arc_extension_input,
                            arc_output,
                            arc_preview,
                            circle_around_input,
                            circle_around_output,
                            circle_deform_input,
                            conic_input,
                            conic_reference,
                            rectangle_input,
                            surface_flow_preview: self
                                .surface_flow_session
                                .as_ref()
                                .and_then(|s| s.preview())
                                .filter(|_| model_input_active),
                            flow_preview: self
                                .flow_session
                                .as_ref()
                                .and_then(flow::Session::preview)
                                .filter(|_| model_input_active),
                            flow_local_preview: self
                                .flow_session
                                .as_ref()
                                .and_then(flow::Session::local_preview)
                                .filter(|_| model_input_active),
                            twist_preview,
                            bend_preview,
                            stretch_preview,
                            taper_preview,
                            maelstrom_preview,
                            orient_surface_preview: surface_session
                                .and_then(|s| s.deformation(surface_normals[index])),
                            face_pick,
                            parametric_pick,
                            parametric_highlights: &parametric_highlights,
                            component_preselection,
                            control_point_pick,
                            component_pick,
                            component_highlights: &component_highlights,
                            component_highlight_segments: &component_highlight_segments,
                            edge_pick,
                            mesh_boundary_edges,
                            curve_or_edge_pick,
                            point_deviation: deviation_view.as_ref(),
                            curve_deviation: curve_deviation_view.as_ref(),
                            edge_analysis: edge_analysis_view.as_ref(),
                            curvature_graph: curvature_graph_view.as_ref(),
                            edge_zoom: edge_zoom_view.as_ref(),
                            edge_highlights: &edge_highlights,
                            edge_endpoints,
                            edge_curve,
                            edge_parameters,
                            edge_distance_parameters,
                            end_markers: &end_markers,
                            current_end_marker,
                            end_marker_color,
                            surface_corners: &tween_corner_controls,
                        },
                        curve_points,
                        index,
                        index == active_viewport,
                    );
                }
            }
            ui.advance_cursor_after_rect(available);
        });
        let parametric_menu_consumed = self.show_reparameterize_menu(ui, &viewport_outputs);
        let menu_action = self.show_selection_menu(ui);
        let mut menu_consumed = parametric_menu_consumed || menu_action.is_some();
        if let Some(object_id) = menu_action {
            let mode = self.selection_menu.take().unwrap().choice.mode;
            if let Some(object_id) = object_id {
                self.apply_selection_click(SelectionClick {
                    object_id: Some(object_id),
                    mode,
                });
            }
        } else if let Some(menu) = self.selection_menu.as_mut() {
            if ui.input(|input| {
                (!ui.ctx().egui_wants_keyboard_input() && input.key_pressed(egui::Key::Enter))
                    || input.pointer.secondary_clicked()
            }) {
                let click = menu.highlighted_click();
                self.selection_menu = None;
                self.apply_selection_click(click);
                menu_consumed = true;
            } else if ui.input(|input| input.pointer.primary_clicked()) {
                let pointer = ui.input(|input| input.pointer.interact_pos());
                let original = pointer.is_some_and(|pointer| {
                    viewport_outputs.iter().enumerate().any(|(index, output)| {
                        (output.selection_choice.is_some() || output.selection_click.is_some())
                            && menu.is_original_pick(index, pointer)
                    })
                });
                if original {
                    menu.cycle();
                    menu_consumed = true;
                } else {
                    self.selection_menu = None;
                    menu_consumed = !viewport_outputs.iter().any(|output| {
                        output.selection_choice.is_some()
                            || output
                                .selection_click
                                .as_ref()
                                .is_some_and(|click| click.object_id.is_some())
                    });
                }
            }
        }
        let mut handled_action = false;
        for (index, output) in viewport_outputs.into_iter().enumerate() {
            if let Some(preview) = output.mirror_preview
                && self.update_mirror_preview(preview)
            {
                ui.ctx().request_repaint();
            }
            if let Some(preview) = output.translation_preview
                && self.update_translation_preview(preview)
            {
                ui.ctx().request_repaint();
            }
            if let Some(preview) = output.affine_preview
                && self.update_affine_preview(preview)
            {
                ui.ctx().request_repaint();
            }
            if let Some(angle) = output.twist_preview
                && self.update_twist_preview(angle)
            {
                ui.ctx().request_repaint();
            }
            if let Some(point) = output.bend_preview
                && self.update_bend_preview(point)
            {
                ui.ctx().request_repaint();
            }
            if let Some(point) = output.stretch_preview
                && self.update_stretch_preview(point)
            {
                ui.ctx().request_repaint();
            }
            if let Some(point) = output.taper_preview
                && self.update_taper_preview(point)
            {
                ui.ctx().request_repaint();
            }
            if let Some(cursor) = output.maelstrom_preview
                && self.update_maelstrom_preview(cursor)
            {
                ui.ctx().request_repaint();
            }
            if let Some(point) = output.orient_surface_preview
                && self.update_orient_surface_preview(point, surface_normals[index])
            {
                ui.ctx().request_repaint();
            }
            if output.activated {
                self.active_viewport = index;
            }
            if output.toggle_maximized {
                self.active_viewport = index;
                self.maximized_viewport = if self.maximized_viewport == Some(index) {
                    None
                } else {
                    Some(index)
                };
                continue;
            }
            if !handled_action && !menu_consumed {
                handled_action = self.handle_viewport_action(output);
            }
        }
        if subcurve_hover != self.subcurve_preview_hover() {
            ui.ctx().request_repaint();
        }
    }
}
