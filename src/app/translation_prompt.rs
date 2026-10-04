//! Accepted Move/Copy destinations share geometry/history with full invocations.
use super::*;
use viboceros_command::translation::{CopyPlacement, DestinationConstraint};
use viboceros_document::HistoryGroup;

pub(super) struct TranslationSession {
    group: HistoryGroup,
    sources: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
    copy: bool,
    pub(super) vertical: bool,
    pub(super) normal: Option<move_normal::MoveNormal>,
    pub(super) placement: Option<CopyPlacement>,
    pub(super) applied: bool,
    preview: Option<viboceros_geometry::AffineTransform3>,
}

impl TranslationSession {
    pub(super) fn preview(&self) -> Option<crate::viewport::TranslationPreview<'_>> {
        Some(crate::viewport::TranslationPreview {
            sources: &self.sources,
            grips: &self.grips,
            base: self.placement?.base(),
            copy: self.copy,
            reference: self
                .normal
                .as_ref()
                .and_then(|normal| normal.target.map(|(id, _)| id)),
            last_transform: self.preview,
        })
    }
    pub(super) fn set_base(&mut self, base: Point3) {
        self.placement = Some(CopyPlacement::new(base));
    }
    pub(super) fn constraint(&self, plane: Frame3) -> Option<DestinationConstraint> {
        let mut constraint = self.placement?.constraint(
            self.normal
                .as_ref()
                .and_then(|normal| normal.direction)
                .or_else(|| self.vertical.then_some(plane.z_axis())),
        );
        if let Some(normal) = &self.normal {
            constraint.distance = normal.distance;
        }
        Some(constraint)
    }
    fn source_argument(&self) -> String {
        let objects = if self.sources.is_empty() {
            String::new()
        } else {
            format!(
                "{}={}",
                if self.postselected {
                    "PickedSources"
                } else {
                    "Sources"
                },
                self.sources
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let grips = super::transform_sources::grip_argument(&self.grips, self.postselected);
        format!("{objects} {grips}")
    }
}

pub(super) fn supports(command: InteractiveCommand) -> bool {
    matches!(
        command,
        InteractiveCommand::Move { .. } | InteractiveCommand::Copy { .. }
    )
}

pub(super) fn supports_name(name: &str) -> bool {
    matches!(name, "move" | "m" | "copy")
}

/// Only base-prompt options can start an interactive command inline.
pub(super) fn start_options(name: &str, arguments: &[&str]) -> Option<(bool, bool)> {
    if !supports_name(name) {
        return None;
    }
    let mut vertical = false;
    let mut in_place = false;
    for argument in arguments {
        let word = argument.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Vertical") {
            vertical = true;
        } else if word.eq_ignore_ascii_case("InPlace") && name == "copy" {
            in_place = true;
        } else if name == "copy"
            && let Some((key, value)) = word.split_once('=')
            && key.eq_ignore_ascii_case("Vertical")
        {
            vertical = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "yes" => true,
                "no" => false,
                _ => return None,
            };
        } else if word.eq_ignore_ascii_case("Normal") && name != "copy" {
            // Normal's separate reference prompt starts after the session.
        } else {
            return None;
        }
    }
    Some((vertical, in_place))
}

