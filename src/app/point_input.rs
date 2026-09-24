//! Route typed and picked points through the same interactive command state.

use super::*;
use viboceros_drafting::{
    PointConstraintInput, PointConstraintState, PointFilter, PointFilterSession, PointInput,
};

/// Curve's point prompt uses a fixed coordinate-wise comparison, not model
/// distance tolerance. Private Rhino probes check the inclusive 2^-32 boundary
/// and diagonal offsets (see docs/control-point-threshold-measurement.json).
pub(super) fn coincident_curve_controls(first: Point3, second: Point3) -> bool {
    const ZERO_TOLERANCE: f64 = 2.3283064365386963e-10; // 2^-32
    (first.x() - second.x()).abs() <= ZERO_TOLERANCE
        && (first.y() - second.y()).abs() <= ZERO_TOLERANCE
        && (first.z() - second.z()).abs() <= ZERO_TOLERANCE
}

impl VibocerosApp {
    pub(super) fn try_continue_point_constraint(&mut self, input: &str) -> bool {
        let Some(parsed) = PointConstraintInput::parse_with_units(input, self.document.units())
        else {
            return false;
        };
        if self.active_command.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        let constraint = match parsed {
            Ok(value) => value,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let anchor = if self
            .active_command
            .is_some_and(InteractiveCommand::collects_curve_points)
        {
            self.curve_points.last().copied()
        } else {
            self.active_command.and_then(InteractiveCommand::anchor)
        };
        let Some(anchor) = anchor else {
            self.push_log("Error: distance and angle constraints require a previous point".into());
            return true;
        };
        let plane = self.viewports[self.active_viewport].construction_plane();
        let mut state = self
            .point_constraint
            .unwrap_or_else(|| PointConstraintState::new(anchor, plane));
        match state.set(constraint, plane) {
            Ok(()) => {
                self.point_constraint = Some(state);
                self.command_input.clear();
                self.push_log(format!("Constraint: {input}"));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn try_continue_point_filter(&mut self, input: &str) -> bool {
        let Some(filter) = PointFilter::parse(input) else {
            return false;
        };
        if self.active_command.is_none()
            || self.active_command == Some(InteractiveCommand::DomainFace)
            || self.plane_prompt.is_some()
        {
            return false;
        }
        if self.point_filter.is_some() {
            self.push_log("Error: finish the current filtered point first".into());
        } else {
            self.point_filter = Some(PointFilterSession::new(
                filter,
                self.viewports[self.active_viewport].construction_plane(),
            ));
            self.push_log(format!("{input}: pick a coordinate source"));
            self.command_input.clear();
        }
        true
    }

    pub(super) fn try_continue_point_input(&mut self, input: &str) -> bool {
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            return false;
        }
        let Some(parsed) = PointInput::parse_with_units(input, self.document.units()) else {
            return false;
        };
        let point = parsed.and_then(|input| {
            input.resolve(
                self.viewports[self.active_viewport].construction_plane(),
                self.last_point,
            )
        });
        match point {
            Ok(point) => {
                self.push_log(format!("> {input}"));
                self.accept_filtered_drafting_point(point, true);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn accept_filtered_drafting_point(&mut self, point: Point3, typed: bool) -> bool {
        let point = if let Some(session) = self.point_filter.as_mut() {
            match session.offer_point(point) {
                Ok(None) => {
                    self.snaps.model_override = None;
                    self.command_input.clear();
                    self.push_log("Filter coordinate captured; pick remaining coordinates".into());
                    return true;
                }
                Ok(Some(point)) => point,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            }
        } else {
            point
        };
        let point = match self.point_constraint.map_or(Ok(point), |state| {
            if typed {
                state.apply_typed(point)
            } else {
                state.apply_cursor(point)
            }
        }) {
            Ok(point) => point,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        self.accept_drafting_point(point)
    }

    pub(super) fn accept_drafting_point(&mut self, point: Point3) -> bool {
        let plane = self.viewports[self.active_viewport].construction_plane();
        if self.apply_drafting_point(point) {
            self.snaps.model_override = None;
            if self.active_command.is_some() {
                self.drafting_plane.get_or_insert(plane);
            }
            self.last_point = Some(point);
            self.point_filter = None;
            self.point_constraint = None;
            self.command_input.clear();
            true
        } else {
            false
        }
    }
}

pub(super) fn plane_radius_exceeds_tolerance(
    plane: Frame3,
    center: Point3,
    point: Point3,
    tolerance: Tolerance,
) -> bool {
    plane
        .with_origin(center)
        .coordinates_of(point)
        .is_ok_and(|[x, y, _]| x.hypot(y) > tolerance.absolute())
}

pub(super) fn plane_rectangle_exceeds_tolerance(
    plane: Frame3,
    first: Point3,
    opposite: Point3,
    tolerance: Tolerance,
) -> bool {
    plane
        .with_origin(first)
        .coordinates_of(opposite)
        .is_ok_and(|[x, y, _]| x.abs() > tolerance.absolute() && y.abs() > tolerance.absolute())
}
