//! Interactive box state; geometry and transactions remain in the command layer.
use super::*;

impl VibocerosApp {
    pub(super) fn apply_box_point(
        &mut self,
        plane: Frame3,
        base: Option<Point3>,
        opposite: Option<Point3>,
        point: Point3,
    ) -> bool {
        let command = match (base, opposite) {
            (None, None) => InteractiveCommand::Box {
                base: Some(point),
                opposite: None,
            },
            (Some(base), None) => {
                if !plane_rectangle_exceeds_tolerance(plane, base, point, self.document.tolerance())
                {
                    self.push_log(
                        "Error: box base width and depth must exceed model tolerance".to_owned(),
                    );
                    return false;
                }
                let frame = plane.with_origin(base);
                let opposite = frame
                    .coordinates_of(point)
                    .and_then(|[x, y, _]| frame.point_at([x, y, 0.0]));
                let Ok(opposite) = opposite else {
                    self.push_log("Error: box base corner is not finite".to_owned());
                    return false;
                };
                InteractiveCommand::Box {
                    base: Some(base),
                    opposite: Some(opposite),
                }
            }
            (Some(base), Some(opposite)) => {
                if !plane
                    .with_origin(base)
                    .coordinates_of(point)
                    .is_ok_and(|p| p[2].abs() > self.document.tolerance().absolute())
                {
                    self.push_log("Error: box height must exceed model tolerance".to_owned());
                    return false;
                }
                self.active_command = None;
                self.execute_command(&format!(
                    "Box {} {} {}",
                    format_model_point(base),
                    format_model_point(opposite),
                    format_model_point(point)
                ));
                return true;
            }
            (None, Some(_)) => unreachable!("opposite corner requires a base"),
        };
        self.active_command = Some(command);
        self.push_log(command.prompt().to_owned());
        true
    }
}
