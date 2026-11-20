//! Roadlike directions use the ordinary two-point drafting getter.
use super::*;
use viboceros_geometry::GeometryError;

pub(super) fn between(start: Point3, end: Point3) -> Result<UnitVector3, GeometryError> {
    start.vector_to(end)?.normalized_nonzero()
}

pub(super) fn requests_axis(input: &str, value: Option<&str>) -> bool {
    let (key, value) = if let Some(key) = value {
        (key, input)
    } else if let Some(pair) = input.split_once('=') {
        pair
    } else {
        return false;
    };
    (key.eq_ignore_ascii_case("Style") || key.eq_ignore_ascii_case("FrameStyle"))
        && value
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Roadlike")
}

impl VibocerosApp {
    pub(super) fn begin_sweep_axis(&mut self) {
        let session = self.sweep_session.as_mut().unwrap();
        session.picking = Some(Pick::AxisStart);
        session.value = None;
        self.active_command = Some(InteractiveCommand::SweepPoint);
        self.drafting_plane = Some(self.viewports[self.active_viewport].construction_plane());
        self.log_sweep_input();
    }

    pub(in crate::app) fn sweep_axis_origin(&self) -> Option<Point3> {
        match self.sweep_session.as_ref()?.picking? {
            Pick::AxisEnd(start) => Some(start),
            _ => None,
        }
    }

    pub(super) fn continue_sweep_axis_input(&mut self, input: &str) -> bool {
        let Some(pick @ (Pick::AxisStart | Pick::AxisEnd(_))) = self
            .sweep_session
            .as_ref()
            .and_then(|session| session.picking)
        else {
            return false;
        };
        let automatic = input.eq_ignore_ascii_case("Automatic");
        if automatic || input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            if self.validate_sweep_sources() {
                let axis = (!automatic).then(|| {
                    self.drafting_plane
                        .unwrap_or_else(|| {
                            self.viewports[self.active_viewport].construction_plane()
                        })
                        .z_axis()
                });
                self.accept_sweep_draft_change(DraftChange::Axis(axis));
            }
        } else if input.eq_ignore_ascii_case("Undo") {
            if matches!(pick, Pick::AxisEnd(_)) {
                let session = self.sweep_session.as_mut().unwrap();
                session.picking = Some(Pick::AxisStart);
                session.preview.clear_wires();
                self.snaps.model_override = None;
                self.point_filter = None;
                self.point_constraint = None;
                self.log_sweep_input();
            } else {
                self.cancel_sweep_axis();
            }
        } else if input.eq_ignore_ascii_case("Cancel") || input == "!" {
            self.cancel_sweep_axis();
        } else {
            return false;
        }
        self.command_input.clear();
        true
    }

    pub(in crate::app) fn cancel_sweep_axis(&mut self) -> bool {
        let Some(session) = self
            .sweep_session
            .as_mut()
            .filter(|session| matches!(session.picking, Some(Pick::AxisStart | Pick::AxisEnd(_))))
        else {
            return false;
        };
        session.picking = None;
        session.preview.clear_wires();
        self.clear_sweep_point_getter();
        self.command_input.clear();
        self.log_sweep_input();
        true
    }
}
