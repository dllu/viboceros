//! Bend source selection, spine input, option prompts and repeated copies.
use super::*;
use viboceros_command::bend::BendOptions;

pub(super) struct BendSession {
    sources: Vec<ObjectId>,
    postselected: bool,
    option: Option<&'static str>,
    group: viboceros_document::HistoryGroup,
    placed: bool,
    initial_options: BendOptions,
    preserve_available: bool,
}

impl VibocerosApp {
    pub(super) fn start_bend_session(&mut self, picked: Option<Vec<ObjectId>>) -> bool {
        let group = match self.document.begin_history_group("Bend") {
            Ok(group) => group,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        };
        let postselected = picked.is_some();
        let sources = picked.unwrap_or_else(|| self.document.selected_object_ids().collect());
        let preserve_available = sources.iter().any(|id| {
            self.document.object(*id).is_some_and(|object| {
                !matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() > 1)
            })
        });
        self.bend_session = Some(BendSession {
            sources,
            postselected,
            option: None,
            group,
            placed: false,
            initial_options: self.commands.bend_options_default(),
            preserve_available,
        });
        true
    }

    pub(super) fn finish_bend_session(&mut self, completed: bool) {
        let Some(session) = self.bend_session.take() else {
            return;
        };
        if !session.placed {
            return;
        }
        if !completed
            && !session.postselected
            && let Err(e) = self
                .document
                .retain_history_group_selection_on_replay(&session.group)
        {
            self.push_log(format!("Error: {e}"));
        }
        let options = if completed {
            match self.active_command {
                Some(InteractiveCommand::Bend { options, .. }) => options,
                _ => session.initial_options,
            }
        } else {
            session.initial_options
        };
        self.commands.remember_bend_completion_options(options);
        self.commands.complete_copy_options("Bend", options.copy);
    }

    pub(super) fn try_continue_bend(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Bend {
            points,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = self.bend_session.as_mut() else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel")
            || ((input.is_empty() || word.eq_ignore_ascii_case("Enter"))
                && session.placed
                && session.option.is_none())
        {
            if !word.eq_ignore_ascii_case("Cancel") {
                self.finish_bend_session(true);
            }
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if let Some(name) = session.option.take() {
            let value =
                if name == "Angle" && (input.is_empty() || word.eq_ignore_ascii_case("Enter")) {
                    self.commands
                        .transform_scalar_default("Bend")
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                } else {
                    input.to_owned()
                };
            match options.update(&format!("{name}={value}")) {
                Ok(()) => {
                    self.commands.remember_bend_prompt_option(options, name);
                    self.active_command = Some(InteractiveCommand::Bend { points, options });
                    self.push_log(options.command_options());
                }
                Err(e) => {
                    self.bend_session.as_mut().unwrap().option = Some(name);
                    self.push_log(format!("Error: {e}"));
                }
            }
            self.command_input.clear();
            return true;
        }
        if !matches!(points, [Some(_), Some(_)]) {
            return false;
        }
        if let Some(name) = [
            "Copy",
            "Rigid",
            "LimitToSpine",
            "Symmetric",
            "PreserveStructure",
            "NonAttenuated",
            "Angle",
        ]
        .into_iter()
        .find(|name| word.eq_ignore_ascii_case(name))
        {
            if name == "PreserveStructure" && !session.preserve_available {
                self.push_log("PreserveStructure is unavailable for polysurfaces".into());
            } else if name == "LimitToSpine" && options.angle.is_some_and(|a| a != 0.) {
                self.push_log(
                    "LimitToSpine is available in through-point mode; set Angle to 0".into(),
                );
            } else {
                session.option = Some(name);
                if name == "Angle" {
                    self.push_log(match self.commands.transform_scalar_default("Bend") {
                        Some(value) => format!("Bend Angle <{value}>"),
                        None => "Bend Angle".into(),
                    });
                } else {
                    self.push_log(format!("{name}: Yes or No"));
                }
            }
            self.command_input.clear();
            return true;
        }
        if input.contains('=') && !input.contains(',') {
            if input.split_once('=').is_some_and(|(name, _)| {
                name.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("PreserveStructure")
            }) && !session.preserve_available
            {
                self.push_log("PreserveStructure is unavailable for polysurfaces".into());
                self.command_input.clear();
                return true;
            }
            match options.update(input) {
                Ok(()) => {
                    self.commands
                        .remember_bend_prompt_option(options, input.split_once('=').unwrap().0);
                    self.active_command = Some(InteractiveCommand::Bend { points, options });
                    self.push_log(options.command_options());
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish Bend before using Undo".into());
            self.command_input.clear();
            return true;
        }
        false
    }

    pub(super) fn accept_bend_point(&mut self, point: Point3) -> bool {
        let Some(InteractiveCommand::Bend {
            mut points,
            options,
        }) = self.active_command
        else {
            return false;
        };
        if self
            .bend_session
            .as_ref()
            .is_some_and(|s| s.option.is_some())
        {
            return false;
        }
        let count = points.iter().flatten().count();
        if count == 1 && points[0].is_some_and(|p| p.is_near(point, self.document.tolerance())) {
            self.push_log("Error: bend spine points must differ".into());
            return false;
        }
        if count < 2 {
            points[count] = Some(point);
            let command = InteractiveCommand::Bend { points, options };
            self.active_command = Some(command);
            self.push_log(command.prompt().into());
            return true;
        }
        self.finish_bend(points[0].unwrap(), points[1].unwrap(), point, options)
    }

    fn finish_bend(
        &mut self,
        start: Point3,
        end: Point3,
        through: Point3,
        options: BendOptions,
    ) -> bool {
        let Some(session) = self.bend_session.as_ref() else {
            return false;
        };
        let sources = session
            .sources
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let input = format!(
            "Bend {} {} {} {} {}={sources}",
            format_model_point(start),
            format_model_point(end),
            format_model_point(through),
            options.command_options(),
            if session.postselected {
                "PickedSources"
            } else {
                "Sources"
            }
        );
        self.push_log(format!("> {input}"));
        let session = self.bend_session.as_mut().unwrap();
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            viboceros_command::CommandContext::default(),
            &mut session.group,
        ) {
            Ok(message) => {
                session.placed = true;
                if options.copy {
                    self.commands
                        .remember_bend_completion_options(session.initial_options);
                    self.commands
                        .complete_copy_options("Bend", session.initial_options.copy);
                }
                self.push_log(message);
                if options.copy {
                    self.active_command = Some(InteractiveCommand::Bend {
                        points: [Some(start), Some(end)],
                        options,
                    });
                    self.push_log("Bend: pick another through point; Enter finishes, Esc keeps accepted copies".into());
                } else {
                    self.finish_bend_session(true);
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
