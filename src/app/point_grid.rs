//! PointGrid picking; construction and transactional validation live in the command crate.
use super::*;
use viboceros_command::PointGridOptions;

impl VibocerosApp {
    pub(super) fn try_continue_point_grid_height(&mut self, input: &str) -> bool {
        if !matches!(
            self.active_command,
            Some(InteractiveCommand::PointGrid {
                base: Some(_),
                opposite: Some(_),
                ..
            })
        ) {
            return false;
        }
        if input.is_empty() {
            self.finish_point_grid(None);
            return true;
        }
        if let Ok(height) = input.parse::<f64>() {
            self.finish_point_grid(Some(height));
            return true;
        }
        false
    }

    pub(super) fn apply_point_grid_point(
        &mut self,
        plane: Frame3,
        base: Option<Point3>,
        opposite: Option<Point3>,
        options: PointGridOptions,
        point: Point3,
    ) -> bool {
        let command = match (base, opposite) {
            (None, None) => InteractiveCommand::PointGrid {
                base: Some(point),
                opposite: None,
                options,
            },
            (Some(base), None) => {
                if !plane
                    .with_origin(base)
                    .coordinates_of(point)
                    .is_ok_and(|[x, y, _]| x != 0.0 && y != 0.0)
                {
                    self.push_log(
                        "Error: point grid base must have nonzero finite width and depth"
                            .to_owned(),
                    );
                    return false;
                }
                InteractiveCommand::PointGrid {
                    base: Some(base),
                    opposite: Some(point),
                    options,
                }
            }
            (Some(base), Some(_)) => {
                match plane.with_origin(base).coordinates_of(point) {
                    Ok([_, _, height]) => return self.finish_point_grid(Some(height)),
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                return false;
            }
            (None, Some(_)) => unreachable!("opposite corner requires a base"),
        };
        self.active_command = Some(command);
        self.push_log(command.prompt().to_owned());
        true
    }

    fn finish_point_grid(&mut self, height: Option<f64>) -> bool {
        let Some(
            command @ InteractiveCommand::PointGrid {
                base: Some(base),
                opposite: Some(opposite),
                options,
            },
        ) = self.active_command
        else {
            return false;
        };
        let plane = self.drafting_plane;
        let height = height.map_or_else(String::new, |height| format!(" {height}"));
        let input = format!(
            "PointGrid {} {}{height}{options}",
            format_model_point(base),
            format_model_point(opposite)
        );
        // Clear the state before dispatch so the captured construction plane is
        // used even if the final point comes from a different viewport.
        self.active_command = None;
        if self.try_execute_command(&input) {
            self.command_input.clear();
            true
        } else {
            self.active_command = Some(command);
            self.drafting_plane = plane;
            false
        }
    }
}
