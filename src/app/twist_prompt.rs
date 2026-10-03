//! Twist's source, axis and scalar/reference input lifecycle.
use super::*;

pub(super) struct TwistSession {
    sources: Vec<ObjectId>,
    postselected: bool,
    option: Option<&'static str>,
    group: viboceros_document::HistoryGroup,
    placed: bool,
}
impl VibocerosApp {
    pub(super) fn start_twist_session(&mut self, picked: Option<Vec<ObjectId>>) -> bool {
        let group = match self.document.begin_history_group("Twist") {
            Ok(group) => group,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let postselected = picked.is_some();
        let sources = picked.unwrap_or_else(|| {
            self.document
                .objects()
                .filter(|o| self.document.is_selected(o.id()))
                .map(|o| o.id())
                .collect()
        });
        self.twist_session = Some(TwistSession {
            sources,
            postselected,
            option: None,
            group,
            placed: false,
        });
        true
    }
    pub(super) fn try_continue_twist(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Twist {
            points,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = self.twist_session.as_mut() else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel")
            || ((input.is_empty() || word.eq_ignore_ascii_case("Enter")) && session.placed)
        {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if let Some(name) = session.option.take() {
            match options.update(&format!("{name}={input}")) {
                Ok(()) => {
                    self.active_command = Some(InteractiveCommand::Twist { points, options });
                    self.push_log(options.command_options());
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            };
            self.command_input.clear();
            return true;
        }
        if let Some(name) = ["Copy", "Rigid", "Infinite", "PreserveStructure"]
            .into_iter()
            .find(|name| word.eq_ignore_ascii_case(name))
        {
            session.option = Some(name);
            self.push_log(format!("{name}: Yes or No"));
            self.command_input.clear();
            return true;
        }
        if input.contains('=') && !input.contains(',') {
            match options.update(input) {
                Ok(()) => {
                    self.active_command = Some(InteractiveCommand::Twist { points, options });
                    self.push_log(options.command_options());
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            };
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish Twist before using Undo".into());
            self.command_input.clear();
            return true;
        }
        if let [Some(start), Some(end), None] = points {
            let degrees = if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
                self.commands.transform_scalar_default("Twist")
            } else {
                input.parse::<f64>().ok().filter(|v| v.is_finite())
            };
            if degrees.is_none() && input.parse::<f64>().is_ok() {
                self.push_log("Error: twist angle must be finite".into());
                self.command_input.clear();
                return true;
            }
            if let Some(degrees) = degrees {
                self.finish_twist(start, end, degrees, options);
                self.command_input.clear();
                return true;
            }
        }
        false
    }
    pub(super) fn accept_twist_point(&mut self, point: Point3) -> bool {
        let Some(InteractiveCommand::Twist {
            mut points,
            options,
        }) = self.active_command
        else {
            return false;
        };
        let count = points.iter().flatten().count();
        if count == 1
            && points[0].is_some_and(|a| {
                !a.distance_to(point)
                    .is_ok_and(|d| d > self.document.tolerance().absolute())
            })
        {
            self.push_log("Error: twist axis points must differ".into());
            return false;
        }
        if count >= 2
            && point_is_near_axis(
                points[0].unwrap(),
                points[1].unwrap(),
                point,
                self.document.tolerance(),
            )
        {
            self.push_log("Error: twist reference points must lie off the axis".into());
            return false;
        }
        if count < 3 {
            points[count] = Some(point);
            if count == 1 {
                self.drafting_plane = points[0].and_then(|start| {
                    start.vector_to(point).ok().and_then(|normal| {
                        Frame3::try_from_normal(start, normal, self.document.tolerance()).ok()
                    })
                });
            }
            let command = InteractiveCommand::Twist { points, options };
            self.active_command = Some(command);
            self.push_log(command.prompt().into());
            true
        } else {
            match viboceros_command::twist::reference_angle(
                points[0].unwrap(),
                points[1].unwrap(),
                points[2].unwrap(),
                point,
                self.document.tolerance(),
            ) {
                Ok(degrees) => {
                    self.finish_twist(points[0].unwrap(), points[1].unwrap(), degrees, options)
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    false
                }
            }
        }
    }
    fn finish_twist(
        &mut self,
        start: Point3,
        end: Point3,
        degrees: f64,
        options: viboceros_command::twist::TwistOptions,
    ) -> bool {
        let Some(session) = self.twist_session.as_ref() else {
            return false;
        };
        let sources = session
            .sources
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let input = format!(
            "Twist {} {} {degrees} {} {}={sources}",
            format_model_point(start),
            format_model_point(end),
            options.command_options(),
            if session.postselected {
                "PickedSources"
            } else {
                "Sources"
            }
        );
        self.push_log(format!("> {input}"));
        let session = self.twist_session.as_mut().unwrap();
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            viboceros_command::CommandContext::default(),
            &mut session.group,
        ) {
            Ok(message) => {
                session.placed = true;
                self.push_log(message);
                if options.copy {
                    let next = InteractiveCommand::Twist {
                        points: [Some(start), Some(end), None],
                        options,
                    };
                    self.active_command = Some(next);
                    self.push_log("Twist: enter another angle or reference point; Enter finishes, Esc keeps accepted copies".into());
                } else {
                    self.cancel_interactive_command(false);
                }
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
}
