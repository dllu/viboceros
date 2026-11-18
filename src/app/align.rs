//! Alignment mode/point phases stay outside document transactions.
use super::*;
use viboceros_command::align::{
    self as solver, AlignedPlacement, AlignmentLayout, AlignmentSources,
};
use viboceros_command::component_selection::ComponentPick;
use viboceros_command::placement::Placement;
use viboceros_command::{AlignmentMode, AlignmentOptions, CommandError};
mod plane;
mod preview;
use preview::Preview;

pub(super) struct Session {
    sources: AlignmentSources,
    components: Vec<ComponentPick>,
    prepared: Option<(AlignmentMode, Frame3, AlignmentLayout)>,
    preview: Option<Preview>,
    frame_choice: bool,
    rectangle: plane::RectangleGetter,
}

impl Session {
    pub(super) fn sources(&self) -> &AlignmentSources {
        &self.sources
    }

    pub(super) fn preview(&self) -> Option<&Placement> {
        self.preview
            .as_ref()?
            .placement
            .as_ref()
            .ok()
            .map(AlignedPlacement::preview)
    }

    pub(super) fn rectangle_preview(
        &self,
    ) -> Option<viboceros_command::rectangle_input::RectangleDefinition> {
        self.preview.as_ref()?.rectangle
    }

    fn placement(
        &mut self,
        document: &Document,
        options: AlignmentOptions,
        plane: Frame3,
    ) -> Result<AlignedPlacement, CommandError> {
        let mode = options
            .mode
            .ok_or(CommandError::Usage("Align requires an alignment mode"))?;
        let frame = if options.world {
            viboceros_command::CommandContext::default().construction_plane
        } else {
            plane
        };
        if !self
            .prepared
            .as_ref()
            .is_some_and(|(old_mode, old_frame, _)| *old_mode == mode && *old_frame == frame)
        {
            self.prepared = Some((mode, frame, self.sources.prepare(options, frame)?));
        }
        self.prepared
            .as_ref()
            .unwrap()
            .2
            .placement(document, options)
    }
}

impl VibocerosApp {
    fn alignment_components(&self) -> Vec<ComponentPick> {
        self.commands
            .component_selection()
            .live_picks(&self.document)
            .into_iter()
            .filter(|pick| solver::supports_component(pick.kind))
            .collect()
    }

    pub(super) fn selecting_alignment_sources(&self) -> bool {
        self.plane_prompt.is_none()
            && self.object_prompt.as_ref().is_some_and(|pending| {
                pending.description.command == "Align"
                    && pending.phase == object_selection::ObjectPromptPhase::Selecting
            })
    }

    pub(super) fn has_alignment_components(&self) -> bool {
        !self.alignment_components().is_empty()
    }

    pub(super) fn align_wants_point(&self) -> bool {
        self.align_session
            .as_ref()
            .is_none_or(|session| !session.frame_choice)
            && matches!(self.active_command, Some(InteractiveCommand::Align { options, .. })
                if options.mode.is_some() && options.mode != Some(AlignmentMode::ToCurve))
    }
}

impl VibocerosApp {
    pub(super) fn picking_alignment_curve(&self) -> bool {
        self.plane_prompt.is_none()
            && self.object_prompt.is_none()
            && matches!(
                self.active_command,
                Some(InteractiveCommand::Align {
                    options: AlignmentOptions {
                        mode: Some(viboceros_command::AlignmentMode::ToCurve),
                        ..
                    },
                    ..
                })
            )
    }

    pub(super) fn pick_alignment_curve(&mut self, id: Option<viboceros_document::ObjectId>) {
        let Some(id) = id else { return };
        if !self.document.is_object_selectable(id) {
            self.push_log("Error: alignment target is not selectable".into());
            return;
        }
        let Some(InteractiveCommand::Align { mut options, .. }) = self.active_command else {
            return;
        };
        options.curve = Some(id);
        self.execute_align(options);
    }

    pub(super) fn update_align_selection(
        &self,
        prompt: &viboceros_command::ObjectSelectionPrompt,
        input: &str,
    ) -> Result<viboceros_command::ObjectSelectionPrompt, viboceros_command::CommandError> {
        let usage = viboceros_command::CommandError::Usage(
            "alignment options only; finish object selection before entering points",
        );
        let line = prompt.command_line();
        let options = AlignmentOptions::default()
            .parse(&line.split_whitespace().skip(1).collect::<Vec<_>>())?
            .parse(&input.split_whitespace().collect::<Vec<_>>())?;
        if options.target.is_some()
            || options.curve.is_some()
            || options.automatic
            || options.references.iter().any(Option::is_some)
        {
            return Err(usage);
        }
        self.commands
            .object_selection_prompt(&options.command_line())?
            .ok_or(usage)
    }

