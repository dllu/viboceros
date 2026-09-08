//! Route typed and picked points through the same interactive command state.

use super::*;
use viboceros_drafting::PointInput;

impl VibocerosApp {
    pub(super) fn try_continue_point_input(&mut self, input: &str) -> bool {
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Close")
            && self.active_command == Some(InteractiveCommand::Polyline)
        {
            self.push_log(format!("> {input}"));
            if self.curve_points.len() < 3 {
                self.push_log("Error: Close requires at least three polyline points".to_owned());
                return true;
            }
            let first = self.curve_points[0];
            let last = *self.curve_points.last().expect("at least three points");
            let append = first != last;
            if append
                && !first
                    .distance_to(last)
                    .is_ok_and(|length| length > self.document.tolerance().absolute())
            {
                self.push_log(
                    "Error: closing segment length must be finite and greater than tolerance"
                        .to_owned(),
                );
                return true;
            }
            if append {
                self.curve_points.push(first);
            }
            self.finish_interactive_curve();
            if self.active_command.is_none() {
                self.command_input.clear();
            } else if append {
                // Failed completion restores its input points. Remove the
                // implicit closing vertex so the user's original draft survives.
                self.curve_points.pop();
            }
            return true;
        }
        // A prompt option takes precedence over the document-level command.
        // Match the whole entry so malformed command arguments are not ignored.
        if input
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Undo")
            && matches!(
                self.active_command,
                Some(
                    InteractiveCommand::Polyline
                        | InteractiveCommand::Curve { .. }
                        | InteractiveCommand::InterpCrv
                )
            )
        {
            self.push_log(format!("> {input}"));
            if self.curve_points.pop().is_some() {
                self.last_point = self.curve_points.last().copied();
                if self.curve_points.is_empty() {
                    self.drafting_plane = None;
                }
            } else {
                self.push_log("No draft points to undo".to_owned());
            }
            self.command_input.clear();
            return true;
        }
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
