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
    viewport: usize,
    frame: Frame3,
    pub(super) points: Vec<Point3>,
    previous: Option<Point3>,
}

impl PlanePrompt {
    pub(super) fn anchor(&self) -> Option<Point3> {
        self.points.first().copied()
    }
    fn message(&self) -> &'static str {
        match (self.kind, self.points.len()) {
            (PlanePromptKind::Origin, _) => {
                "CPlane: pick a new origin (Enter keeps the current origin)"
            }
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
            (PlanePromptKind::Rotate, 0) => "CPlane Rotate: pick the rotation axis start",
            (PlanePromptKind::Rotate, 1) => "CPlane Rotate: pick the rotation axis end",
            (PlanePromptKind::Rotate, _) => "CPlane Rotate: type the angle in degrees",
        }
    }
}

impl VibocerosApp {
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
                self.apply_plane_action(action, self.active_viewport);
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn apply_plane_action(&mut self, action: PlaneAction, viewport: usize) {
        if let PlaneAction::Prompt(kind) = action {
            let prompt = PlanePrompt {
                kind,
                viewport,
                frame: self.viewports[viewport].construction_plane(),
                points: Vec::new(),
                previous: self.last_point,
            };
            self.push_log(prompt.message().into());
            self.plane_prompt = Some(prompt);
            return;
        }
        let state = &mut self.viewports[viewport].plane;
        let changed = match action {
            PlaneAction::Set(frame) => state.set(frame),
            PlaneAction::Undo => state.undo(),
            PlaneAction::Redo => state.redo(),
            PlaneAction::Prompt(_) => unreachable!(),
        };
        self.push_log(
            if changed {
                "Construction plane updated"
            } else {
                "Construction plane unchanged"
            }
            .into(),
        );
    }

    pub(super) fn cancel_plane_prompt(&mut self) {
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
                PlanePromptKind::Origin | PlanePromptKind::ThreePoint
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
                    let viewport = prompt.viewport;
                    self.plane_prompt = None;
                    self.apply_plane_action(PlaneAction::Set(frame), viewport);
                    self.command_input.clear();
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            return true;
        }
        let point = PointInput::parse(input)
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
        let result = (|| -> Result<Option<Frame3>, PlaneCommandError> {
            Ok(match prompt.kind {
                PlanePromptKind::Origin => Some(prompt.frame.with_origin(point)),
                PlanePromptKind::Through | PlanePromptKind::Elevation => {
                    Some(cplane::through(prompt.frame, point)?)
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
                    [origin, x] => Some(Frame3::try_from_points(*origin, *x, point, tolerance)?),
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
            Ok(Some(frame)) => {
                self.apply_plane_action(PlaneAction::Set(frame), prompt.viewport);
                self.command_input.clear();
                true
            }
            Ok(None) => {
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
