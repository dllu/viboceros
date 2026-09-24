//! Application adapter: interface actions do not enter the model command lifecycle.

use super::*;
use std::collections::HashSet;
use viboceros_command::interface::{
    self, GridUpdate, InterfaceCommand, InterfaceState, RectSelectionMode, SnapSpacing,
    SwitchAction, ViewportTarget, WorldView, ZoomFactor,
};

impl VibocerosApp {
    fn apply_grid_update(&mut self, update: GridUpdate, apply_to: ViewportTarget) {
        if update.is_empty() {
            self.grid_settings_open = true;
            self.grid_settings_apply_to = apply_to;
            self.push_log("Grid settings opened".into());
            return;
        }
        let indexes = match apply_to {
            ViewportTarget::Active => vec![self.active_viewport],
            ViewportTarget::All => (0..self.viewports.len()).collect::<Vec<_>>(),
        };
        let mut staged = Vec::with_capacity(indexes.len());
        for &index in &indexes {
            let mut grid = self.viewports[index].grid_settings();
            if let Some(value) = update.snap_spacing {
                grid.snap_spacing = value.value();
            }
            if let Some(value) = update.minor_spacing {
                grid.minor_spacing = value.value();
            }
            if let Some(value) = update.major_interval {
                grid.major_interval = value;
            }
            if let Some(value) = update.line_count {
                grid.line_count = value;
            }
            if let Some(value) = update.show_grid {
                grid.show_grid = value;
            }
            if let Some(value) = update.show_axes {
                grid.show_axes = value;
            }
            if let Some(value) = update.show_world_axes {
                grid.show_world_axes = value;
            }
            if !grid.valid() {
                self.push_log("Error: Grid settings exceed the supported finite range".into());
                return;
            }
            staged.push(grid);
        }
        for (index, grid) in indexes.into_iter().zip(staged) {
            self.viewports[index].set_grid_settings(grid);
        }
        self.push_log(format!(
            "Grid settings updated ({})",
            if apply_to == ViewportTarget::All {
                "all viewports"
            } else {
                "active viewport"
            }
        ));
    }

    fn set_snap_size(&mut self, spacing: SnapSpacing, apply_to: ViewportTarget, active: usize) {
        match apply_to {
            ViewportTarget::Active => self.viewports[active].set_snap_spacing(spacing.value()),
            ViewportTarget::All => {
                for viewport in &mut self.viewports {
                    viewport.set_snap_spacing(spacing.value());
                }
            }
        }
        self.snap_size_pending = None;
        self.push_log(format!(
            "Grid snap spacing: {} ({})",
            spacing.value(),
            if apply_to == ViewportTarget::All {
                "all viewports"
            } else {
                "active viewport"
            }
        ));
    }

    pub(super) fn start_end_analysis_pick(&mut self, mode: EndAnalysisPickMode) {
        if mode != EndAnalysisPickMode::Show && self.end_analysis.is_none() {
            self.push_log("Run ShowEnds first".into());
            return;
        }
        self.cancel_end_analysis_pick(false);
        self.end_analysis_pick = Some(EndAnalysisPick {
            mode,
            before: self.end_analysis.clone(),
            started: false,
        });
        self.push_log("Pick curves in a viewport; Enter finishes, Esc cancels".into());
    }

    pub(super) fn cancel_end_analysis_pick(&mut self, announce: bool) {
        if let Some(pick) = self.end_analysis_pick.take() {
            self.end_analysis = pick.before;
            if announce {
                self.push_log("End Analysis selection canceled".into());
            }
        }
    }

    pub(super) fn finish_end_analysis_pick(&mut self) {
        if let Some(pick) = self.end_analysis_pick.take() {
            if !pick.started {
                self.end_analysis = pick.before;
            }
            self.push_log("End Analysis selection finished".into());
        }
    }

