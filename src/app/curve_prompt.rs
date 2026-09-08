//! Options for collected curve drafts, independent of document history.

use super::*;

impl VibocerosApp {
    /// A closing point is a completion gesture, not another interpolation point.
    /// Private Rhino probes establish an inclusive Euclidean ON_SQRT_EPSILON
    /// threshold (the OpenNURBS decimal constant, not f64::EPSILON.sqrt()).
    pub(super) fn try_auto_close_interpolation(
        &mut self,
        point: Point3,
        options: viboceros_geometry::CurveInterpolationOptions,
    ) -> Option<bool> {
        if self.curve_points.len() < 2
            || !self.curve_points[0]
                .distance_to(point)
                .is_ok_and(|distance| distance <= 1.490116119385e-8)
        {
            return None;
        }
        let original = self.active_command;
        let closure = if self.curve_points.len() == 2
            || options.closure() == viboceros_geometry::InterpolatedCurveClosure::Sharp
        {
            viboceros_geometry::InterpolatedCurveClosure::Sharp
        } else {
            viboceros_geometry::InterpolatedCurveClosure::Smooth
        };
        self.active_command = Some(InteractiveCommand::InterpCrv {
            options: options.with_closure(closure),
        });
        self.finish_interactive_curve();
        if self.active_command.is_some() {
            self.active_command = original;
            Some(false)
        } else {
            Some(true)
        }
    }

    pub(super) fn curve_draft_preview(
        &mut self,
    ) -> Option<std::sync::Arc<viboceros_geometry::NurbsCurve>> {
        let settings = match (self.plane_prompt.is_some(), self.active_command) {
            (false, Some(InteractiveCommand::Curve { degree, closure })) => Some(
                curve_preview::CurvePreviewSettings::Control(degree, closure),
            ),
            (false, Some(InteractiveCommand::InterpCrv { options })) => {
                Some(curve_preview::CurvePreviewSettings::Interpolated(
                    options,
                    self.document.tolerance(),
                ))
            }
            _ => None,
        };
        self.curve_preview.get(settings, &self.curve_points)
    }

    pub(super) fn try_continue_curve_option(&mut self, input: &str) -> bool {
        let option = input.trim_start_matches(['_', '-']);
        if let Some(InteractiveCommand::InterpCrv { options }) = self.active_command
            && let Some((name, _)) = option.split_once('=')
            && ["Degree", "Knots", "Close", "StartTangent", "EndTangent"]
                .iter()
                .any(|known| name.eq_ignore_ascii_case(known))
        {
            self.push_log(format!("> {input}"));
            match update_interp_curve_options(
                options,
                &input.split_whitespace().collect::<Vec<_>>(),
            ) {
                Ok(options) => {
                    self.active_command = Some(InteractiveCommand::InterpCrv { options });
                    self.command_input.clear();
                    self.push_log(format!(
                        "InterpCrv settings: {}",
                        format_interp_curve_options(options)
                    ));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        if let Some(InteractiveCommand::Curve { degree, closure }) = self.active_command
            && let Some((name, value)) = option.split_once('=')
        {
            let updated = if name.eq_ignore_ascii_case("Degree") {
                parse_curve_degree(value)
                    .ok()
                    .map(|degree| InteractiveCommand::Curve { degree, closure })
            } else if name.eq_ignore_ascii_case("Close") {
                parse_curve_closure(value)
                    .ok()
                    .map(|closure| InteractiveCommand::Curve { degree, closure })
            } else {
                return false;
            };
            self.push_log(format!("> {input}"));
            if let Some(command) = updated {
                self.active_command = Some(command);
                self.command_input.clear();
                if let InteractiveCommand::Curve { degree, closure } = command {
                    self.push_log(format!("Curve settings: Degree={degree} Close={closure:?}"));
                }
            } else {
                self.push_log("Error: use Degree=integer or Close=Open|Smooth|Sharp".to_owned());
            }
            return true;
        }
        if let Some(command) = self.active_command.filter(|command| {
            matches!(
                command,
                InteractiveCommand::Curve { .. } | InteractiveCommand::InterpCrv { .. }
            )
        }) && (option.eq_ignore_ascii_case("Close") || option.eq_ignore_ascii_case("Sharp"))
        {
            self.push_log(format!("> {input}"));
            let sharp = option.eq_ignore_ascii_case("Sharp");
            let closed = match command {
                InteractiveCommand::Curve { degree, .. } => InteractiveCommand::Curve {
                    degree,
                    closure: if sharp {
                        ControlPointCurveClosure::Sharp
                    } else {
                        ControlPointCurveClosure::Smooth
                    },
                },
                InteractiveCommand::InterpCrv { options } => InteractiveCommand::InterpCrv {
                    options: options.with_closure(if sharp {
                        viboceros_geometry::InterpolatedCurveClosure::Sharp
                    } else {
                        viboceros_geometry::InterpolatedCurveClosure::Smooth
                    }),
                },
                _ => unreachable!("only curve drafts reach closure options"),
            };
            self.active_command = Some(closed);
            self.finish_interactive_curve();
            if self.active_command.is_some() {
                // Closing is an attempted completion, not a persistent option
                // change: a failed attempt must leave the original draft intact.
                self.active_command = Some(command);
            } else {
                self.command_input.clear();
            }
            return true;
        }
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
                        | InteractiveCommand::InterpCrv { .. }
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
        false
    }
}
