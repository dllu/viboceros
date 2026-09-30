//! A nestable view-option prompt which leaves modeling state intact.

use super::*;
use viboceros_command::interface;
use viboceros_command::set_view::{SetViewAnswer, SetViewPrompt};

pub(super) struct SetViewSession {
    pub prompt: SetViewPrompt,
    plane: Option<construction_plane::PlanePrompt>,
    plane_snap: Option<viboceros_drafting::ObjectSnapModes>,
    copy_source: Option<construction_plane::CopyCPlaneKind>,
    zoom_factor: Option<usize>,
    snap_size: Option<(interface::ViewportTarget, usize)>,
    zoom_target: Option<ZoomTargetState>,
    zoom_window: bool,
    selection_window: Option<RectSelectionMode>,
    circular_selection: Option<CircularSelectionState>,
    boundary_selection: Option<RectSelectionMode>,
    fence_selection: Option<FenceSelectionState>,
    lasso_selection: Option<LassoSelectionState>,
}

impl SetViewSession {
    pub(super) fn restore(self, app: &mut VibocerosApp) {
        app.plane_prompt = self
            .plane
            .filter(|prompt| prompt.viewport < app.viewports.len());
        app.snaps.plane_override = app.plane_prompt.as_ref().and(self.plane_snap);
        app.copy_cplane_source = self.copy_source;
        app.zoom_factor_pending = self
            .zoom_factor
            .filter(|&index| index < app.viewports.len());
        app.snap_size_pending = self
            .snap_size
            .filter(|&(_, index)| index < app.viewports.len());
        app.zoom_target = self.zoom_target.filter(|state| match state {
            ZoomTargetState::PickTarget => true,
            ZoomTargetState::PickWindow { viewport, .. } => *viewport < app.viewports.len(),
        });
        app.zoom_window_pending = self.zoom_window;
        app.selection_window_override = self.selection_window;
        app.circular_selection = self.circular_selection.filter(|state| match state {
            CircularSelectionState::PickCenter(_) => true,
            CircularSelectionState::PickRadius { viewport, .. } => *viewport < app.viewports.len(),
        });
        app.boundary_selection = self.boundary_selection;
        app.fence_selection = self.fence_selection.filter(|state| {
            state
                .viewport
                .is_none_or(|index| index < app.viewports.len())
        });
        app.lasso_selection = self.lasso_selection.filter(|state| {
            state
                .viewport
                .is_none_or(|index| index < app.viewports.len())
        });
    }

    pub(super) fn viewport_closed(&mut self, removed: usize) {
        let remap = |index: usize| {
            if index == removed {
                None
            } else {
                Some(index - usize::from(index > removed))
            }
        };
        self.plane = self.plane.take().and_then(|mut prompt| {
            prompt.viewport = remap(prompt.viewport)?;
            Some(prompt)
        });
        if self.plane.is_none() {
            self.plane_snap = None;
        }
        self.zoom_factor = self.zoom_factor.and_then(remap);
        self.snap_size = self
            .snap_size
            .and_then(|(target, index)| remap(index).map(|index| (target, index)));
        self.zoom_target = self.zoom_target.and_then(|state| match state {
            ZoomTargetState::PickTarget => Some(state),
            ZoomTargetState::PickWindow { target, viewport } => {
                remap(viewport).map(|viewport| ZoomTargetState::PickWindow { target, viewport })
            }
        });
        self.circular_selection = self.circular_selection.and_then(|state| match state {
            CircularSelectionState::PickCenter(_) => Some(state),
            CircularSelectionState::PickRadius {
                mode,
                center,
                viewport,
            } => remap(viewport).map(|viewport| CircularSelectionState::PickRadius {
                mode,
                center,
                viewport,
            }),
        });
        self.fence_selection = self.fence_selection.take().and_then(|mut state| {
            if let Some(index) = state.viewport {
                state.viewport = Some(remap(index)?);
            }
            Some(state)
        });
        self.lasso_selection = self.lasso_selection.take().and_then(|mut state| {
            if let Some(index) = state.viewport {
                state.viewport = Some(remap(index)?);
            }
            Some(state)
        });
    }
}

impl VibocerosApp {
    pub(super) fn try_run_view_command(&mut self, input: &str) -> bool {
        self.try_run_plane_command(input)
            || self.try_run_copy_cplane_command(input)
            || self.try_run_synchronize_cplanes_command(input)
            || self.try_run_named_view_command(input)
            || self.try_run_named_cplane_command(input)
            || self.try_run_read_viewports_command(input)
            || self.try_run_viewport_properties_command(input)
            || self.try_run_interface_command(input)
    }

