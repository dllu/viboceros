//! Maelstrom source selection, Circle radii and repeated coil-angle placements.
use super::*;
use viboceros_command::circle_input::{CircleDefinition, CircleInput, CircleSizeMode};
use viboceros_command::maelstrom::{MaelstromRadius, circle_frame, coil_angle, point_morph};
use viboceros_geometry::GeometryError;

pub(super) struct MaelstromSession {
    sources: Vec<ObjectId>,
    postselected: bool,
    option: Option<&'static str>,
    group: viboceros_document::HistoryGroup,
    placed: bool,
    context: Option<viboceros_command::CommandContext>,
    circle_getter: Option<CircleInput>,
    preview_cursor: Option<crate::viewport::MaelstromCursor>,
    preview_cache: std::cell::RefCell<crate::viewport::MaelstromPreviewCache>,
}

impl MaelstromSession {
    pub(super) fn preview(
        &self,
        command: Option<InteractiveCommand>,
        cplane: Frame3,
    ) -> Option<crate::viewport::MaelstromPreview<'_>> {
        let InteractiveCommand::Maelstrom {
            center,
            initial,
            target,
            options,
        } = command?
        else {
            return None;
        };
        let center = center.or_else(|| self.circle_getter.and_then(CircleInput::anchor))?;
        Some(crate::viewport::MaelstromPreview {
            circle_getter: self.circle_getter,
            sources: &self.sources,
            center,
            initial,
            target,
            options,
            cplane: self.context.map_or(cplane, |c| c.construction_plane),
            last: self.preview_cursor,
            cache: &self.preview_cache,
        })
    }
}

