//! A small, separate prompt stack for transparent construction-plane editing.
//! Accepted model points, the model's latched plane, and model undo stay intact.
use super::*;
use viboceros_command::construction_plane::{
    self as cplane, PlaneAction, PlaneCommandError, PlanePromptKind,
};
use viboceros_drafting::PointInput;

#[derive(Clone, Debug)]
pub(super) struct PlanePrompt {
    kind: PlanePromptKind,
    pub(super) viewport: usize,
    frame: Frame3,
    pub(super) points: Vec<Point3>,
    previous: Option<Point3>,
}

impl PlanePrompt {
    pub(super) fn requests_point(&self) -> bool {
        self.kind != PlanePromptKind::Rotate || self.points.len() < 2
    }
    pub(super) fn anchor(&self) -> Option<Point3> {
        self.points.first().copied()
    }
    fn message(&self) -> &'static str {
        match (self.kind, self.points.len()) {
            (PlanePromptKind::Origin, _) => {
                "CPlane: pick a new origin (Enter keeps the current origin)"
            }
            (PlanePromptKind::AllOrigin, _) => "CPlane All: pick the new origin for every viewport",
            (PlanePromptKind::ThreePoint, 0) => {
                "CPlane 3Point: pick the origin (Enter keeps the current origin)"
            }
            (PlanePromptKind::ThreePoint, 1) => {
                "CPlane 3Point: pick a point on the positive X axis"
            }
            (PlanePromptKind::ThreePoint, _) => {
                "CPlane 3Point: pick a point in the positive XY half-plane"
            }
            (PlanePromptKind::Elevation, _) => {
                "CPlane Elevation: type an offset distance or pick a height point"
            }
            (PlanePromptKind::Through, _) => {
                "CPlane Through: pick a point for the plane to pass through"
            }
            (PlanePromptKind::ThroughAll, _) => {
                "CPlane Through All: pick a point for every plane to pass through"
            }
            (PlanePromptKind::Rotate, 0) => "CPlane Rotate: pick the rotation axis start",
            (PlanePromptKind::Rotate, 1) => "CPlane Rotate: pick the rotation axis end",
            (PlanePromptKind::Rotate, _) => "CPlane Rotate: type the angle in degrees",
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_run_synchronize_cplanes_command(&mut self, input: &str) -> bool {
        let (command, argument) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        if !command
            .trim_start_matches(['\'', '_', '-'])
            .eq_ignore_ascii_case("SynchronizeCPlanes")
        {
            return false;
        }
        self.push_log(format!("> {input}"));
        let result = (|| -> Result<String, String> {
            let mut set_view = true;
            let mut source_words = Vec::new();
            for word in argument.split_whitespace() {
                if let Some(value) = word.strip_prefix("SetView=").or_else(|| {
                    word.get(..8)
                        .filter(|prefix| prefix.eq_ignore_ascii_case("SetView="))
                        .map(|_| &word[8..])
                }) {
                    set_view = if value.eq_ignore_ascii_case("Yes") {
                        true
                    } else if value.eq_ignore_ascii_case("No") {
                        false
                    } else {
                        return Err("SetView must be Yes or No".into());
                    };
                } else {
                    source_words.push(word);
                }
            }
            let source_name = source_words.join(" ");
            let source_name = source_name.trim_matches('"');
            let source_index = if source_name.is_empty() {
                self.active_viewport
            } else {
                self.resolve_viewport_reference(source_name)?
            };
            let source = self.viewports[source_index].construction_plane();
            let mut updated = 0;
            for viewport in &mut self.viewports {
                if let Some((named_role, plane_role)) = viewport.synchronization_plane_role() {
                    viewport.synchronize_cplane(source, named_role, plane_role, set_view);
                    updated += 1;
                }
            }
            Ok(format!(
                "Synchronized {updated} standard viewports from viewport {} ({})",
                source_index + 1,
                self.viewports[source_index].view_label()
            ))
        })();
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(message) => self.push_log(format!("Error: {message}")),
        }
        true
    }