    pub(super) fn set_view_options_active(&self) -> bool {
        self.set_view_prompt.is_some()
            && self.plane_prompt.is_none()
            && self.copy_cplane_source.is_none()
            && self.zoom_factor_pending.is_none()
            && self.snap_size_pending.is_none()
            && self.zoom_target.is_none()
            && !self.zoom_window_pending
    }

    pub(super) fn start_set_view_prompt(&mut self, prompt: SetViewPrompt) {
        if self.set_view_prompt.is_some() && !self.set_view_options_active() {
            self.push_log(
                "Error: finish or cancel the current view command before restarting SetView".into(),
            );
            self.command_input.clear();
            return;
        }
        if let Some(session) = &mut self.set_view_prompt {
            session.prompt = prompt;
        } else {
            self.set_view_prompt = Some(SetViewSession {
                prompt,
                plane: self.plane_prompt.take(),
                plane_snap: self.snaps.plane_override.take(),
                copy_source: self.copy_cplane_source.take(),
                zoom_factor: self.zoom_factor_pending.take(),
                snap_size: self.snap_size_pending.take(),
                zoom_target: self.zoom_target.take(),
                zoom_window: std::mem::take(&mut self.zoom_window_pending),
                selection_window: self.selection_window_override.take(),
                circular_selection: self.circular_selection.take(),
                boundary_selection: self.boundary_selection.take(),
                fence_selection: self.fence_selection.take(),
                lasso_selection: self.lasso_selection.take(),
            });
        }
        self.command_input.clear();
        self.command_focus_requested = true;
        self.push_log(prompt.message().into());
    }

    pub(super) fn cancel_set_view_prompt(&mut self) {
        if let Some(session) = self.set_view_prompt.take() {
            session.restore(self);
            self.command_input.clear();
            self.command_focus_requested = true;
            self.push_log("SetView canceled; previous command retained".into());
        }
    }

    pub(super) fn try_continue_set_view(&mut self, input: &str) -> bool {
        if self.set_view_prompt.is_none() {
            return matches!(
                interface::parse(input),
                Some(Ok(interface::InterfaceCommand::SetViewPrompt(_)))
            ) && self.try_run_view_command(input);
        }
        if !self.set_view_options_active() {
            // Route child view input before suspended selection/model prompts
            // can interpret Enter, numbers, or construction-plane options.
            if input == "!" {
                self.cancel_current_prompt_or_selection();
            } else if input.is_empty() && self.copy_cplane_source.is_some() {
                self.accept_copy_cplane_source(self.active_viewport);
            } else if self.try_one_shot_snap(input)
                || (!input.is_empty() && self.try_run_view_command(input))
                || self.try_continue_zoom_target(input)
                || self.try_continue_snap_size(input)
                || self.try_continue_zoom_factor(input)
                || self.try_continue_plane_prompt(input)
            {
                // The child command handled this input.
            } else {
                self.command_input.clear();
                self.push_log("Error: finish or cancel the current view prompt".into());
            }
            return true;
        }
        let prompt = self.set_view_prompt.as_ref().unwrap().prompt;
        match prompt.answer(input) {
            Ok(SetViewAnswer::Prompt(next)) => self.start_set_view_prompt(next),
            Ok(SetViewAnswer::Cancel) => self.cancel_set_view_prompt(),
            Ok(SetViewAnswer::Command(command)) => {
                self.apply_interface_command(command);
                self.command_input.clear();
                self.command_focus_requested = true;
            }
            Err(error) => {
                // Rhino's option input admits transparent view commands. In
                // particular CPlane may open its own prompt inside this one.
                let name = input
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches(['\'', '_', '-']);
                if interface::parse(input).is_some()
                    || matches!(
                        name.to_ascii_lowercase().as_str(),
                        "cplane"
                            | "copycplanetoall"
                            | "copycplanesettingstoall"
                            | "synchronizecplanes"
                            | "namedview"
                            | "namedcplane"
                            | "readviewportsfromfile"
                            | "setactiveviewport"
                            | "setmaximizedviewport"
                            | "viewportproperties"
                            | "help"
                    )
                {
                    return false;
                }
                self.command_input.clear();
                self.push_log(format!("Error: {error}"));
                self.push_log(prompt.message().into());
            }
        }
        true
    }

    pub(super) fn show_set_view_choices(&mut self, ui: &mut egui::Ui) {
        if !self.set_view_options_active() {
            return;
        }
        let prompt = self.set_view_prompt.as_ref().unwrap().prompt;
        let mut chosen = None;
        ui.horizontal_wrapped(|ui| {
            ui.small("SetView:");
            for &choice in prompt.choices() {
                if ui.button(choice).clicked() {
                    chosen = Some(choice);
                }
            }
        });
        if let Some(choice) = chosen {
            self.command_input = choice.into();
            self.run_command();
        }
    }
}
