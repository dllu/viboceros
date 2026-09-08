//! Route typed and picked points through the same interactive command state.

use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(super) fn try_continue_point_input(&mut self, input: &str) -> bool {
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
        {
            return false;
        }
        let Some(parsed) = PointInput::parse(input) else {
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
                self.accept_drafting_point(point);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn accept_drafting_point(&mut self, point: Point3) -> bool {
        let plane = self.viewports[self.active_viewport].construction_plane();
        if self.apply_drafting_point(point) {
            if self.active_command.is_some() {
                self.drafting_plane.get_or_insert(plane);
            }
            self.last_point = Some(point);
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