    pub(super) fn start_align(&mut self, input: &str, postselected: bool) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Align")
        }) {
            return false;
        }
        let components = self.alignment_components();
        if self.document.selected_object_count() == 0
            && self.document.selected_control_points().next().is_none()
            && components.is_empty()
        {
            return false;
        }
        let Some(prompt) = self
            .commands
            .object_selection_prompt("Align")
            .ok()
            .flatten()
        else {
            return false;
        };
        let line = prompt.command_line();
        let Ok(options) = AlignmentOptions::default()
            .parse(&line.split_whitespace().skip(1).collect::<Vec<_>>())
            .and_then(|base| base.parse(&words.collect::<Vec<_>>()))
        else {
            return false;
        };
        if self.commands.accept_object_selection_input(input).is_err() {
            return false;
        }
        self.cancel_interactive_command(false);
        self.align_session = Some(Session {
            sources: AlignmentSources::capture(&self.document, &components),
            components,
            prepared: None,
            preview: None,
            frame_choice: false,
            rectangle: Default::default(),
        });
        let command = InteractiveCommand::Align {
            options,
            postselected,
        };
        self.active_command = Some(command);
        self.command_input.clear();
        if options.ready() {
            self.execute_align(options);
        } else {
            self.push_log(command.prompt().into());
        }
        true
    }

    pub(super) fn try_continue_align(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Align {
            options,
            postselected,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let word = input.trim_start_matches('_');
        let enter = input.is_empty() || word.eq_ignore_ascii_case("Enter");
        if let Some(session) = &mut self.align_session {
            if session.frame_choice {
                if word.eq_ignore_ascii_case("Cancel") || input == "!" {
                    self.cancel_interactive_command(true);
                    return true;
                }
                if enter {
                    session.frame_choice = false;
                    self.command_input.clear();
                    return true;
                }
                if word.eq_ignore_ascii_case("World") || word.eq_ignore_ascii_case("CPlane") {
                    session.frame_choice = false;
                    return self.try_continue_align(&format!("AlignTo={word}"));
                }
                self.push_log("Choose CPlane or World".into());
                return true;
            }
            if word.eq_ignore_ascii_case("AlignTo") {
                session.frame_choice = true;
                self.push_log("AlignTo (CPlane World)".into());
                self.command_input.clear();
                return true;
            }
            if options.three_point
                && options.references[0].is_none()
                && word.eq_ignore_ascii_case("EdgeMidpoint")
            {
                session.rectangle.edge_midpoint = !session.rectangle.edge_midpoint;
                let prompt = if session.rectangle.edge_midpoint {
                    "Pick the midpoint of the alignment plane edge"
                } else {
                    "Pick the first point of the alignment plane"
                };
                self.push_log(prompt.into());
                self.command_input.clear();
                return true;
            }
        }
        if self.continue_align_rectangle(input, enter, options) {
            return true;
        }
        if enter {
            self.finish_align(None, options);
            return true;
        }
        let arguments = input.split_whitespace().collect::<Vec<_>>();
        let first = arguments[0]
            .split('=')
            .next()
            .unwrap()
            .trim_start_matches('_');
        if !viboceros_command::AlignmentMode::LABELS
            .iter()
            .chain(["AlignTo", "Mode", "Auto", "3Point", "3Points", "CurveId"].iter())
            .any(|name| first.eq_ignore_ascii_case(name))
        {
            return false;
        }
        let previous = options;
        match options.parse(&arguments) {
            Ok(options) => {
                if (options.mode != previous.mode || options.three_point != previous.three_point)
                    && let Some(session) = &mut self.align_session
                {
                    session.rectangle = Default::default();
                }
                if let Err(error) = self
                    .commands
                    .accept_object_selection_input(&options.command_line())
                {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
                if options.ready() {
                    self.execute_align(options);
                } else {
                    let command = InteractiveCommand::Align {
                        options,
                        postselected,
                    };
                    self.active_command = Some(command);
                    if let Some(session) = &mut self.align_session {
                        session.preview = None;
                    }
                    self.push_log(options.command_line());
                    self.push_log(command.prompt().into());
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }

    pub(super) fn finish_align(
        &mut self,
        target: Option<Point3>,
        options: AlignmentOptions,
    ) -> bool {
        if options.mode.is_none() {
            self.push_log("Choose an alignment mode first".into());
            return false;
        }
        let updated = if let Some(point) = target {
            options.with_point(point)
        } else {
            options.parse(&["Auto"])
        };
        let options = match updated {
            Ok(options) => options,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        if !options.ready() {
            if let Some(InteractiveCommand::Align {
                options: active, ..
            }) = &mut self.active_command
            {
                *active = options;
            }
            if let Some(session) = &mut self.align_session {
                session.preview = None;
            }
            self.push_log(self.active_command.unwrap().prompt().into());
            return true;
        }
        self.execute_align(options)
    }

    fn execute_align(&mut self, options: AlignmentOptions) -> bool {
        let input = options.command_line();
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let postselected = matches!(
            self.active_command,
            Some(InteractiveCommand::Align {
                postselected: true,
                ..
            })
        );
        self.validate_align();
        let Some(session) = &mut self.align_session else {
            return false;
        };
        let result = match session.preview.take() {
            Some(preview)
                if preview.matches(options, context.construction_plane, &self.document) =>
            {
                preview.placement
            }
            _ => session.placement(&self.document, options, context.construction_plane),
        }
        .and_then(|placement| placement.commit(&mut self.document, postselected));
        match result {
            Ok(message) => {
                if let Some(session) = self.align_session.take() {
                    self.commands
                        .component_selection()
                        .consume(&session.components);
                }
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(message);
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                if matches!(
                    error,
                    viboceros_command::CommandError::InsufficientPlaneAlignmentObjects { .. }
                ) {
                    if postselected {
                        self.document.clear_selection();
                        self.commands
                            .component_selection()
                            .consume(&session.components);
                    }
                    self.cancel_interactive_command(false);
                }
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
