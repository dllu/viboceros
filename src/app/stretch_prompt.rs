//! Stretch point phases, numeric factors and lengths, and repeated copies.
use super::*;
use viboceros_command::stretch::Target;
use viboceros_document::ControlPointId;
use viboceros_geometry::Real;

pub(super) struct StretchSession {
    sources: Vec<ObjectId>,
    grips: Vec<ControlPointId>,
    postselected: bool,
    option: Option<&'static str>,
    group: viboceros_document::HistoryGroup,
    placed: bool,
    preview_point: Option<Point3>,
    preview_cache: std::cell::RefCell<crate::viewport::StretchPreviewCache>,
}
impl StretchSession {
    pub(super) fn preview(
        &self,
        command: Option<InteractiveCommand>,
    ) -> Option<crate::viewport::StretchPreview<'_>> {
        if self.option.is_some() {
            return None;
        }
        let InteractiveCommand::Stretch {
            points: [Some(start), Some(end)],
            options,
            ..
        } = command?
        else {
            return None;
        };
        Some(crate::viewport::StretchPreview {
            sources: &self.sources,
            grips: &self.grips,
            start,
            end,
            options,
            last_point: self.preview_point,
            cache: &self.preview_cache,
        })
    }
}
impl VibocerosApp {
    pub(super) fn stretch_preview(&self) -> Option<crate::viewport::StretchPreview<'_>> {
        self.stretch_session.as_ref()?.preview(self.active_command)
    }
    pub(super) fn update_stretch_preview(&mut self, point: Option<Point3>) -> bool {
        if self.stretch_preview().is_none() {
            return false;
        }
        let Some(session) = self.stretch_session.as_mut() else {
            return false;
        };
        if session.preview_point == point {
            return false;
        }
        session.preview_point = point;
        true
    }
    pub(super) fn start_stretch_session(&mut self, picked: Option<Vec<ObjectId>>) -> bool {
        let group = match self.document.begin_history_group("Stretch") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        self.stretch_session = Some(StretchSession {
            postselected: picked.is_some(),
            sources: picked.unwrap_or_else(|| self.document.selected_object_ids().collect()),
            grips: self
                .document
                .selected_control_points()
                .map(|(id, _)| id)
                .collect(),
            option: None,
            group,
            placed: false,
            preview_point: None,
            preview_cache: Default::default(),
        });
        true
    }
    pub(super) fn finish_stretch_session(&mut self, _completed: bool) {
        let Some(session) = self.stretch_session.take() else {
            return;
        };
        if !session.placed {
            return;
        }
        if let Some(InteractiveCommand::Stretch { options, .. }) = self.active_command {
            self.commands.complete_copy_options("Stretch", options.copy);
        }
    }
    pub(super) fn try_continue_stretch(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Stretch {
            points,
            factor,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = self.stretch_session.as_mut() else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel")
            || ((input.is_empty() || word.eq_ignore_ascii_case("Enter"))
                && session.placed
                && session.option.is_none())
        {
            self.finish_stretch_session(!word.eq_ignore_ascii_case("Cancel"));
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish Stretch before using Undo".into());
            self.command_input.clear();
            return true;
        }
        if let Some(name) = session.option.filter(|name| *name != "Length") {
            match options.accept(&format!("{name}={input}")) {
                Ok(true) => {
                    session.option = None;
                    self.active_command = Some(InteractiveCommand::Stretch {
                        points,
                        factor,
                        options,
                    });
                }
                Ok(false) => {}
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        if let Some(name) = ["Copy", "Rigid"]
            .into_iter()
            .find(|name| word.eq_ignore_ascii_case(name))
        {
            session.option = Some(name);
            self.push_log(format!("{name}: Yes or No"));
            self.command_input.clear();
            return true;
        }
        if input.contains('=') {
            if let Some((name, value)) = input.split_once('=')
                && name
                    .trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Length")
                && points[1].is_some()
            {
                match self.parse_stretch_scalar(value) {
                    Ok(v) => {
                        self.place_stretch(Target::Length(v));
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                };
                self.command_input.clear();
                return true;
            }
            match options.accept(input) {
                Ok(true) => {
                    self.active_command = Some(InteractiveCommand::Stretch {
                        points,
                        factor,
                        options,
                    });
                }
                Ok(false) => return false,
                Err(e) => self.push_log(format!("Error: {e}")),
            };
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Length") && points[1].is_some() {
            session.option = Some("Length");
            session.preview_point = None;
            self.push_log("Stretch: enter the new axis length".into());
            self.command_input.clear();
            return true;
        }
        if points[0].is_none() {
            return false;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if points[1].is_none() && factor.is_none() {
                self.active_command = Some(InteractiveCommand::Stretch {
                    points,
                    factor: Some(self.commands.stretch_factor_default()),
                    options,
                });
                self.push_log("Stretch: pick the stretch direction".into());
            } else {
                self.finish_stretch_session(false);
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        let scalar = if input.contains(',') || input.starts_with(['w', 'r', '@', '<']) {
            None
        } else {
            self.parse_stretch_scalar(input).ok()
        };
        if let Some(value) = scalar {
            if points[1].is_none() {
                self.active_command = Some(InteractiveCommand::Stretch {
                    points,
                    factor: Some(value),
                    options,
                });
                self.push_log("Stretch: pick the stretch direction".into());
            } else if self.stretch_session.as_ref().unwrap().option == Some("Length") {
                self.place_stretch(Target::Length(value));
            } else {
                self.place_stretch(Target::Factor(value));
            }
            self.command_input.clear();
            return true;
        }
        false
    }
    fn parse_stretch_scalar(&self, input: &str) -> Result<Real, String> {
        if let Ok(v) = input.parse::<Real>() {
            return if v.is_finite() {
                Ok(v)
            } else {
                Err("Stretch requires a finite value".into())
            };
        }
        match viboceros_drafting::PointConstraintInput::parse_with_units(
            input,
            self.document.units(),
        ) {
            Some(Ok(viboceros_drafting::PointConstraintInput::Distance(v))) => Ok(v),
            Some(Err(e)) => Err(e.to_string()),
            _ => Err("Enter a stretch factor, length or point".into()),
        }
    }
    pub(super) fn accept_stretch_point(&mut self, point: Point3) -> bool {
        let Some(InteractiveCommand::Stretch {
            mut points,
            factor,
            options,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = self.stretch_session.as_mut() else {
            return false;
        };
        if session.option.is_some_and(|name| name != "Length") {
            return false;
        }
        let count = points.iter().flatten().count();
        if count < 2 {
            if count == 1
                && let Err(e) = Target::Factor(1.).morph(points[0].unwrap(), point)
            {
                self.push_log(format!("Error: {e}"));
                return false;
            }
            points[count] = Some(point);
            self.active_command = Some(InteractiveCommand::Stretch {
                points,
                factor,
                options,
            });
            if count == 1
                && let Some(value) = factor
            {
                return self.place_stretch(Target::Factor(value));
            }
            self.push_log(self.active_command.unwrap().prompt().into());
            return true;
        }
        if session.option == Some("Length") {
            self.push_log("Length requires a numeric value".into());
            false
        } else {
            self.place_stretch(Target::Point(point))
        }
    }

    fn place_stretch(&mut self, target: Target) -> bool {
        let Some(InteractiveCommand::Stretch {
            points: [Some(start), Some(end)],
            options,
            ..
        }) = self.active_command
        else {
            return false;
        };
        let session = self.stretch_session.as_mut().unwrap();
        let sources = session
            .sources
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let grips = transform_sources::grip_argument(&session.grips, session.postselected);
        let input = format!(
            "Stretch {} {} {} {} {}={sources} {grips}",
            format_model_point(start),
            format_model_point(end),
            target.argument(),
            options.arguments(),
            if session.postselected {
                "PickedSources"
            } else {
                "Sources"
            }
        );
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            Default::default(),
            &mut session.group,
        ) {
            Ok(message) => {
                session.placed = true;
                session.preview_point = None;
                session.option = None;
                self.active_command = Some(InteractiveCommand::Stretch {
                    points: [Some(start), Some(end)],
                    factor: None,
                    options,
                });
                self.push_log(message);
                if options.copy {
                    self.push_log("Stretch: enter another factor or target; Enter finishes, Esc keeps accepted copies".into());
                } else {
                    self.finish_stretch_session(true);
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