    pub(super) fn try_run_copy_cplane_command(&mut self, input: &str) -> bool {
        let (command, argument) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        let command = command.trim_start_matches(['\'', '_', '-']);
        let copy_plane = command.eq_ignore_ascii_case("CopyCPlaneToAll");
        if !copy_plane && !command.eq_ignore_ascii_case("CopyCPlaneSettingsToAll") {
            return false;
        }
        self.push_log(format!("> {input}"));
        let result = (|| -> Result<String, String> {
            let argument = argument.trim();
            let source = if argument.is_empty() {
                self.active_viewport
            } else {
                let argument = if argument.starts_with('"')
                    && argument.ends_with('"')
                    && argument.len() >= 2
                {
                    &argument[1..argument.len() - 1]
                } else {
                    argument
                };
                self.resolve_viewport_reference(argument)?
            };
            if copy_plane {
                let frame = self.viewports[source].construction_plane();
                for (index, viewport) in self.viewports.iter_mut().enumerate() {
                    if index != source {
                        viewport.plane.set(frame);
                    }
                }
            } else {
                let grid = self.viewports[source].grid_settings();
                for (index, viewport) in self.viewports.iter_mut().enumerate() {
                    if index != source {
                        viewport.set_grid_settings(grid);
                    }
                }
            }
            Ok(format!(
                "Copied {} from viewport {} ({}) to all viewports",
                if copy_plane {
                    "construction plane"
                } else {
                    "grid and snap settings"
                },
                source + 1,
                self.viewports[source].view_label()
            ))
        })();
        match result {
            Ok(message) => {
                self.push_log(message);
                self.command_input.clear();
            }
            Err(message) => self.push_log(format!("Error: {message}")),
        }
        true
    }

