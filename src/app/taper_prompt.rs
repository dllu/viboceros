//! Taper source selection, axis, radial distances and repeated Copy placements.
use super::*;
use viboceros_command::taper::{TaperDistance, TaperOptions, point_morph};

pub(super) struct TaperSession {
    sources: Vec<ObjectId>,
    postselected: bool,
    option: Option<&'static str>,
    group: viboceros_document::HistoryGroup,
    placed: bool,
    initial_options: TaperOptions,
    preserve_available: bool,
    context: Option<viboceros_command::CommandContext>,
    preview_point: Option<Point3>,
    preview_cache: std::cell::RefCell<crate::viewport::TaperPreviewCache>,
}
impl TaperSession {
    pub(super) fn preview(
        &self,
        command: Option<InteractiveCommand>,
        cplane: Frame3,
    ) -> Option<crate::viewport::TaperPreview<'_>> {
        let InteractiveCommand::Taper {
            points: [Some(start), Some(end)],
            initial,
            options,
        } = command?
        else {
            return None;
        };
        Some(crate::viewport::TaperPreview {
            sources: &self.sources,
            start,
            end,
            initial,
            options,
            cplane: self.context.map_or(cplane, |c| c.construction_plane),
            last_point: self.preview_point,
            cache: &self.preview_cache,
        })
    }
}
impl VibocerosApp {
    pub(super) fn taper_preview(&self) -> Option<crate::viewport::TaperPreview<'_>> {
        self.taper_session.as_ref()?.preview(
            self.active_command,
            self.viewports[self.active_viewport].construction_plane(),
        )
    }
    pub(super) fn update_taper_preview(&mut self, point: Option<Point3>) -> bool {
        if self.taper_preview().is_none() {
            return false;
        }
        let session = self.taper_session.as_mut().unwrap();
        if session.preview_point == point {
            return false;
        }
        session.preview_point = point;
        true
    }
    pub(super) fn start_taper_session(&mut self, picked: Option<Vec<ObjectId>>) -> bool {
        let group = match self.document.begin_history_group("Taper") {
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
        let preserve_available = sources.iter().any(|id| {
            self.document
                .object(*id)
                .is_some_and(|o| !matches!(o.geometry(),Geometry::Brep(b) if b.faces().len()>1))
        });
        self.taper_session = Some(TaperSession {
            sources,
            postselected,
            option: None,
            group,
            placed: false,
            initial_options: self.commands.taper_options_default(),
            preserve_available,
            context: None,
            preview_point: None,
            preview_cache: Default::default(),
        });
        true
    }
    pub(super) fn finish_taper_session(&mut self, completed: bool) {
        let Some(session) = self.taper_session.take() else {
            return;
        };
        if !session.placed {
            return;
        }
        let mut options = if completed {
            match self.active_command {
                Some(InteractiveCommand::Taper { options, .. }) => options,
                _ => session.initial_options,
            }
        } else {
            session.initial_options
        };
        if !session.preserve_available {
            options.preserve_structure = session.initial_options.preserve_structure;
        }
        self.commands.remember_taper_completion_options(options);
        self.commands.complete_copy_options("Taper", options.copy);
    }
    pub(super) fn try_continue_taper(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Taper {
            points,
            initial,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = self.taper_session.as_mut() else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel")
            || ((input.is_empty() || word.eq_ignore_ascii_case("Enter"))
                && session.placed
                && session.option.is_none())
        {
            self.finish_taper_session(!word.eq_ignore_ascii_case("Cancel"));
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if let Some(name) = session.option.take() {
            match options.update(&format!("{name}={input}")) {
                Ok(()) => {
                    self.active_command = Some(InteractiveCommand::Taper {
                        points,
                        initial,
                        options,
                    });
                    self.push_log(options.command_options());
                }
                Err(e) => {
                    self.taper_session.as_mut().unwrap().option = Some(name);
                    self.push_log(format!("Error: {e}"));
                }
            }
            self.command_input.clear();
            return true;
        }
        if !matches!(points, [Some(_), Some(_)]) {
            return false;
        }
        if let Some(name) = ["Copy", "Rigid", "Flat", "Infinite", "PreserveStructure"]
            .into_iter()
            .find(|name| word.eq_ignore_ascii_case(name))
        {
            if name == "PreserveStructure" && !session.preserve_available {
                self.push_log("PreserveStructure is unavailable for polysurfaces".into());
            } else {
                session.option = Some(name);
                self.push_log(format!("{name}: Yes or No"));
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
            } else {
                match options.update(input) {
                    Ok(()) => {
                        self.active_command = Some(InteractiveCommand::Taper {
                            points,
                            initial,
                            options,
                        });
                        self.push_log(options.command_options());
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish Taper before using Undo".into());
            self.command_input.clear();
            return true;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            self.push_log("Taper requires a nonzero distance".into());
            self.command_input.clear();
            return true;
        }
        // Scalars are radii at these prompts, before the common point-input
        // parser could interpret them as a locked drafting distance.
        let value = if let Ok(v) = input.parse::<f64>() {
            Some(Ok(v))
        } else {
            match viboceros_drafting::PointConstraintInput::parse_with_units(
                input,
                self.document.units(),
            ) {
                Some(Ok(viboceros_drafting::PointConstraintInput::Distance(v))) => Some(Ok(v)),
                Some(Err(e)) => Some(Err(e.to_string())),
                _ => None,
            }
        };
        if let Some(value) = value {
            match value {
                Ok(v) => {
                    self.accept_taper_distance(TaperDistance::Number(v));
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
    pub(super) fn accept_taper_point(&mut self, point: Point3) -> bool {
        let Some(InteractiveCommand::Taper {
            mut points,
            initial,
            options,
        }) = self.active_command
        else {
            return false;
        };
        if self
            .taper_session
            .as_ref()
            .is_some_and(|s| s.option.is_some())
        {
            return false;
        }
        let count = points.iter().flatten().count();
        if count == 1 {
            let context = viboceros_command::CommandContext {
                construction_plane: self.viewports[self.active_viewport].construction_plane(),
            };
            if let Err(e) = point_morph(
                points[0].unwrap(),
                point,
                TaperDistance::Number(1.),
                TaperDistance::Number(1.),
                TaperOptions::default(),
                context,
            ) {
                self.push_log(format!("Error: {e}"));
                return false;
            }
        }
        if count < 2 {
            points[count] = Some(point);
            let command = InteractiveCommand::Taper {
                points,
                initial,
                options,
            };
            self.active_command = Some(command);
            self.push_log(command.prompt().into());
            return true;
        }
        self.accept_taper_distance(TaperDistance::Point(point))
    }
    fn accept_taper_distance(&mut self, distance: TaperDistance) -> bool {
        let Some(InteractiveCommand::Taper {
            points: [Some(start), Some(end)],
            initial,
            options,
        }) = self.active_command
        else {
            return false;
        };
        let context = self
            .taper_session
            .as_ref()
            .and_then(|s| s.context)
            .unwrap_or(viboceros_command::CommandContext {
                construction_plane: self.viewports[self.active_viewport].construction_plane(),
            });
        if initial.is_none() {
            if let Err(e) = point_morph(start, end, distance, distance, options, context) {
                self.push_log(format!("Error: {e}"));
                return false;
            }
            self.taper_session.as_mut().unwrap().context = Some(context);
            self.taper_session.as_mut().unwrap().preview_point = None;
            self.active_command = Some(InteractiveCommand::Taper {
                points: [Some(start), Some(end)],
                initial: Some(distance),
                options,
            });
            self.push_log(self.active_command.unwrap().prompt().into());
            return true;
        }
        let session = self.taper_session.as_ref().unwrap();
        let sources = session
            .sources
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let input = format!(
            "Taper {} {} {} {} {} {}={sources}",
            format_model_point(start),
            format_model_point(end),
            initial.unwrap().command_argument(),
            distance.command_argument(),
            options.command_options(),
            if session.postselected {
                "PickedSources"
            } else {
                "Sources"
            }
        );
        self.push_log(format!("> {input}"));
        let session = self.taper_session.as_mut().unwrap();
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            context,
            &mut session.group,
        ) {
            Ok(message) => {
                session.placed = true;
                session.preview_point = None;
                if options.copy {
                    self.commands
                        .remember_taper_completion_options(session.initial_options);
                    self.commands
                        .complete_copy_options("Taper", session.initial_options.copy);
                }
                self.push_log(message);
                if options.copy {
                    self.push_log("Taper: enter another end distance; Enter finishes, Esc keeps accepted copies".into());
                } else {
                    self.finish_taper_session(true);
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
