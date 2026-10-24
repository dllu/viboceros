//! Object selection clicks, ambiguity menus, and selection regions.
use super::*;

impl VibocerosApp {
    pub(super) fn apply_selection_click(&mut self, click: SelectionClick) {
        if self.selecting_insert_source() {
            if let Some(id) = click.object_id {
                self.select_insert_source(id);
            }
            return;
        }
        if self.pick_mesh_polyline(click.object_id) {
            return;
        }
        if self.pick_surface_interpolation_face(click.object_id, None) {
            return;
        }
        if self.pick_short_path_face(click.object_id, None) {
            return;
        }
        if self.pick_surface_offset_points(click.object_id, click.mode) {
            return;
        }
        if self.pick_surface_offset_curve(click.object_id, None) {
            return;
        }
        if self.pick_select_uv_object(click.object_id) {
            return;
        }
        if self.pick_refinement_object(click.object_id) {
            return;
        }
        if self.pick_reparameterize_object(click.object_id) {
            return;
        }
        if self.remap_cplane_session.is_some() && self.plane_prompt.is_none() {
            return;
        }
        if self.pick_surface_extrusion_path(click.object_id) {
            return;
        }
        if self.blend_surface_prompt.is_some()
            || self.fillet_surface_prompt.is_some()
            || self.refinement_prompt.is_some()
            || self.select_uv_prompt.is_some()
            || self.reparameterize_prompt.is_some()
        {
            return;
        }
        if self.pick_block_edit_sources(click.object_id, click.mode) {
            return;
        }
        if self.pick_block_add(click.object_id, click.mode) {
            return;
        }
        if self.pick_replace_block(click.object_id) {
            return;
        }
        if self.pick_tween_surface(click.object_id) {
            return;
        }
        if self.pick_planar_boolean(click.object_id) {
            return;
        }
        if self
            .boolean_two_prompt
            .as_ref()
            .is_some_and(|p| !p.cycling())
        {
            self.pick_boolean_two_objects(click.object_id, click.mode);
            return;
        }
        if self.pick_subcurve_source(click.object_id) {
            return;
        }
        if self.picking_surface_array() {
            if let Some(id) = click.object_id {
                self.accept_surface_array_target(id, None);
            }
            return;
        }
        if self.picking_array_surface() {
            if let Some(id) = click.object_id {
                self.accept_array_surface(id, None);
            }
            return;
        }
        if self.picking_orient_surface() {
            if let Some(id) = click.object_id {
                self.accept_orient_surface(id, None);
            }
            return;
        }
        if self.pick_spiral_rail(click.object_id, None) {
            return;
        }
        if self.pick_conic_curve(click.object_id, None) {
            return;
        }
        if self.pick_arc_extension_curve(click.object_id, None) {
            return;
        }
        if self.pick_tangent_curve(click.object_id, None) {
            return;
        }
        if self.pick_splop_reference_curve(click.object_id, None) {
            return;
        }
        if self.pick_circle_around_curve(click.object_id, None) {
            return;
        }
        if self.pick_ellipse_curve(click.object_id, None) {
            return;
        }
        if self.pick_rectangle_curve(click.object_id, None) {
            return;
        }
        if self.pick_surface_flow(click.object_id, None, None) {
            return;
        }
        if self.pick_flow_surface(click.object_id, None) {
            return;
        }
        if self.pick_flow_guide(click.object_id, None) {
            return;
        }
        if self.pick_curve_array(click.object_id, None) {
            return;
        }
        if self.pick_array_curve(click.object_id, None) {
            return;
        }
        if self.pick_curve_to_edge(click.object_id, None) {
            return;
        }
        if self.pick_orient_curve(click.object_id) {
            return;
        }
        if self.selecting_move_normal_reference() {
            if let Some(id) = click.object_id {
                self.accept_move_normal_reference(id, None);
            }
            return;
        }
        if self.picking_scale_by_plane_object() {
            if let Some(id) = click.object_id {
                self.accept_scale_by_plane_object(id, None);
            }
            return;
        }
        if self.picking_mirror_object() {
            if let Some(id) = click.object_id {
                self.accept_mirror_object(id, None);
            }
            return;
        }
        if self
            .plane_prompt
            .as_ref()
            .is_some_and(construction_plane::PlanePrompt::requests_object)
        {
            if let Some(id) = click.object_id {
                if self
                    .plane_prompt
                    .as_ref()
                    .is_some_and(construction_plane::PlanePrompt::requests_surface)
                {
                    self.accept_plane_prompt_surface(id, None);
                } else if self
                    .plane_prompt
                    .as_ref()
                    .is_some_and(construction_plane::PlanePrompt::requests_curve)
                {
                    self.accept_plane_prompt_curve(id);
                } else {
                    self.accept_plane_prompt_object(id);
                }
            }
            return;
        }
        if self.end_analysis_pick.is_some() {
            if let Some(id) = click.object_id {
                self.apply_end_analysis_pick_ids(vec![id]);
            }
            return;
        }
        if let Some(InteractiveCommand::Pipe {
            source: None,
            cap_flat,
            cap_round,
            blend_global,
            fit_rail,
            wall_thickness,
            pick_second_radius,
            first_radius,
        }) = self.active_command
        {
            if let Some(source) = click.object_id
                && self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .is_some()
            {
                let command = InteractiveCommand::Pipe {
                    source: Some(source),
                    cap_flat,
                    cap_round,
                    blend_global,
                    fit_rail,
                    wall_thickness,
                    pick_second_radius,
                    first_radius,
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
            return;
        }
        if let Some(InteractiveCommand::SelVolumeObject { mode }) = self.active_command {
            if let Some(source) = click.object_id {
                let eligible = self.document.object(source).is_some_and(|object| {
                    matches!(
                        object.geometry(),
                        Geometry::Mesh(_) | Geometry::Brep(_) | Geometry::NurbsSurface(_)
                    )
                });
                if eligible {
                    let mode_name = match mode {
                        RectSelectionMode::Automatic | RectSelectionMode::Crossing => "Crossing",
                        RectSelectionMode::Window => "Window",
                        RectSelectionMode::InvertWindow => "InvertWindow",
                        RectSelectionMode::InvertCrossing => "InvertCrossing",
                    };
                    self.active_command = None;
                    self.execute_command(&format!(
                        "SelVolumeObject {source} SelectionMode={mode_name}"
                    ));
                } else {
                    self.push_log("Select a closed mesh or polysurface".to_owned());
                }
            }
            return;
        }
        if let Some(InteractiveCommand::SelVolumePipe { source: None, mode }) = self.active_command
        {
            if let Some(source) = click.object_id
                && self
                    .document
                    .object(source)
                    .and_then(|object| object.geometry().curve_ref())
                    .is_some()
            {
                let command = InteractiveCommand::SelVolumePipe {
                    source: Some(source),
                    mode,
                };
                self.active_command = Some(command);
                self.push_log(command.prompt().to_owned());
            }
            return;
        }
        if let Some(mode) = self.boundary_selection {
            if let Some(id) = click.object_id {
                self.finish_boundary_selection(id, mode, click.mode);
            }
            return;
        }
        if self
            .fence_selection
            .as_ref()
            .is_some_and(|state| state.curve_pick)
        {
            if let Some(id) = click.object_id {
                self.finish_curve_fence_selection(id, click.mode);
            }
            return;
        }
        if self.picking_alignment_curve() {
            self.pick_alignment_curve(click.object_id);
            return;
        }
        if self.group_prompt == Some(group_prompt::GroupPrompt::Target) {
            self.pick_group_prompt_target(click.object_id);
            return;
        }
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(click.object_id, click.mode);
            return;
        }
        if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(click.object_id, click.mode);
            return;
        }
        if self.object_prompt.is_some() {
            self.select_prompt_objects(click.object_id, click.mode);
            return;
        }
        if click.mode == viboceros_document::SelectionMode::Replace {
            self.component_selection.clear();
        }
        match click.object_id {
            Some(id) => match self.document.select_object(id, click.mode) {
                Ok(count) => self.push_log(format!("Selected {count} object(s)")),
                Err(error) => self.push_log(format!("Error: {error}")),
            },
            None if click.mode == viboceros_document::SelectionMode::Replace => {
                let count = self.document.clear_selection();
                if count > 0 {
                    self.push_log(format!("Deselected {count} object(s)"));
                }
            }
            None => {}
        }
    }

