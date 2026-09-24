//! Transparent two-point Zoom Target prompt, separate from model input.

use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(super) fn accept_zoom_target_pick(&mut self, point: Point3, viewport: usize) {
        if matches!(self.zoom_target, Some(ZoomTargetState::PickTarget)) {
            self.zoom_target = Some(ZoomTargetState::PickWindow {
                target: point,
                viewport,
            });
            self.push_log("Zoom Target: pick or type a window corner".into());
        }
    }

    pub(super) fn finish_zoom_target(&mut self, result: Result<bool, &'static str>) {
        match result {
            Ok(changed) => {
                self.zoom_target = None;
                self.push_log(if changed {
                    "Zoomed to target".into()
                } else {
                    "Zoom Target left view unchanged".into()
                });
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn try_continue_zoom_target(&mut self, input: &str) -> bool {
        let Some(state) = self.zoom_target else {
            return false;
        };
        if input.is_empty() {
            self.zoom_target = None;
            self.push_log("Zoom Target canceled".into());
            return true;
        }
        let Some(parsed) = PointInput::parse_with_units(input, self.document.units()) else {
            self.zoom_target = None;
            return false;
        };
        self.push_log(format!("> {input}"));
        let viewport = match state {
            ZoomTargetState::PickTarget => self.active_viewport,
            ZoomTargetState::PickWindow { viewport, .. } => viewport,
        };
        let previous = match state {
            ZoomTargetState::PickTarget => self.last_point,
            ZoomTargetState::PickWindow { target, .. } => Some(target),
        };
        let result = parsed.and_then(|point| {
            point.resolve(self.viewports[viewport].construction_plane(), previous)
        });
        let point = match result {
            Ok(point) => point,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        match state {
            ZoomTargetState::PickTarget => self.accept_zoom_target_pick(point, viewport),
            ZoomTargetState::PickWindow { target, .. } => {
                let result = self.viewports[viewport].zoom_target_from_point(target, point);
                if result.is_ok() {
                    self.command_input.clear();
                }
                self.finish_zoom_target(result);
                return true;
            }
        }
        self.command_input.clear();
        true
    }
}