impl VibocerosApp {
    pub(super) fn maelstrom_preview(&self) -> Option<crate::viewport::MaelstromPreview<'_>> {
        self.maelstrom_session.as_ref()?.preview(
            self.active_command,
            self.viewports[self.active_viewport].construction_plane(),
        )
    }
    pub(super) fn update_maelstrom_preview(
        &mut self,
        cursor: crate::viewport::MaelstromCursor,
    ) -> bool {
        if self.maelstrom_preview().is_none() {
            return false;
        }
        let session = self.maelstrom_session.as_mut().unwrap();
        if session.preview_cursor == Some(cursor) {
            return false;
        }
        session.preview_cursor = Some(cursor);
        true
    }
    pub(super) fn maelstrom_circle_anchor(&self) -> Option<Point3> {
        self.maelstrom_session.as_ref()?.circle_getter?.anchor()
    }
    fn push_maelstrom_prompt(&mut self) {
        let command = self.active_command.unwrap();
        let prompt = if matches!(
            command,
            InteractiveCommand::Maelstrom {
                initial: Some(_),
                target: None,
                ..
            }
        ) && self.commands.maelstrom_uses_diameter()
        {
            "Maelstrom: enter or pick the second diameter; Copy, Rigid"
        } else {
            command.prompt()
        };
        self.push_log(prompt.into());
    }
    fn accept_maelstrom_circle(&mut self, circle: CircleDefinition) -> bool {
        let Some(InteractiveCommand::Maelstrom { options, .. }) = self.active_command else {
            return false;
        };
        self.commands.remember_maelstrom_radius(circle.radius);
        self.commands.remember_maelstrom_copy_option(options.copy);
        let session = self.maelstrom_session.as_mut().unwrap();
        session.context = Some(viboceros_command::CommandContext {
            construction_plane: circle.frame,
        });
        session.circle_getter = None;
        session.preview_cursor = None;
        self.drafting_plane = Some(circle.frame);
        self.active_command = Some(InteractiveCommand::Maelstrom {
            center: Some(circle.frame.origin()),
            initial: Some(MaelstromRadius::Number(circle.radius)),
            target: None,
            options,
        });
        self.push_maelstrom_prompt();
        true
    }
    fn continue_maelstrom_circle_result(
        &mut self,
        getter: CircleInput,
        result: Result<Option<CircleDefinition>, GeometryError>,
    ) -> bool {
        match result {
            Ok(Some(circle)) => self.accept_maelstrom_circle(circle),
            Ok(None) => {
                self.maelstrom_session.as_mut().unwrap().circle_getter = Some(getter);
                self.push_log(getter.prompt().into());
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn start_maelstrom_session(&mut self, picked: Option<Vec<ObjectId>>) -> bool {
        let group = match self.document.begin_history_group("Maelstrom") {
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
        self.maelstrom_session = Some(MaelstromSession {
            sources,
            postselected,
            option: None,
            group,
            placed: false,
            context: None,
            circle_getter: self.commands.maelstrom_uses_diameter().then(|| {
                CircleInput::new(self.viewports[self.active_viewport].construction_plane())
                    .with_size_mode(CircleSizeMode::Diameter)
            }),
            preview_cursor: None,
            preview_cache: Default::default(),
        });
        true
    }

    pub(super) fn finish_maelstrom_session(&mut self) -> bool {
        self.maelstrom_session.take().is_some_and(|s| s.placed)
    }

    pub(super) fn try_continue_maelstrom(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Maelstrom {
            center,
            initial,
            target,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let Some(session) = self.maelstrom_session.as_mut() else {
            return false;
        };
        let word = input.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if initial.is_none() {
            let (option, value) = input
                .trim()
                .split_once('=')
                .map_or((word, None), |(name, value)| (name, Some(value)));
            let option = option.trim().trim_start_matches(['_', '-']);
            let mut getter = session.circle_getter.unwrap_or_else(|| {
                let plane = self.viewports[self.active_viewport].construction_plane();
                center.map_or(CircleInput::new(plane), |center| {
                    CircleInput::at_center(plane, center)
                })
            });
            if getter.option(option) {
                if option.eq_ignore_ascii_case("Diameter") {
                    self.commands.remember_maelstrom_diameter(true);
                }
                if option.eq_ignore_ascii_case("Radius") && getter.has_size_mode() {
                    self.commands.remember_maelstrom_diameter(false);
                }
                self.maelstrom_session.as_mut().unwrap().circle_getter = Some(getter);
                if let Some(value) = value {
                    match value.parse::<f64>() {
                        Ok(value) => {
                            let result = getter.number(value);
                            self.continue_maelstrom_circle_result(getter, result);
                        }
                        Err(_) => {
                            self.push_log("Error: circle size must be a finite number".into())
                        }
                    }
                } else {
                    self.push_log(getter.prompt().into());
                }
                self.command_input.clear();
                return true;
            }
        }
        if let Some(name) = session.option.take() {
            match options.update(&format!("{name}={input}")) {
                Ok(()) => self.update_maelstrom_options(center, initial, target, options),
                Err(e) => {
                    self.maelstrom_session.as_mut().unwrap().option = Some(name);
                    self.push_log(format!("Error: {e}"));
                }
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish Maelstrom before using Undo".into());
            self.command_input.clear();
            return true;
        }
        if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if initial.is_none()
                && self
                    .maelstrom_session
                    .as_ref()
                    .and_then(|s| s.circle_getter)
                    .is_some()
            {
                let mut getter = self
                    .maelstrom_session
                    .as_ref()
                    .unwrap()
                    .circle_getter
                    .unwrap();
                let radius = self.commands.maelstrom_radius_default();
                let size = match getter.size_mode() {
                    CircleSizeMode::Radius => radius,
                    CircleSizeMode::Diameter => 2. * radius,
                    CircleSizeMode::Circumference => std::f64::consts::TAU * radius,
                    CircleSizeMode::Area => std::f64::consts::PI * radius * radius,
                };
                let result = getter.number(size);
                self.continue_maelstrom_circle_result(getter, result);
            } else if center.is_some() && initial.is_none() {
                self.accept_maelstrom_radius(MaelstromRadius::Number(
                    self.commands.maelstrom_radius_default(),
                ));
            } else if initial.is_some() {
                // Native second-radius/GetAngle getters permit an empty exit.
                // Accepted copies remain in their single command history group.
                self.cancel_interactive_command(false);
            }
            self.command_input.clear();
            return true;
        }
        if let Some(name) = ["Copy", "Rigid"]
            .into_iter()
            .find(|name| word.eq_ignore_ascii_case(name))
        {
            if initial.is_some() {
                session.option = Some(name);
                self.push_log(format!("{name}: Yes or No"));
            } else {
                self.push_log("Copy and Rigid are available after the first radius".into());
            }
            self.command_input.clear();
            return true;
        }
        if input.contains('=') && !input.contains(',') {
            if initial.is_none() {
                self.push_log("Enter the center and first radius before transform options".into());
            } else {
                match options.update(input) {
                    Ok(()) => self.update_maelstrom_options(center, initial, target, options),
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        if center.is_none()
            && self
                .maelstrom_session
                .as_ref()
                .and_then(|s| s.circle_getter)
                .is_none()
        {
            return false;
        }
        if initial.is_none()
            && self
                .maelstrom_session
                .as_ref()
                .and_then(|s| s.circle_getter)
                .is_some_and(|getter| !getter.requests_size())
        {
            return false;
        }
        // At radius prompts a scalar is a size, before common drafting input
        // could treat it as a locked distance. Length suffixes use model units.
        let value = if target.is_some() {
            input.parse::<f64>().ok().map(Ok)
        } else if let Ok(v) = input.parse::<f64>() {
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
                Ok(v) if target.is_some() => {
                    self.finish_maelstrom(v);
                }
                Ok(v) => {
                    if let Some(mut getter) = self
                        .maelstrom_session
                        .as_ref()
                        .and_then(|s| s.circle_getter)
                    {
                        let result = getter.number(v);
                        self.continue_maelstrom_circle_result(getter, result);
                    } else {
                        let v = if initial.is_some() && self.commands.maelstrom_uses_diameter() {
                            v * 0.5
                        } else {
                            v
                        };
                        self.accept_maelstrom_radius(MaelstromRadius::Number(v));
                    }
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        false
    }

    fn update_maelstrom_options(
        &mut self,
        center: Option<Point3>,
        initial: Option<MaelstromRadius>,
        target: Option<MaelstromRadius>,
        options: viboceros_command::maelstrom::MaelstromOptions,
    ) {
        self.commands.remember_maelstrom_copy_option(options.copy);
        self.active_command = Some(InteractiveCommand::Maelstrom {
            center,
            initial,
            target,
            options,
        });
        self.push_log(options.command_options());
    }

    pub(super) fn accept_maelstrom_point(&mut self, point: Point3) -> bool {
        let Some(InteractiveCommand::Maelstrom {
            center,
            initial,
            target,
            options,
        }) = self.active_command
        else {
            return false;
        };
        if self
            .maelstrom_session
            .as_ref()
            .is_some_and(|s| s.option.is_some())
        {
            return false;
        }
        if let Some(mut getter) = self
            .maelstrom_session
            .as_ref()
            .and_then(|s| s.circle_getter)
        {
            let result = getter.point(point);
            return self.continue_maelstrom_circle_result(getter, result);
        }
        let Some(center) = center else {
            self.active_command = Some(InteractiveCommand::Maelstrom {
                center: Some(point),
                initial,
                target,
                options,
            });
            self.push_log(format!(
                "First radius <{}>; Diameter, Orientation, Circumference, Area",
                self.commands.maelstrom_radius_default()
            ));
            return true;
        };
        if target.is_none() {
            return self.accept_maelstrom_radius(MaelstromRadius::Point(point));
        }
        if self.maelstrom_preview().is_some_and(|p| p.last.is_some()) {
            return self
                .maelstrom_preview()
                .and_then(|p| p.angle(point))
                .is_some_and(|degrees| self.finish_maelstrom(degrees));
        }
        let context = self.maelstrom_session.as_ref().unwrap().context.unwrap();
        match circle_frame(center, initial.unwrap(), context)
            .and_then(|frame| coil_angle(frame, point))
        {
            Ok(degrees) => self.finish_maelstrom(degrees),
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }

    fn accept_maelstrom_radius(&mut self, radius: MaelstromRadius) -> bool {
        let Some(InteractiveCommand::Maelstrom {
            center: Some(center),
            initial,
            target: None,
            options,
        }) = self.active_command
        else {
            return false;
        };
        let context = self
            .maelstrom_session
            .as_ref()
            .and_then(|s| s.context)
            .unwrap_or(viboceros_command::CommandContext {
                construction_plane: self.viewports[self.active_viewport].construction_plane(),
            });
        // Both getters use the command adapter's measured Circle/radius policy.
        if let Err(e) = point_morph(center, initial.unwrap_or(radius), radius, 0., context) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let (initial, target) = if let Some(initial) = initial {
            (initial, Some(radius))
        } else {
            let frame = circle_frame(center, radius, context).unwrap();
            let length = radius.radius(frame).unwrap();
            self.commands.remember_maelstrom_radius(length);
            self.commands.remember_maelstrom_copy_option(options.copy);
            self.maelstrom_session.as_mut().unwrap().context = Some(context);
            self.drafting_plane = Some(frame);
            (radius, None)
        };
        self.active_command = Some(InteractiveCommand::Maelstrom {
            center: Some(center),
            initial: Some(initial),
            target,
            options,
        });
        self.maelstrom_session.as_mut().unwrap().preview_cursor = None;
        self.push_maelstrom_prompt();
        true
    }

    fn finish_maelstrom(&mut self, degrees: f64) -> bool {
        let Some(InteractiveCommand::Maelstrom {
            center: Some(center),
            initial: Some(initial),
            target: Some(target),
            options,
        }) = self.active_command
        else {
            return false;
        };
        let session = self.maelstrom_session.as_ref().unwrap();
        let context = session.context.unwrap();
        let sources = session
            .sources
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        // The command adapter uses the remembered numeric size mode. The
        // session stores physical radii and serializes them in that mode.
        let display_size = |r| match r {
            MaelstromRadius::Number(v) if self.commands.maelstrom_uses_diameter() => {
                MaelstromRadius::Number(v * 2.)
            }
            r => r,
        };
        let input = format!(
            "Maelstrom {} {} {} {degrees} {} {}={sources}",
            format_model_point(center),
            display_size(initial).command_argument(),
            display_size(target).command_argument(),
            options.command_options(),
            if session.postselected {
                "PickedSources"
            } else {
                "Sources"
            }
        );
        self.push_log(format!("> {input}"));
        let session = self.maelstrom_session.as_mut().unwrap();
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            context,
            &mut session.group,
        ) {
            Ok(message) => {
                session.placed = true;
                session.preview_cursor = None;
                self.push_log(message);
                if options.copy {
                    self.push_log("Maelstrom: enter another coil angle; Enter finishes, Esc keeps accepted copies".into());
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