    pub(super) fn show_selection_menu(&mut self, ui: &egui::Ui) -> Option<Option<ObjectId>> {
        let menu = self.selection_menu.as_ref()?;
        let mut picked = None;
        let mut hovered = None;
        egui::Window::new("Selection Menu")
            .id(egui::Id::new("viewport_selection_menu"))
            .fixed_pos(menu.choice.pointer + egui::vec2(12.0, 12.0))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (index, id) in menu.choice.object_ids.iter().copied().enumerate() {
                            let Some(object) = self.document.object(id) else {
                                continue;
                            };
                            let kind = match object.geometry() {
                                viboceros_document::Geometry::Point(_) => "point",
                                viboceros_document::Geometry::PointCloud(_) => "point cloud",
                                viboceros_document::Geometry::Line(_) => "line",
                                viboceros_document::Geometry::Circle(_) => "circle",
                                viboceros_document::Geometry::Arc(_) => "arc",
                                viboceros_document::Geometry::Ellipse(_) => "ellipse",
                                viboceros_document::Geometry::Polyline(_) => "polyline",
                                viboceros_document::Geometry::NurbsCurve(_)
                                | viboceros_document::Geometry::PolyCurve(_) => "curve",
                                viboceros_document::Geometry::NurbsSurface(_) => "surface",
                                viboceros_document::Geometry::Brep(_) => "polysurface",
                                viboceros_document::Geometry::Mesh(_)
                                | viboceros_document::Geometry::EmptyMesh => "mesh",
                                viboceros_document::Geometry::SubD(_) => "SubD",
                                viboceros_document::Geometry::BlockInstance(_) => "block instance",
                            };
                            let label = object.attributes().name().map_or_else(
                                || format!("{}. {kind}", index + 1),
                                |name| format!("{}. {name} ({kind})", index + 1),
                            );
                            let response = ui.selectable_label(index == menu.highlighted, label);
                            if response.hovered() {
                                hovered = Some(index);
                            }
                            if response.clicked() {
                                picked = Some(Some(id));
                            }
                        }
                        if ui.button("None").clicked() {
                            picked = Some(None);
                        }
                    });
            });
        if let Some(index) = hovered {
            self.selection_menu.as_mut().unwrap().highlighted = index;
        }
        picked
    }

    pub(super) fn apply_selection_window(&mut self, selection: SelectionWindow) {
        if self.tween_surfaces_prompt.is_some()
            || self.blend_surface_prompt.is_some()
            || self.fillet_surface_prompt.is_some()
            || self.refinement_prompt.is_some()
            || self.select_uv_prompt.is_some()
            || self.reparameterize_prompt.is_some()
        {
            return;
        }
        if self
            .boolean_two_prompt
            .as_ref()
            .is_some_and(|p| !p.cycling())
        {
            self.pick_boolean_two_objects(selection.object_ids, selection.mode);
            return;
        }
        self.apply_selection_region(selection, false);
    }

    pub(super) fn add_fence_point(
        &mut self,
        point: egui::Pos2,
        viewport: usize,
        mode: SelectionMode,
    ) {
        let Some(state) = self.fence_selection.as_ref() else {
            return;
        };
        if state.curve_pick {
            return;
        }
        if let Some(first_viewport) = state.viewport
            && first_viewport != viewport
        {
            self.push_log("Continue the fence in its starting viewport".into());
            return;
        }
        let Some(view) = self.viewports.get(viewport) else {
            return;
        };
        let Some(anchor) = view.fence_anchor(point) else {
            self.push_log("Fence point could not be placed in the view".into());
            return;
        };
        if state.points.last().is_some_and(|last| {
            view.project_fence(&[*last])
                .is_some_and(|projected| projected[0].distance(point) < 1.0)
        }) {
            return;
        }
        let state = self.fence_selection.as_mut().unwrap();
        state.viewport.get_or_insert(viewport);
        state.points.push(anchor);
        state.mode = mode;
    }

    pub(super) fn finish_fence_selection(&mut self) {
        if self.tween_surfaces_prompt.is_some()
            || self.blend_surface_prompt.is_some()
            || self.fillet_surface_prompt.is_some()
            || self.refinement_prompt.is_some()
            || self.select_uv_prompt.is_some()
            || self.reparameterize_prompt.is_some()
        {
            self.fence_selection = None;
            return;
        }
        let Some(state) = self.fence_selection.take() else {
            return;
        };
        if state.curve_pick {
            self.fence_selection = Some(state);
            self.push_log("Select a curve for the fence; Esc to cancel".into());
            return;
        }
        if state.points.len() < 2 {
            self.fence_selection = Some(state);
            self.push_log("Fence needs at least two distinct points; Esc to cancel".into());
            return;
        }
        let Some(viewport) = state.viewport else {
            return;
        };
        let Some(fence) = self.viewports[viewport].project_fence(&state.points) else {
            self.fence_selection = Some(state);
            self.push_log(
                "Fence is outside the current camera view; restore the view or Esc".into(),
            );
            return;
        };
        let Some(filter) = self.viewport_object_filter() else {
            self.fence_selection = Some(state);
            self.push_log("Fence selection is unavailable during this prompt".into());
            return;
        };
        let preview = self.selection_preview_filter();
        let ids = self.viewports[viewport].objects_crossed_by_fence_preview(
            &fence,
            &self.document,
            filter,
            preview,
        );
        self.apply_fence_selection_ids(ids, state.mode);
    }

    pub(super) fn finish_curve_fence_selection(&mut self, source: ObjectId, mode: SelectionMode) {
        if !self
            .fence_selection
            .as_ref()
            .is_some_and(|state| state.curve_pick)
        {
            return;
        }
        let Some(filter) = self.viewport_object_filter() else {
            self.push_log("Fence selection is unavailable during this prompt".into());
            return;
        };
        let preview = self.selection_preview_filter();
        let Some(ids) = self.viewports[self.active_viewport].objects_crossed_by_curve_preview(
            source,
            &self.document,
            filter,
            preview,
        ) else {
            self.push_log("The chosen curve has no visible fence stroke".into());
            return;
        };
        self.fence_selection = None;
        self.apply_fence_selection_ids(ids, mode);
    }

    pub(super) fn apply_fence_selection_ids(&mut self, ids: Vec<ObjectId>, mode: SelectionMode) {
        if self.pick_block_edit_sources(ids.iter().copied(), mode) {
            return;
        }
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(ids, mode);
        } else if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(ids, mode);
        } else if self.object_prompt.is_some() {
            self.select_prompt_objects(ids, mode);
        } else {
            match self.document.select_objects(ids, mode) {
                Ok(count) => self.push_log(format!("Fence selection: {count} object(s) selected")),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    pub(super) fn finish_boundary_selection(
        &mut self,
        source: ObjectId,
        region_mode: RectSelectionMode,
        selection_mode: SelectionMode,
    ) {
        if self.tween_surfaces_prompt.is_some()
            || self.blend_surface_prompt.is_some()
            || self.fillet_surface_prompt.is_some()
            || self.refinement_prompt.is_some()
            || self.select_uv_prompt.is_some()
            || self.reparameterize_prompt.is_some()
        {
            self.boundary_selection = None;
            return;
        }
        let Some(filter) = self.viewport_object_filter() else {
            self.push_log("Boundary selection is unavailable during this prompt".into());
            return;
        };
        let preview = self.selection_preview_filter();
        let Some(ids) = self.viewports[self.active_viewport].objects_in_boundary_curve_preview(
            source,
            region_mode,
            &self.document,
            filter,
            preview,
        ) else {
            self.push_log("Select a closed curve visible in the active viewport".into());
            return;
        };
        self.boundary_selection = None;
        if self.pick_block_edit_sources(ids.iter().copied(), selection_mode) {
            return;
        }
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(ids, selection_mode);
        } else if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(ids, selection_mode);
        } else if self.object_prompt.is_some() {
            self.select_prompt_objects(ids, selection_mode);
        } else {
            match self.document.select_objects(ids, selection_mode) {
                Ok(count) => {
                    self.push_log(format!("Boundary selection: {count} object(s) selected"))
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    pub(super) fn accept_lasso_point(
        &mut self,
        point: egui::Pos2,
        viewport: usize,
        mode: SelectionMode,
    ) {
        let Some(state) = self.lasso_selection.as_mut() else {
            return;
        };
        if state.viewport.is_some_and(|active| active != viewport) || !point.is_finite() {
            return;
        }
        state.viewport = Some(viewport);
        if state.points.is_empty() {
            state.selection_mode = mode;
        }
        if state.points.len() >= 3 && state.points[0].distance(point) <= 5.0 {
            self.finish_lasso_selection();
            return;
        }
        if state
            .points
            .last()
            .is_none_or(|last| last.distance(point) >= 1.0)
        {
            state.points.push(point);
        }
    }

    pub(super) fn accept_lasso_stroke(
        &mut self,
        path: Vec<egui::Pos2>,
        viewport: usize,
        mode: SelectionMode,
    ) {
        let Some(state) = self.lasso_selection.as_mut() else {
            return;
        };
        if state.viewport.is_some_and(|active| active != viewport)
            || path.iter().any(|p| !p.is_finite())
        {
            return;
        }
        state.viewport = Some(viewport);
        if state.points.is_empty() {
            state.selection_mode = mode;
        }
        for point in path {
            if state
                .points
                .last()
                .is_none_or(|last| last.distance(point) >= 1.0)
            {
                state.points.push(point);
            }
        }
        if state.points.len() >= 3 {
            self.finish_lasso_selection();
        }
    }

    pub(super) fn finish_lasso_selection(&mut self) {
        let Some(state) = self.lasso_selection.take() else {
            return;
        };
        let Some(viewport) = state.viewport else {
            self.lasso_selection = Some(state);
            self.push_log("Draw a lasso in one viewport".into());
            return;
        };
        let Some(filter) = self.viewport_object_filter() else {
            self.lasso_selection = Some(state);
            self.push_log("Lasso selection unavailable during this prompt".into());
            return;
        };
        let preview = self.selection_preview_filter();
        let Some(ids) = self.viewports[viewport].objects_in_lasso_preview(
            &state.points,
            state.mode,
            &self.document,
            filter,
            preview,
        ) else {
            self.lasso_selection = Some(state);
            self.push_log("Lasso needs at least three noncollinear points".into());
            return;
        };
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(ids, state.selection_mode);
        } else if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(ids, state.selection_mode);
        } else if self.object_prompt.is_some() {
            self.select_prompt_objects(ids, state.selection_mode);
        } else {
            match self.document.select_objects(ids, state.selection_mode) {
                Ok(count) => self.push_log(format!("Lasso selection: {count} object(s) selected")),
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
    }

    pub(super) fn apply_selection_region(&mut self, selection: SelectionWindow, circular: bool) {
        if self.pick_surface_offset_points(selection.object_ids.iter().copied(), selection.mode) {
            return;
        }
        if self.tangent_session.is_some() {
            return;
        }
        if self.pick_block_edit_sources(selection.object_ids.iter().copied(), selection.mode) {
            return;
        }
        if self.select_block_add_region(&selection) {
            return;
        }
        if self.tween_surfaces_prompt.is_some()
            || self.blend_surface_prompt.is_some()
            || self.fillet_surface_prompt.is_some()
            || self.refinement_prompt.is_some()
            || self.select_uv_prompt.is_some()
            || self.reparameterize_prompt.is_some()
        {
            return;
        }
        if self.end_analysis_pick.is_some() {
            self.apply_end_analysis_pick_ids(selection.object_ids);
            return;
        }
        self.selection_window_override = None;
        if self.replacing_block()
            || self.flow_session.is_some()
            || self.surface_flow_session.is_some()
            || self.picking_alignment_curve()
            || self.picking_scale_by_plane_object()
            || self.picking_scale_by_plane_view()
            || self.remap_cplane_session.is_some()
        {
            // This phase needs one target; a window must not change the sources.
            return;
        }
        if self.group_prompt.is_some() {
            self.select_group_prompt_objects(selection.object_ids, selection.mode);
            return;
        }
        if self.intersection_prompt.is_some() {
            self.select_intersection_prompt_objects(selection.object_ids, selection.mode);
            return;
        }
        if self.object_prompt.is_some() {
            self.select_prompt_objects(selection.object_ids, selection.mode);
            return;
        }
        let selected_kind = match (selection.crossing, selection.inverted) {
            (false, false) => "window",
            (true, false) => "crossing",
            (false, true) => "inverse window",
            (true, true) => "inverse crossing",
        };
        if selection.mode == viboceros_document::SelectionMode::Replace {
            self.component_selection.clear();
        }
        match self
            .document
            .select_objects(selection.object_ids, selection.mode)
        {
            Ok(count) => self.push_log(format!(
                "{}{selected_kind} selection: {count} object(s) selected",
                if circular { "circular " } else { "" },
            )),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }
}
