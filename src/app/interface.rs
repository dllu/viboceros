//! Application adapter: interface actions do not enter the model command lifecycle.

use super::*;
use viboceros_command::interface::{
    self, InterfaceCommand, InterfaceState, RectSelectionMode, SwitchAction, ViewportTarget,
    WorldView,
};

impl VibocerosApp {
    pub(super) fn try_continue_rect_selection_option(&mut self, input: &str) -> bool {
        if self.selection_window_override.is_none() {
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
        match interface::parse(&format!("SelRectangular {input}")) {
            Some(Ok(InterfaceCommand::SelRectangular(mode))) => {
                self.selection_window_override = Some(mode);
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
                if matches!(
                    command,
                    InterfaceCommand::SelWindow
                        | InterfaceCommand::SelCrossing
                        | InterfaceCommand::SelRectangular(_)
                ) {
                    if self.viewport_object_filter().is_none()
                        || self.active_command.is_some()
                        || self.plane_prompt.is_some()
                        || self.group_prompt == Some(group_prompt::GroupPrompt::Target)
                    {
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