    pub(super) fn handle_plane_shortcuts(&mut self, ui: &mut egui::Ui) {
        // Shift+Home/End select text in editors. Keep those editing operations
        // intact; CPlane Undo/Redo can also be entered at any command prompt.
        if ui.ctx().text_edit_focused() {
            return;
        }
        let actions = ui.input_mut(|input| {
            let mut actions = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    modifiers,
                    pressed,
                    repeat,
                    ..
                } = event
                    && modifiers.matches_exact(egui::Modifiers::SHIFT)
                    && matches!(key, egui::Key::Home | egui::Key::End)
                {
                    if *pressed && !*repeat {
                        actions.push(if *key == egui::Key::Home {
                            PlaneAction::Undo
                        } else {
                            PlaneAction::Redo
                        });
                    }
                    false
                } else {
                    true
                }
            });
            actions
        });
        for action in actions {
            self.apply_plane_action(action, self.active_viewport);
        }
    }

    pub(super) fn try_run_plane_command(&mut self, input: &str) -> bool {
        let frame = self.viewports[self.active_viewport].construction_plane();
        let Some(action) = cplane::parse(input, frame, self.last_point, self.document.tolerance())
        else {
            return false;
        };
        self.push_log(format!("> {input}"));
        match action {
            Ok(action) => {
                self.plane_prompt = None;
                if self.apply_plane_action(action, self.active_viewport) {
                    self.command_input.clear();
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn apply_plane_action(&mut self, action: PlaneAction, viewport: usize) -> bool {
        if let PlaneAction::Prompt(kind) = action {
            self.snaps.plane_override = None;
            let prompt = PlanePrompt {
                kind,
                viewport,
                frame: self.viewports[viewport].construction_plane(),
                points: Vec::new(),
                previous: self.last_point,
            };
            self.push_log(prompt.message().into());
            self.plane_prompt = Some(prompt);
            return true;
        }
        if let PlaneAction::SetAllOrigin(point) | PlaneAction::SetThroughAll(point) = action {
            let frames = self
                .viewports
                .iter()
                .map(|view| {
                    let frame = view.construction_plane();
                    if matches!(action, PlaneAction::SetAllOrigin(_)) {
                        Ok(frame.with_origin(point))
                    } else {
                        cplane::through(frame, point)
                    }
                })
                .collect::<Result<Vec<_>, _>>();
            let frames = match frames {
                Ok(frames) => frames,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            };
            for (view, frame) in self.viewports.iter_mut().zip(frames) {
                view.plane.set(frame);
            }
            self.push_log(format!(
                "Updated construction planes in {} viewports",
                self.viewports.len()
            ));
            return true;
        }
        let state = &mut self.viewports[viewport].plane;
        let changed = match action {
            PlaneAction::Set(frame) => state.set(frame),
            PlaneAction::Undo => state.undo(),
            PlaneAction::Redo => state.redo(),
            PlaneAction::Prompt(_) => unreachable!(),
            PlaneAction::SetAllOrigin(_) | PlaneAction::SetThroughAll(_) => unreachable!(),
        };
        self.push_log(
            if changed {
                "Construction plane updated"
            } else {
                "Construction plane unchanged"
            }
            .into(),
        );
        true
    }

    pub(super) fn cancel_plane_prompt(&mut self) {
        self.snaps.plane_override = None;
        if self.plane_prompt.take().is_some() {
            self.command_input.clear();
            self.push_log("CPlane cancelled; previous modeling prompt retained".into());
        }
    }

    pub(super) fn try_continue_plane_prompt(&mut self, input: &str) -> bool {
        let Some(prompt) = &self.plane_prompt else {
            return false;
        };
        if self
            .commands
            .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_plane_prompt();
            return false;
        }
        if input.is_empty()
            && prompt.points.is_empty()
            && matches!(
                prompt.kind,
                PlanePromptKind::Origin | PlanePromptKind::AllOrigin | PlanePromptKind::ThreePoint
            )
        {
            self.accept_plane_prompt_point(prompt.frame.origin());
            return true;
        }
        if (prompt.kind == PlanePromptKind::Elevation
            || (prompt.kind == PlanePromptKind::Rotate && prompt.points.len() == 2))
            && let Ok(value) = input.parse::<f64>()
        {
            let frame = if !value.is_finite() {
                Err(PlaneCommandError::Number)
            } else if prompt.kind == PlanePromptKind::Elevation {
                cplane::elevated(prompt.frame, value).map_err(PlaneCommandError::from)
            } else {
                cplane::rotated(
                    prompt.frame,
                    prompt.points[0],
                    prompt.points[1],
                    value,
                    self.document.tolerance(),
                )
                .map_err(PlaneCommandError::from)
            };
            match frame {
                Ok(frame) => {
                    self.snaps.plane_override = None;
                    let viewport = prompt.viewport;
                    self.plane_prompt = None;
                    self.apply_plane_action(PlaneAction::Set(frame), viewport);
                    self.command_input.clear();
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        let point = PointInput::parse_with_units(input, self.document.units())
            .ok_or(PlaneCommandError::Usage)
            .and_then(|p| p.map_err(PlaneCommandError::from))
            .and_then(|p| {
                p.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    prompt.previous,
                )
                .map_err(PlaneCommandError::from)
            });
        match point {
            Ok(point) => {
                self.accept_plane_prompt_point(point);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn accept_plane_prompt_point(&mut self, point: Point3) -> bool {
        let Some(mut prompt) = self.plane_prompt.take() else {
            return false;
        };
        let tolerance = self.document.tolerance();
        let result = (|| -> Result<Option<PlaneAction>, PlaneCommandError> {
            Ok(match prompt.kind {
                PlanePromptKind::Origin => Some(PlaneAction::Set(prompt.frame.with_origin(point))),
                PlanePromptKind::AllOrigin => Some(PlaneAction::SetAllOrigin(point)),
                PlanePromptKind::ThroughAll => Some(PlaneAction::SetThroughAll(point)),
                PlanePromptKind::Through | PlanePromptKind::Elevation => {
                    Some(PlaneAction::Set(cplane::through(prompt.frame, point)?))
                }
                PlanePromptKind::ThreePoint => match prompt.points.as_slice() {
                    [] => {
                        prompt.points.push(point);
                        None
                    }
                    [origin] => {
                        origin.vector_to(point)?.normalized(tolerance)?;
                        prompt.points.push(point);
                        None
                    }
                    [origin, x] => Some(PlaneAction::Set(Frame3::try_from_points(
                        *origin, *x, point, tolerance,
                    )?)),
                    _ => unreachable!(),
                },
                PlanePromptKind::Rotate => match prompt.points.as_slice() {
                    [] => {
                        prompt.points.push(point);
                        None
                    }
                    [start] => {
                        start.vector_to(point)?.normalized(tolerance)?;
                        prompt.points.push(point);
                        None
                    }
                    _ => return Err(PlaneCommandError::Number),
                },
            })
        })();
        match result {
            Ok(Some(action)) => {
                self.snaps.plane_override = None;
                if self.apply_plane_action(action, prompt.viewport) {
                    self.command_input.clear();
                    true
                } else {
                    self.plane_prompt = Some(prompt);
                    false
                }
            }
            Ok(None) => {
                self.snaps.plane_override = None;
                prompt.previous = Some(point);
                self.push_log(prompt.message().into());
                self.plane_prompt = Some(prompt);
                self.command_input.clear();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.plane_prompt = Some(prompt);
                false
            }
        }
    }
}