    pub(super) fn apply_end_analysis_pick_ids(&mut self, ids: Vec<ObjectId>) {
        let Some(pick) = self.end_analysis_pick.as_ref() else {
            return;
        };
        let mode = pick.mode;
        let started = pick.started;
        if ids.is_empty() {
            return;
        }
        if mode == EndAnalysisPickMode::Show && !started {
            match collect_end_markers(
                &self.document,
                ids.iter().copied(),
                EndMarkerOptions::default(),
            ) {
                Ok(markers) if !markers.is_empty() => {
                    self.end_analysis = Some(EndAnalysisState::new(Vec::new()));
                    self.end_analysis_pick.as_mut().unwrap().started = true;
                }
                Ok(_) => return,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return;
                }
            }
        }
        match mode {
            EndAnalysisPickMode::Show | EndAnalysisPickMode::Add => self.add_end_analysis_ids(ids),
            EndAnalysisPickMode::Remove => self.remove_end_analysis_ids(ids),
        }
        if let Some(pick) = self.end_analysis_pick.as_mut() {
            pick.started = true;
        }
    }

    pub(super) fn add_selected_to_end_analysis(&mut self) {
        let ids = self.document.selected_object_ids().collect::<Vec<_>>();
        self.add_end_analysis_ids(ids);
    }

    fn add_end_analysis_ids(&mut self, ids: Vec<ObjectId>) {
        let Some(analysis) = &self.end_analysis else {
            return;
        };
        let mut seen = analysis.sources.iter().copied().collect::<HashSet<_>>();
        let additions = ids
            .into_iter()
            .filter(|id| {
                seen.insert(*id)
                    && self
                        .document
                        .object(*id)
                        .is_some_and(|object| object.geometry().curve_ref().is_some())
            })
            .collect::<Vec<_>>();
        if additions.is_empty() {
            self.push_log("No new selected curves for End Analysis".into());
            return;
        }
        let mut sources = analysis.sources.clone();
        sources.extend(additions.iter().copied());
        if let Err(error) = collect_end_markers(
            &self.document,
            sources.iter().copied(),
            EndMarkerOptions::default(),
        ) {
            self.push_log(format!("Error: {error}"));
            return;
        }
        if let Some(analysis) = self.end_analysis.as_mut() {
            analysis.sources = sources;
            analysis.current = 0;
            analysis.all_active = false;
        }
        self.push_log(format!(
            "Added {} curve(s) to End Analysis",
            additions.len()
        ));
    }

    pub(super) fn remove_selected_from_end_analysis(&mut self) {
        let ids = self.document.selected_object_ids().collect::<Vec<_>>();
        self.remove_end_analysis_ids(ids);
    }

    fn remove_end_analysis_ids(&mut self, ids: Vec<ObjectId>) {
        let Some(analysis) = self.end_analysis.as_mut() else {
            return;
        };
        let selected = ids.into_iter().collect::<HashSet<_>>();
        let previous = analysis.sources.len();
        analysis.sources.retain(|id| !selected.contains(id));
        let removed = previous - analysis.sources.len();
        if removed != 0 {
            analysis.current = 0;
            analysis.all_active = false;
        }
        self.push_log(format!("Removed {removed} curve(s) from End Analysis"));
    }

    fn mark_end_analysis(&mut self) {
        let Some(analysis) = &self.end_analysis else {
            self.push_log("Run ShowEnds with visible selected curves first".into());
            return;
        };
        let markers = match collect_end_markers(
            &self.document,
            analysis.sources.iter().copied(),
            analysis.options,
        ) {
            Ok(markers) => markers,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        };
        if markers.is_empty() {
            self.push_log("No visible curve end markers to mark".into());
            return;
        }
        let points = if analysis.all_active {
            markers
                .iter()
                .map(|marker| marker.point)
                .collect::<Vec<_>>()
        } else {
            vec![markers[analysis.current % markers.len()].point]
        };
        if let Err(error) = self.document.begin_transaction("Mark curve ends") {
            self.push_log(format!("Error: {error}"));
            return;
        }
        for point in &points {
            if let Err(error) = self.document.add_geometry(Geometry::Point(*point)) {
                if let Err(rollback) = self.document.rollback_transaction() {
                    self.push_log(format!("Error rolling back Mark curve ends: {rollback}"));
                }
                self.push_log(format!("Error: {error}"));
                return;
            }
        }
        match self.document.commit_transaction() {
            Ok(_) => self.push_log(format!("Marked {} curve end(s)", points.len())),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn can_capture_selection(&self) -> bool {
        self.viewport_object_filter().is_some()
            && self.active_command.is_none()
            && self.plane_prompt.is_none()
            && self.group_prompt != Some(group_prompt::GroupPrompt::Target)
    }

    pub(super) fn try_continue_region_selection_option(&mut self, input: &str) -> bool {
        if self.selection_window_override.is_none()
            && self.circular_selection.is_none()
            && self.boundary_selection.is_none()
            && self.lasso_selection.is_none()
            && !matches!(
                self.active_command,
                Some(
                    InteractiveCommand::SelVolumeSphere { .. }
                        | InteractiveCommand::SelVolumePipe { .. }
                        | InteractiveCommand::SelVolumeObject { .. }
                        | InteractiveCommand::SelBox { .. }
                )
            )
        {
            return false;
        }
        let name = input
            .split_once('=')
            .map_or(input, |(name, _)| name)
            .trim_start_matches('_');
        if !matches!(
            name.to_ascii_lowercase().as_str(),
            "selectionmode" | "window" | "crossing" | "invertwindow" | "invertcrossing"
        ) {
            return false;
        }
        let value = if name.eq_ignore_ascii_case("SelectionMode") {
            input.split_once('=').map(|(_, value)| value).unwrap_or("")
        } else {
            input
        };
        match RectSelectionMode::parse(value.trim_start_matches('_')) {
            Some(mode) => {
                if let Some(state) = self.circular_selection.as_mut() {
                    match state {
                        CircularSelectionState::PickCenter(current)
                        | CircularSelectionState::PickRadius { mode: current, .. } => {
                            *current = mode
                        }
                    }
                } else if self.boundary_selection.is_some() {
                    self.boundary_selection = Some(mode);
                } else if let Some(state) = self.lasso_selection.as_mut() {
                    state.mode = mode;
                } else if let Some(InteractiveCommand::SelVolumeSphere { center, .. }) =
                    self.active_command
                {
                    self.active_command =
                        Some(InteractiveCommand::SelVolumeSphere { center, mode });
                } else if let Some(InteractiveCommand::SelVolumePipe { source, .. }) =
                    self.active_command
                {
                    self.active_command = Some(InteractiveCommand::SelVolumePipe { source, mode });
                } else if matches!(
                    self.active_command,
                    Some(InteractiveCommand::SelVolumeObject { .. })
                ) {
                    self.active_command = Some(InteractiveCommand::SelVolumeObject { mode });
                } else if let Some(InteractiveCommand::SelBox { base, opposite, .. }) =
                    self.active_command
                {
                    self.active_command = Some(InteractiveCommand::SelBox {
                        base,
                        opposite,
                        mode,
                    });
                } else {
                    self.selection_window_override = Some(mode);
                }
                self.push_log(format!("Selection mode: {mode:?}"));
                self.command_input.clear();
            }
            _ => self.push_log(
                "Usage: SelectionMode=Window|Crossing|InvertWindow|InvertCrossing".into(),
            ),
        }
        true
    }

    pub(super) fn interface_state(&self) -> InterfaceState {
        InterfaceState {
            grid_snap: self.grid_snap,
            ortho: self.ortho,
            ortho_snap_to_cplane_z: self.ortho_snap_to_cplane_z,
            planar: self.planar,
            ortho_angle: self.ortho_angle,
            osnap: self.osnap,
            snap_to_meshes: self.snaps.mesh_edges,
            smart_track: self.smart_track,
            active_viewport: self.active_viewport,
            display_modes: self.viewports.iter().map(|v| v.display_mode).collect(),
        }
    }

    pub(super) fn apply_interface_command(&mut self, command: InterfaceCommand) {
        let mut state = self.interface_state();
        match state.apply(command) {
            Ok(message) => {
                if command == InterfaceCommand::ShowEnds {
                    let sources = self
                        .document
                        .selected_object_ids()
                        .filter(|id| {
                            self.document
                                .object(*id)
                                .is_some_and(|object| object.geometry().curve_ref().is_some())
                        })
                        .collect::<Vec<_>>();
                    let options = EndMarkerOptions::default();
                    match collect_end_markers(&self.document, sources.iter().copied(), options) {
                        Ok(markers) if !markers.is_empty() => {
                            self.end_analysis_pick = None;
                            self.end_analysis = Some(EndAnalysisState::new(sources));
                            self.push_log(format!(
                                "End Analysis: {} marker(s) displayed",
                                markers.len()
                            ));
                        }
                        Ok(_) => self.start_end_analysis_pick(EndAnalysisPickMode::Show),
                        Err(error) => self.push_log(format!("Error: {error}")),
                    }
                    return;
                }
                if command == InterfaceCommand::ShowEndsOff {
                    self.end_analysis_pick = None;
                    self.end_analysis = None;
                    self.push_log("End Analysis closed".into());
                    return;
                }
                if command == InterfaceCommand::ZoomEndsMark {
                    self.mark_end_analysis();
                    return;
                }
                if let InterfaceCommand::SnapSize { spacing, apply_to } = command {
                    self.zoom_factor_pending = None;
                    if let Some(spacing) = spacing {
                        self.set_snap_size(spacing, apply_to, self.active_viewport);
                    } else {
                        self.snap_size_pending = Some((apply_to, self.active_viewport));
                        self.push_log(
                            "SnapSize: enter a positive spacing; Enter or Esc to cancel".into(),
                        );
                    }
                    return;
                }
                if let InterfaceCommand::Grid { update, apply_to } = command {
                    self.apply_grid_update(update, apply_to);
                    return;
                }
                if command == InterfaceCommand::ToggleGrid {
                    let show_grid = self.viewports[self.active_viewport]
                        .grid_settings()
                        .show_grid;
                    self.apply_grid_update(
                        GridUpdate {
                            show_grid: Some(!show_grid),
                            ..GridUpdate::default()
                        },
                        ViewportTarget::Active,
                    );
                    return;
                }
                if command == InterfaceCommand::ZoomFactorPrompt {
                    self.snap_size_pending = None;
                    self.zoom_factor_pending = Some(self.active_viewport);
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.push_log(
                        "Zoom Factor: enter a positive number; Enter or Esc to cancel".into(),
                    );
                    return;
                }
                self.zoom_factor_pending = None;
                if matches!(
                    command,
                    InterfaceCommand::NextViewport
                        | InterfaceCommand::PrevViewport
                        | InterfaceCommand::NextOrthoViewport
                        | InterfaceCommand::NextPerspectiveViewport
                ) {
                    let count = self.viewports.len();
                    let next = (1..=count).map(|offset| {
                        if command == InterfaceCommand::PrevViewport {
                            (self.active_viewport + count - offset) % count
                        } else {
                            (self.active_viewport + offset) % count
                        }
                    });
                    let eligible = next.into_iter().find(|&index| match command {
                        InterfaceCommand::NextOrthoViewport => {
                            self.viewports[index].kind() != ViewKind::Perspective
                        }
                        InterfaceCommand::NextPerspectiveViewport => {
                            self.viewports[index].kind() == ViewKind::Perspective
                        }
                        _ => true,
                    });
                    if let Some(index) = eligible {
                        self.active_viewport = index;
                        self.push_log(format!(
                            "Active viewport: {} ({})",
                            index + 1,
                            self.viewports[index].view_label()
                        ));
                    } else {
                        self.push_log("No matching viewport".into());
                    }
                    return;
                }
                if let InterfaceCommand::SelBoundary(mode) = command {
                    if !self.can_capture_selection() {
                        self.push_log("Boundary selection unavailable during this prompt".into());
                        return;
                    }
                    self.boundary_selection = Some(mode);
                    self.selection_window_override = None;
                    self.circular_selection = None;
                    self.fence_selection = None;
                    self.lasso_selection = None;
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    if self.object_prompt.is_none() {
                        let source = {
                            let mut selected = self.document.selected_object_ids();
                            selected.next().filter(|_| selected.next().is_none())
                        };
                        if let Some(source) = source
                            && self
                                .document
                                .object(source)
                                .and_then(|object| object.geometry().curve_ref())
                                .is_some_and(|curve| curve.is_closed().unwrap_or(false))
                        {
                            self.finish_boundary_selection(
                                source,
                                mode,
                                viboceros_document::SelectionMode::Replace,
                            );
                            return;
                        }
                    }
                    self.push_log("Select a closed boundary curve; Esc to cancel".into());
                    return;
                }
                if matches!(
                    command,
                    InterfaceCommand::SelFence | InterfaceCommand::SelFenceCurve
                ) {
                    if !self.can_capture_selection() {
                        self.push_log("Fence selection unavailable during this prompt".into());
                        return;
                    }
                    self.fence_selection = Some(FenceSelectionState {
                        viewport: None,
                        points: Vec::new(),
                        mode: viboceros_document::SelectionMode::Replace,
                        curve_pick: command == InterfaceCommand::SelFenceCurve,
                    });
                    self.selection_window_override = None;
                    self.circular_selection = None;
                    self.boundary_selection = None;
                    self.lasso_selection = None;
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.push_log(if command == InterfaceCommand::SelFenceCurve {
                        "Select an existing curve for the fence; Esc to cancel".into()
                    } else {
                        "Click fence points in one viewport; Enter/right-click to select, Esc to cancel"
                            .into()
                    });
                    return;
                }
                if let InterfaceCommand::SelCircular(mode) = command {
                    if !self.can_capture_selection() {
                        self.push_log("Circular selection unavailable during this prompt".into());
                        return;
                    }
                    self.circular_selection = Some(CircularSelectionState::PickCenter(mode));
                    self.fence_selection = None;
                    self.boundary_selection = None;
                    self.lasso_selection = None;
                    self.selection_window_override = None;
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.push_log("Select the circle center in a viewport; Esc to cancel".into());
                    return;
                }
                if let InterfaceCommand::Lasso(mode) = command {
                    if !self.can_capture_selection() {
                        self.push_log("Lasso selection unavailable during this prompt".into());
                        return;
                    }
                    self.lasso_selection = Some(LassoSelectionState {
                        viewport: None,
                        points: Vec::new(),
                        mode,
                        selection_mode: viboceros_document::SelectionMode::Replace,
                    });
                    self.selection_window_override = None;
                    self.circular_selection = None;
                    self.fence_selection = None;
                    self.boundary_selection = None;
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.push_log("Drag a lasso or click its outline; Enter/right-click to finish, Undo removes a point, Esc cancels".into());
                    return;
                }
                if matches!(
                    command,
                    InterfaceCommand::SelWindow
                        | InterfaceCommand::SelCrossing
                        | InterfaceCommand::SelRectangular(_)
                ) {
                    if !self.can_capture_selection() {
                        self.push_log("Selection window unavailable during this prompt".into());
                        return;
                    }
                    let mode = match command {
                        InterfaceCommand::SelWindow => RectSelectionMode::Window,
                        InterfaceCommand::SelCrossing => RectSelectionMode::Crossing,
                        InterfaceCommand::SelRectangular(mode) => mode,
                        _ => unreachable!(),
                    };
                    self.selection_window_override = Some(mode);
                    self.circular_selection = None;
                    self.fence_selection = None;
                    self.lasso_selection = None;
                    self.boundary_selection = None;
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.push_log(format!(
                        "Drag a {} selection in a viewport; Esc to cancel",
                        match mode {
                            RectSelectionMode::Automatic => "rectangular",
                            RectSelectionMode::Window => "window",
                            RectSelectionMode::Crossing => "crossing",
                            RectSelectionMode::InvertWindow => "inverse window",
                            RectSelectionMode::InvertCrossing => "inverse crossing",
                        }
                    ));
                    return;
                }
                self.selection_window_override = None;
                self.circular_selection = None;
                self.fence_selection = None;
                self.lasso_selection = None;
                self.boundary_selection = None;
                if matches!(
                    command,
                    InterfaceCommand::ZoomEnds
                        | InterfaceCommand::ZoomEndsCurrent
                        | InterfaceCommand::ZoomEndsNext
                        | InterfaceCommand::ZoomEndsPrevious
                ) {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    let result = if let Some(analysis) = &self.end_analysis {
                        collect_end_markers(
                            &self.document,
                            analysis.sources.iter().copied(),
                            analysis.options,
                        )
                        .and_then(|markers| {
                            if markers.is_empty() {
                                return Ok((false, None));
                            }
                            let index = match command {
                                InterfaceCommand::ZoomEnds => None,
                                InterfaceCommand::ZoomEndsCurrent => {
                                    Some(analysis.current % markers.len())
                                }
                                InterfaceCommand::ZoomEndsNext => {
                                    Some((analysis.current + 1) % markers.len())
                                }
                                InterfaceCommand::ZoomEndsPrevious => {
                                    Some((analysis.current + markers.len() - 1) % markers.len())
                                }
                                _ => unreachable!(),
                            };
                            let focus = index.map_or(markers.as_slice(), |index| {
                                std::slice::from_ref(&markers[index])
                            });
                            self.viewports[self.active_viewport]
                                .zoom_end_markers(focus, self.zoom_extents_borders)
                                .map(|changed| (changed, index))
                        })
                    } else if command == InterfaceCommand::ZoomEnds {
                        self.viewports[self.active_viewport]
                            .zoom_curve_ends(&self.document, self.zoom_extents_borders)
                            .map(|changed| (changed, None))
                    } else {
                        self.push_log("Run ShowEnds with visible selected curves first".into());
                        return;
                    };
                    if let Ok((true, index)) = result
                        && let Some(analysis) = self.end_analysis.as_mut()
                    {
                        if let Some(index) = index {
                            analysis.current = index;
                        }
                        analysis.all_active = index.is_none();
                    }
                    self.push_log(match result {
                        Ok((true, Some(index))) => {
                            format!("Zoomed to curve end {} (active viewport)", index + 1)
                        }
                        Ok((true, None)) => "Zoomed to curve ends (active viewport)".into(),
                        Ok((false, _)) => "No visible curve end markers to zoom to".into(),
                        Err(error) => format!("Error: {error}"),
                    });
                    return;
                }
                if command == InterfaceCommand::ZoomWindow {
                    self.zoom_window_pending = true;
                    self.zoom_target = None;
                    self.push_log("Drag a window in a viewport to zoom; Esc to cancel".into());
                    return;
                }
                if command == InterfaceCommand::ZoomTarget {
                    self.zoom_window_pending = false;
                    self.zoom_target = Some(ZoomTargetState::PickTarget);
                    self.push_log("Zoom Target: pick or type the view center".into());
                    return;
                }
                if let InterfaceCommand::SetViewWorld(view) = command {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    let kind = match view {
                        WorldView::Top => ViewKind::Top,
                        WorldView::Bottom => ViewKind::Bottom,
                        WorldView::Front => ViewKind::Front,
                        WorldView::Back => ViewKind::Back,
                        WorldView::Right => ViewKind::Right,
                        WorldView::Left => ViewKind::Left,
                        WorldView::Perspective => ViewKind::Perspective,
                    };
                    self.viewports[self.active_viewport].set_world_view(kind);
                    self.push_log(format!("World {} view (active viewport)", view.label()));
                    return;
                }
                if let InterfaceCommand::SetViewCPlane(direction) = command {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.viewports[self.active_viewport].set_cplane_view(direction);
                    self.push_log(format!(
                        "CPlane {} view (active viewport)",
                        direction.label()
                    ));
                    return;
                }
                if command == InterfaceCommand::Plan {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    self.viewports[self.active_viewport].set_plan_view();
                    self.push_log("Plan view aligned to current construction plane".into());
                    return;
                }
                if matches!(
                    command,
                    InterfaceCommand::UndoView | InterfaceCommand::RedoView
                ) {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    let changed = if command == InterfaceCommand::UndoView {
                        self.viewports[self.active_viewport].undo_view()
                    } else {
                        self.viewports[self.active_viewport].redo_view()
                    };
                    self.push_log(
                        match (command, changed) {
                            (InterfaceCommand::UndoView, true) => "Undid active viewport change",
                            (InterfaceCommand::UndoView, false) => "No viewport change to undo",
                            (InterfaceCommand::RedoView, true) => "Redid active viewport change",
                            (InterfaceCommand::RedoView, false) => "No viewport change to redo",
                            _ => unreachable!(),
                        }
                        .into(),
                    );
                    return;
                }
                if let InterfaceCommand::SetZoomScale(scale) = command {
                    self.zoom_scale = scale.value();
                    self.push_log(format!("View zoom scale factor: {}", self.zoom_scale));
                    return;
                }
                if let InterfaceCommand::SetZoomExtentsBorder {
                    parallel,
                    perspective,
                } = command
                {
                    if let Some(value) = parallel {
                        self.zoom_extents_borders.parallel = value.value();
                    }
                    if let Some(value) = perspective {
                        self.zoom_extents_borders.perspective = value.value();
                    }
                    self.push_log(format!(
                        "Zoom extents border scale: ParallelView={} PerspectiveView={}",
                        self.zoom_extents_borders.parallel, self.zoom_extents_borders.perspective
                    ));
                    return;
                }
                if let InterfaceCommand::ZoomFactor(_)
                | InterfaceCommand::ZoomIn
                | InterfaceCommand::ZoomOut = command
                {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    let (factor, result) = match command {
                        InterfaceCommand::ZoomFactor(factor) => (
                            factor.value(),
                            self.viewports[self.active_viewport].zoom_factor(factor.value()),
                        ),
                        InterfaceCommand::ZoomIn => (
                            self.zoom_scale.recip(),
                            self.viewports[self.active_viewport].zoom_in(self.zoom_scale),
                        ),
                        InterfaceCommand::ZoomOut => (
                            self.zoom_scale,
                            self.viewports[self.active_viewport].zoom_out(self.zoom_scale),
                        ),
                        _ => unreachable!(),
                    };
                    self.push_log(match result {
                        Ok(true) => format!("Zoomed by factor {factor} (active viewport)"),
                        Ok(false) => {
                            "Zoom unchanged (factor has no effect at current precision or limits)"
                                .into()
                        }
                        Err(error) => format!("Error: {error}"),
                    });
                    return;
                }
                if matches!(
                    command,
                    InterfaceCommand::ZoomExtents
                        | InterfaceCommand::ZoomSelected
                        | InterfaceCommand::ZoomAllExtents
                        | InterfaceCommand::ZoomAllSelected
                ) {
                    self.zoom_window_pending = false;
                    self.zoom_target = None;
                    let selected = matches!(
                        command,
                        InterfaceCommand::ZoomSelected | InterfaceCommand::ZoomAllSelected
                    );
                    let all = matches!(
                        command,
                        InterfaceCommand::ZoomAllExtents | InterfaceCommand::ZoomAllSelected
                    );
                    let target = if all {
                        "all viewports"
                    } else {
                        "active viewport"
                    };
                    let result = if all {
                        Viewport::zoom_all(
                            &mut self.viewports,
                            &self.document,
                            selected,
                            self.zoom_extents_borders,
                        )
                    } else if selected {
                        self.viewports[self.active_viewport]
                            .zoom_selected(&self.document, self.zoom_extents_borders)
                    } else {
                        self.viewports[self.active_viewport]
                            .zoom_extents(&self.document, self.zoom_extents_borders)
                    };
                    self.push_log(match result {
                        Ok(true) if selected => {
                            format!("Zoomed to selected visible objects ({target})")
                        }
                        Ok(false) if selected => "No selected visible objects to zoom to".into(),
                        Ok(true) => format!("Zoomed to visible extents ({target})"),
                        Ok(false) => "No visible objects to zoom to".into(),
                        Err(error) => format!("Error: {error}"),
                    });
                    return;
                }
                self.grid_snap = state.grid_snap;
                self.ortho = state.ortho;
                self.ortho_snap_to_cplane_z = state.ortho_snap_to_cplane_z;
                self.planar = state.planar;
                self.ortho_angle = state.ortho_angle;
                self.osnap = state.osnap;
                self.snaps.mesh_edges = state.snap_to_meshes;
                self.smart_track = state.smart_track;
                for (viewport, mode) in self.viewports.iter_mut().zip(state.display_modes) {
                    viewport.display_mode = mode;
                }
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn try_continue_zoom_factor(&mut self, input: &str) -> bool {
        let Some(viewport) = self.zoom_factor_pending else {
            return false;
        };
        self.command_input.clear();
        if input.is_empty() {
            self.zoom_factor_pending = None;
            self.push_log("Zoom Factor canceled".into());
            return true;
        }
        self.push_log(format!("> {input}"));
        let Some(factor) = input.parse::<f64>().ok().and_then(ZoomFactor::try_new) else {
            self.push_log("Error: Zoom Factor requires a finite positive number".into());
            return true;
        };
        self.zoom_factor_pending = None;
        let result = self.viewports[viewport].zoom_factor(factor.value());
        self.push_log(match result {
            Ok(true) => format!(
                "Zoomed by factor {} (viewport {})",
                factor.value(),
                viewport + 1
            ),
            Ok(false) => {
                "Zoom unchanged (factor has no effect at current precision or limits)".into()
            }
            Err(error) => format!("Error: {error}"),
        });
        true
    }

    pub(super) fn try_continue_snap_size(&mut self, input: &str) -> bool {
        let Some((apply_to, active)) = self.snap_size_pending else {
            return false;
        };
        self.command_input.clear();
        if input.is_empty() {
            self.snap_size_pending = None;
            self.push_log("SnapSize canceled".into());
            return true;
        }
        self.push_log(format!("> {input}"));
        let Some(spacing) = input.parse::<f64>().ok().and_then(SnapSpacing::try_new) else {
            self.push_log("Error: SnapSize requires a finite positive number".into());
            return true;
        };
        self.set_snap_size(spacing, apply_to, active);
        true
    }

    pub(super) fn try_run_interface_command(&mut self, input: &str) -> bool {
        let mut tokens = input.split_whitespace();
        if self.active_command.is_some()
            && input.split_whitespace().next().is_some_and(|name| {
                matches!(
                    name.trim_start_matches(['\'', '_', '-'])
                        .to_ascii_lowercase()
                        .as_str(),
                    "w" | "c"
                )
            })
        {
            return false;
        }
        if tokens.next().is_some_and(|name| {
            name.trim_start_matches(['\'', '_', '-'])
                .eq_ignore_ascii_case("Help")
        }) {
            self.push_log(format!("> {input}"));
            match tokens.collect::<Vec<_>>().as_slice() {
                [] => {
                    let mut names = self.commands.command_names();
                    names.extend(interface::COMMAND_NAMES);
                    names.push("CPlane");
                    names.sort_unstable();
                    names.dedup();
                    self.push_log(format!("Commands: {}", names.join(", ")));
                    self.push_log("Enter Help UI for interface commands and shortcuts.".into());
                    self.command_input.clear();
                }
                [topic] if topic.trim_start_matches('_').eq_ignore_ascii_case("UI") => {
                    self.push_log(interface::HELP.into());
                    self.push_log(snapping::HELP.into());
                    self.push_log(viboceros_command::construction_plane::USAGE.into());
                    self.command_input.clear();
                }
                _ => self.push_log("Usage: Help [UI]".into()),
            }
            return true;
        }
        let Some(command) = interface::parse(input) else {
            return false;
        };
        self.push_log(format!("> {input}"));
        match command {
            Ok(command) => {
                self.apply_interface_command(command);
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn handle_interface_shortcuts(&mut self, ui: &mut egui::Ui) {
        self.handle_plane_shortcuts(ui);
        // These keys have no text-editing meaning. Text editors retain their
        // own undo history; document undo/redo is not intercepted here.
        let shortcuts = [
            (
                egui::Modifiers::COMMAND,
                egui::Key::Tab,
                InterfaceCommand::NextViewport,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Tab,
                InterfaceCommand::PrevViewport,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::Home,
                InterfaceCommand::UndoView,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::End,
                InterfaceCommand::RedoView,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::W,
                InterfaceCommand::ZoomWindow,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::E,
                InterfaceCommand::ZoomExtents,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::E,
                InterfaceCommand::ZoomAllExtents,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::F7,
                InterfaceCommand::ToggleGrid,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::F8,
                InterfaceCommand::SetOrtho(SwitchAction::Toggle),
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::F9,
                InterfaceCommand::SetSnap(SwitchAction::Toggle),
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::F4,
                InterfaceCommand::SetOsnap(SwitchAction::Toggle),
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::W,
                InterfaceCommand::SetDisplayMode {
                    viewport: ViewportTarget::Active,
                    mode: DisplayMode::Wireframe,
                },
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::S,
                InterfaceCommand::SetDisplayMode {
                    viewport: ViewportTarget::Active,
                    mode: DisplayMode::Shaded,
                },
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::G,
                InterfaceCommand::SetDisplayMode {
                    viewport: ViewportTarget::Active,
                    mode: DisplayMode::Ghosted,
                },
            ),
        ];
        // Preserve event order, reject extra modifiers (notably Alt+F4), and
        // consume auto-repeat without repeatedly toggling a setting.
        let text_edit_focused = ui.ctx().text_edit_focused();
        let actions = ui.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } = event
                    && let Some((_, _, command)) =
                        shortcuts.iter().find(|(pattern, shortcut_key, _)| {
                            key == shortcut_key && modifiers.matches_exact(*pattern)
                        })
                    && !(text_edit_focused
                        && matches!(
                            *command,
                            InterfaceCommand::UndoView | InterfaceCommand::RedoView
                        ))
                {
                    if *pressed && !*repeat {
                        actions.push(*command);
                    }
                    false
                } else {
                    true
                }
            });
            actions
        });
        for action in actions {
            self.apply_interface_command(action);
        }
    }
}
