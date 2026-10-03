//! Mirror option editing and three-point picking share the transform transaction.
use super::*;
use viboceros_command::mirror::MirrorPlaneOption;

impl VibocerosApp {
    pub(super) fn try_continue_mirror_option(&mut self, input: &str) -> bool {
        let Some(command) = self.active_command else {
            return false;
        };
        if !matches!(
            command,
            InteractiveCommand::Mirror { .. } | InteractiveCommand::MirrorThreePoint { .. }
        ) || self.transform_session.is_none()
            || self.plane_prompt.is_some()
        {
            return false;
        }
        let Some(option) = MirrorPlaneOption::from_token(input.trim()) else {
            return false;
        };
        if !matches!(command, InteractiveCommand::Mirror { start: None }) {
            self.push_log("Error: choose the mirror plane option before picking its start".into());
        } else if option == MirrorPlaneOption::ThreePoint {
            let next = InteractiveCommand::MirrorThreePoint { points: [None; 2] };
            self.active_command = Some(next);
            self.push_log(next.prompt().into());
        } else {
            self.apply_transform_step(&format!("Mirror {}", option.token().unwrap()), command);
        }
        self.command_input.clear();
        true
    }

    pub(super) fn accept_mirror_three_point(
        &mut self,
        points: [Option<Point3>; 2],
        point: Point3,
    ) -> bool {
        let next = match points {
            [None, _] => InteractiveCommand::MirrorThreePoint {
                points: [Some(point), None],
            },
            [Some(origin), None] => {
                if let Err(error) = origin
                    .vector_to(point)
                    .and_then(|v| v.normalized(self.document.tolerance()))
                {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
                InteractiveCommand::MirrorThreePoint {
                    points: [Some(origin), Some(point)],
                }
            }
            [Some(origin), Some(x)] => {
                if let Err(error) = viboceros_geometry::Frame3::try_from_points(
                    origin,
                    x,
                    point,
                    self.document.tolerance(),
                ) {
                    // The third point is accepted by GetPoint; an invalid
                    // resulting plane then ends native Mirror with Failure.
                    self.push_log(format!("Error: {error}"));
                    self.cancel_interactive_command(false);
                    return true;
                }
                return self.apply_transform_step(
                    &format!(
                        "Mirror 3Point {} {} {}",
                        format_model_point(origin),
                        format_model_point(x),
                        format_model_point(point)
                    ),
                    InteractiveCommand::MirrorThreePoint { points },
                );
            }
        };
        self.active_command = Some(next);
        self.push_log(next.prompt().into());
        true
    }
}