impl VibocerosApp {
    pub(super) fn update_translation_preview(
        &mut self,
        preview: Option<viboceros_geometry::AffineTransform3>,
    ) -> bool {
        let Some(session) = self.translation_session.as_mut() else {
            return false;
        };
        if session.placement.is_none() || session.preview == preview {
            return false;
        }
        session.preview = preview;
        true
    }
    pub(super) fn start_translation_session(
        &mut self,
        command: InteractiveCommand,
        picked: Option<Vec<ObjectId>>,
        vertical: bool,
    ) -> bool {
        let group = match self.document.begin_history_group(command.name()) {
            Ok(group) => group,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let postselected = picked.is_some();
        let sources = picked.unwrap_or_else(|| {
            self.document
                .objects()
                .filter_map(|object| {
                    self.document
                        .is_selected(object.id())
                        .then_some(object.id())
                })
                .collect()
        });
        self.translation_session = Some(TranslationSession {
            group,
            sources,
            grips: self
                .document
                .selected_control_points()
                .map(|(id, _)| id)
                .collect(),
            postselected,
            copy: matches!(command, InteractiveCommand::Copy { .. }),
            vertical,
            normal: None,
            placement: None,
            applied: false,
            preview: None,
        });
        true
    }

    pub(super) fn translation_constraint(&self) -> Option<DestinationConstraint> {
        self.translation_session
            .as_ref()?
            .constraint(self.viewports[self.active_viewport].construction_plane())
    }

    fn execute_translation_edit(&mut self, display: &str) -> bool {
        let session = self.translation_session.as_mut().unwrap();
        let input = format!("{display} {}", session.source_argument());
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            context,
            &mut session.group,
        ) {
            Ok(message) => {
                session.applied = true;
                self.push_log(format!("> {display}"));
                self.push_log(message);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn apply_translation_step(&mut self, destination: Point3) -> bool {
        let Some(session) = self.translation_session.as_ref() else {
            return false;
        };
        let Some(mut placement) = session.placement else {
            return false;
        };
        if let Err(error) = placement.accept(destination) {
            self.push_log(format!("Error: {error}"));
            return false;
        }
        let copy = session.copy;
        if session.vertical && copy {
            placement.use_direction(
                self.viewports[self.active_viewport]
                    .construction_plane()
                    .z_axis(),
            );
        }
        let name = if copy { "Copy" } else { "Move" };
        let display = format!(
            "{name} {} {}",
            format_model_point(placement.base()),
            format_model_point(destination)
        );
        if !self.execute_translation_edit(&display) {
            return false;
        }
        let session = self.translation_session.as_mut().unwrap();
        session.placement = Some(placement);
        if copy {
            session.vertical = false;
        }
        if copy {
            self.active_command = Some(InteractiveCommand::Copy {
                start: Some(placement.anchor()),
            });
            self.log_copy_placement();
        } else {
            self.cancel_interactive_command(false);
        }
        true
    }

    fn log_copy_placement(&mut self) {
        let options = self
            .translation_session
            .as_ref()
            .unwrap()
            .placement
            .unwrap()
            .options;
        let yn = |value| if value { "Yes" } else { "No" };
        self.push_log(format!("Copy: pick another destination; FromLastPoint={} UseLastDistance={} UseLastDirection={}; Enter or Esc finishes", yn(options.from_last_point), yn(options.use_last_distance), yn(options.use_last_direction)));
    }

    pub(super) fn try_continue_translation(&mut self, input: &str) -> bool {
        let Some(session) = self.translation_session.as_ref() else {
            return false;
        };
        let input = input.trim();
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            if session.normal.is_some() && session.placement.is_none() {
                self.cancel_interactive_command(true);
            } else if session.placement.is_none() && !(session.vertical && !session.copy) {
                match viboceros_command::selected_bounding_box_center(
                    &self.document,
                    viboceros_command::CommandContext::default().construction_plane,
                ) {
                    Ok(center) => {
                        self.accept_drafting_point(center);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            } else if !session.copy && session.placement.is_some() {
                if session.normal.is_some() {
                    let distance = self
                        .commands
                        .transform_scalar_default("Move")
                        .filter(|distance| *distance > 0.0);
                    self.translation_session
                        .as_mut()
                        .unwrap()
                        .normal
                        .as_mut()
                        .unwrap()
                        .distance = distance;
                }
                self.push_log("Pick the destination; Esc cancels Move".into());
            } else {
                self.cancel_interactive_command(true);
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish the command before using Undo".into());
            self.command_input.clear();
            return true;
        }
        if self.try_continue_move_normal(input) {
            return true;
        }
        let session = self.translation_session.as_ref().unwrap();
        if word.eq_ignore_ascii_case("Normal") && !session.copy {
            if session.placement.is_none() {
                self.start_move_normal();
            } else {
                self.push_log("Normal is available at the base-point prompt".into());
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("InPlace") && session.copy {
            if session.placement.is_some() {
                self.push_log("InPlace is available at the base-point prompt".into());
            } else if self.execute_translation_edit("Copy InPlace") {
                self.cancel_interactive_command(false);
            }
            self.command_input.clear();
            return true;
        }
        let name = word.split('=').next().unwrap_or("");
        if name.eq_ignore_ascii_case("Vertical") {
            if session.placement.is_some() || session.normal.is_some() {
                self.push_log("Vertical is available at the base-point prompt".into());
            } else if let Some((vertical, _)) =
                start_options(if session.copy { "copy" } else { "move" }, &[word])
            {
                let value = if word.eq_ignore_ascii_case("Vertical") && session.copy {
                    !session.vertical
                } else {
                    vertical
                };
                self.translation_session.as_mut().unwrap().vertical = value;
                self.push_log("Pick the base point".into());
            } else {
                self.push_log("Error: Vertical=Yes|No".into());
            }
            self.command_input.clear();
            return true;
        }
        if ["FromLastPoint", "UseLastDistance", "UseLastDirection"]
            .iter()
            .any(|option| name.eq_ignore_ascii_case(option))
        {
            if !session.copy || !session.applied {
                self.push_log(
                    "Copy repetition options are available after the first placement".into(),
                );
            } else {
                let placement = self
                    .translation_session
                    .as_mut()
                    .unwrap()
                    .placement
                    .as_mut()
                    .unwrap();
                match placement.options.update(word) {
                    Ok(()) => {
                        self.active_command = Some(InteractiveCommand::Copy {
                            start: Some(placement.anchor()),
                        });
                        self.log_copy_placement();
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
            }
            self.command_input.clear();
            return true;
        }
        if self.point_filter.is_none()
            && let Some(constraint) = self.translation_constraint()
            && let Some(direction) = constraint.direction
        {
            let distance = match viboceros_drafting::PointConstraintInput::parse_with_units(
                input,
                self.document.units(),
            ) {
                Some(Ok(viboceros_drafting::PointConstraintInput::Distance(distance))) => {
                    Some(distance)
                }
                _ => input
                    .parse::<f64>()
                    .ok()
                    .filter(|distance| distance.is_finite()),
            };
            if let Some(distance) = distance {
                let target = direction
                    .as_vector()
                    .scaled(distance)
                    .and_then(|offset| constraint.anchor.translated(offset));
                match target {
                    Ok(target) => {
                        self.accept_resolved_translation_point(target);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                self.command_input.clear();
                return true;
            }
        }
        false
    }
}
