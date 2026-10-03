//! Repeated affine picks commit atomically into one external history entry.
use super::*;
use viboceros_document::HistoryGroup;

pub(super) struct TransformSession {
    group: HistoryGroup,
    copy: bool,
    applied: bool,
    factor: Option<f64>,
    sources: Vec<ObjectId>,
    postselected: bool,
    preview: Option<viboceros_geometry::AffineTransform3>,
}

impl TransformSession {
    pub(super) fn affine_preview(
        &self,
        command: Option<InteractiveCommand>,
        plane: Frame3,
    ) -> Option<crate::viewport::AffinePreview<'_>> {
        use viboceros_command::point_transform::PointTransform;
        let definition = match command? {
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                reference: Some(reference),
            } => match kind {
                InteractiveScaleKind::Uniform => PointTransform::Scale { center, reference },
                InteractiveScaleKind::OneDimensional => {
                    PointTransform::Scale1D { center, reference }
                }
                InteractiveScaleKind::TwoDimensional => {
                    PointTransform::Scale2D { center, reference }
                }
            },
            InteractiveCommand::Scale {
                kind: InteractiveScaleKind::OneDimensional,
                center: Some(center),
                reference: None,
            } => PointTransform::Scale1DDirection {
                center,
                factor: self.factor?,
            },
            InteractiveCommand::Rotate {
                center: Some(center),
                reference: Some(reference),
            } => PointTransform::Rotate { center, reference },
            InteractiveCommand::Rotate3D {
                points: [Some(start), Some(end), Some(reference)],
            } => PointTransform::Rotate3D {
                start,
                end,
                reference,
            },
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: Some(reference),
            } => PointTransform::Shear { origin, reference },
            _ => return None,
        };
        Some(crate::viewport::AffinePreview {
            sources: &self.sources,
            definition,
            frame: (!matches!(definition, PointTransform::Scale2D { .. })).then_some(plane),
            copy: self.copy,
            last_transform: self.preview,
        })
    }
    pub(super) fn mirror_preview(
        &self,
        command: Option<InteractiveCommand>,
    ) -> Option<crate::viewport::MirrorPreview<'_>> {
        use viboceros_command::mirror::MirrorPointPlane;
        let plane = match command? {
            InteractiveCommand::Mirror { start: Some(start) } => {
                MirrorPointPlane::TwoPoint { start }
            }
            InteractiveCommand::MirrorThreePoint {
                points: [Some(origin), Some(x)],
            } => MirrorPointPlane::ThreePoint { origin, x },
            _ => return None,
        };
        Some(crate::viewport::MirrorPreview {
            plane,
            sources: &self.sources,
            copy: self.copy,
            last_transform: self.preview,
        })
    }
}

pub(super) fn supports(command: InteractiveCommand) -> bool {
    matches!(
        command,
        InteractiveCommand::Scale { .. }
            | InteractiveCommand::Rotate { .. }
            | InteractiveCommand::Rotate3D { .. }
            | InteractiveCommand::Mirror { .. }
            | InteractiveCommand::MirrorThreePoint { .. }
            | InteractiveCommand::MirrorObject
            | InteractiveCommand::Shear { .. }
    )
}

pub(super) fn supports_name(name: &str) -> bool {
    matches!(
        name,
        "scale" | "scale1d" | "scale2d" | "rotate" | "rotate3d" | "mirror" | "shear"
    )
}

pub(super) fn copy_option(arguments: &[&str], default: bool) -> Option<bool> {
    let (name, value) = match arguments {
        [] => return Some(default),
        [argument] => argument.split_once('=')?,
        [name, value] => (*name, *value),
        _ => return None,
    };
    if !name
        .trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case("Copy")
    {
        return None;
    }
    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

pub(super) fn start_copy_option(name: &str, arguments: &[&str], default: bool) -> Option<bool> {
    if name == "mirror" {
        viboceros_command::mirror::start_options(arguments, default)
            .ok()
            .map(|(_, copy)| copy)
    } else {
        copy_option(arguments, default)
    }
}

impl VibocerosApp {
    pub(super) fn affine_preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        self.transform_session.as_ref()?.affine_preview(
            self.active_command,
            self.drafting_plane
                .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane()),
        )
    }

    pub(super) fn update_affine_preview(
        &mut self,
        preview: Option<viboceros_geometry::AffineTransform3>,
    ) -> bool {
        if self.affine_preview().is_none() {
            return false;
        }
        let session = self.transform_session.as_mut().unwrap();
        if session.preview == preview {
            return false;
        }
        session.preview = preview;
        true
    }
    pub(super) fn update_mirror_preview(
        &mut self,
        preview: Option<viboceros_geometry::AffineTransform3>,
    ) -> bool {
        let Some(session) = self.transform_session.as_mut() else {
            return false;
        };
        if session.mirror_preview(self.active_command).is_none() || session.preview == preview {
            return false;
        }
        session.preview = preview;
        true
    }

    pub(super) fn start_transform_session(
        &mut self,
        command: InteractiveCommand,
        copy: bool,
        picked_sources: Option<Vec<ObjectId>>,
    ) -> bool {
        match self.document.begin_history_group(command.name()) {
            Ok(group) => {
                self.transform_session = Some(TransformSession {
                    group,
                    copy,
                    applied: false,
                    factor: None,
                    postselected: picked_sources.is_some(),
                    sources: picked_sources.unwrap_or_else(|| {
                        self.document
                            .objects()
                            .filter_map(|object| {
                                self.document
                                    .is_selected(object.id())
                                    .then_some(object.id())
                            })
                            .collect()
                    }),
                    preview: None,
                });
                self.push_log(format!(
                    "Copy={} (edit with Copy=Yes|No)",
                    if copy { "Yes" } else { "No" }
                ));
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn apply_transform_step(
        &mut self,
        input: &str,
        continuation: InteractiveCommand,
    ) -> bool {
        let Some(session) = self.transform_session.as_mut() else {
            return false;
        };
        let display_input = format!("{input} Copy={}", if session.copy { "Yes" } else { "No" });
        let input = if session.postselected || continuation == InteractiveCommand::MirrorObject {
            format!(
                "{display_input} {}={}",
                if session.postselected {
                    "PickedSources"
                } else {
                    "Sources"
                },
                session
                    .sources
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        } else {
            display_input.clone()
        };
        let context = viboceros_command::CommandContext {
            construction_plane: if matches!(
                continuation,
                InteractiveCommand::Rotate { .. } | InteractiveCommand::Shear { .. }
            ) {
                self.drafting_plane
                    .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane())
            } else {
                self.viewports[self.active_viewport].construction_plane()
            },
        };
        match self.commands.execute_in_history_group(
            &mut self.document,
            &input,
            context,
            &mut session.group,
        ) {
            Ok(message) => {
                session.applied = true;
                self.push_log(format!("> {display_input}"));
                self.push_log(message);
                if self.transform_session.as_ref().unwrap().copy
                    && !matches!(
                        continuation,
                        InteractiveCommand::Mirror { .. }
                            | InteractiveCommand::MirrorThreePoint { .. }
                            | InteractiveCommand::MirrorObject
                    )
                {
                    self.active_command = Some(continuation);
                    let action = match continuation {
                        InteractiveCommand::Scale {
                            kind: InteractiveScaleKind::OneDimensional,
                            ..
                        } if self.transform_session.as_ref().unwrap().factor.is_some() => {
                            "Pick another direction"
                        }
                        InteractiveCommand::Scale {
                            reference: Some(_), ..
                        }
                        | InteractiveCommand::Rotate {
                            reference: Some(_), ..
                        }
                        | InteractiveCommand::Rotate3D {
                            points: [_, _, Some(_)],
                        } => "Pick another target",
                        InteractiveCommand::Scale { .. } => {
                            "Type another factor or pick a reference"
                        }
                        InteractiveCommand::Rotate { .. } | InteractiveCommand::Rotate3D { .. } => {
                            "Type another angle or pick a reference"
                        }
                        _ => "Type another shear angle or pick a target",
                    };
                    self.push_log(format!("{action}; Enter or Esc finishes accepted copies"));
                } else {
                    self.cancel_interactive_command(false);
                }
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn apply_numeric_scale_direction(
        &mut self,
        command: InteractiveCommand,
        direction: Point3,
    ) -> Option<bool> {
        let InteractiveCommand::Scale {
            kind: InteractiveScaleKind::OneDimensional,
            center: Some(center),
            reference: None,
        } = command
        else {
            return None;
        };
        let factor = self.transform_session.as_ref()?.factor?;
        Some(self.apply_transform_step(
            &format!(
                "Scale1D {} {factor} {}",
                format_model_point(center),
                format_model_point(direction)
            ),
            command,
        ))
    }

    pub(super) fn try_continue_transform(&mut self, input: &str) -> bool {
        if self.try_continue_mirror_option(input) || self.try_continue_mirror_target(input) {
            return true;
        }
        let (Some(command), Some(session)) = (self.active_command, self.transform_session.as_ref())
        else {
            return false;
        };
        if !supports(command) || self.plane_prompt.is_some() {
            return false;
        }
        let input = input.trim();
        let word = input.trim_start_matches(['_', '-']);
        if (input.is_empty() || word.eq_ignore_ascii_case("Enter"))
            && matches!(command, InteractiveCommand::Scale { center: None, .. })
        {
            // Rhino's automatic base point uses World bounds even when the
            // active construction plane is rotated or tilted.
            match viboceros_command::selected_bounding_box_center(
                &self.document,
                viboceros_command::CommandContext::default().construction_plane,
            ) {
                Ok(center) => {
                    self.accept_drafting_point(center);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        if word.eq_ignore_ascii_case("Cancel")
            || ((input.is_empty() || word.eq_ignore_ascii_case("Enter"))
                && (session.applied
                    || matches!(
                        command,
                        InteractiveCommand::Mirror { .. }
                            | InteractiveCommand::MirrorThreePoint { .. }
                            | InteractiveCommand::MirrorObject
                    )))
        {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        // Native Undo at these prompts leaves the accepted copies intact.
        // External Undo is available after the command finishes.
        if word.eq_ignore_ascii_case("Undo") {
            self.push_log("Finish the transform before using Undo".into());
            self.command_input.clear();
            return true;
        }
        let can_edit_copy = !matches!(
            command,
            InteractiveCommand::Rotate3D {
                points: [_, None, _] | [None, _, _]
            } | InteractiveCommand::MirrorObject
        );
        if word.to_ascii_lowercase().starts_with("copy=")
            || word
                .split_whitespace()
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("Copy"))
        {
            if !can_edit_copy {
                self.push_log(
                    if command == InteractiveCommand::MirrorObject {
                        "Set Copy before choosing the Object plane option"
                    } else {
                        "Copy is available after choosing the rotation axis"
                    }
                    .into(),
                );
                self.command_input.clear();
                return true;
            }
            if let Some(copy) =
                copy_option(&input.split_whitespace().collect::<Vec<_>>(), session.copy)
            {
                self.transform_session.as_mut().unwrap().copy = copy;
                self.push_log(format!("Copy={}", if copy { "Yes" } else { "No" }));
            } else {
                self.push_log("Enter Copy=Yes or Copy=No".into());
            }
            self.command_input.clear();
            return true;
        }
        let ready_for_scalar = matches!(
            command,
            InteractiveCommand::Scale {
                center: Some(_),
                reference: None,
                ..
            } | InteractiveCommand::Rotate {
                center: Some(_),
                reference: None
            } | InteractiveCommand::Rotate3D {
                points: [Some(_), Some(_), None]
            } | InteractiveCommand::Shear {
                origin: Some(_),
                reference: Some(_)
            }
        );
        if !ready_for_scalar || session.factor.is_some() {
            return false;
        }
        let value = if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let Some(value) = self.commands.transform_scalar_default(command.name()) else {
                // Fresh rotation commands and Shear have no angle default.
                self.cancel_interactive_command(true);
                self.command_input.clear();
                return true;
            };
            value
        } else {
            let Ok(value) = input.parse::<f64>() else {
                return false;
            };
            value
        };
        if !value.is_finite() {
            self.push_log("Error: transform values must be finite".into());
            self.command_input.clear();
            return true;
        }
        let script = match command {
            InteractiveCommand::Scale {
                kind: InteractiveScaleKind::OneDimensional,
                ..
            } => {
                self.commands
                    .remember_pending_transform_scalar(command.name(), value);
                self.transform_session.as_mut().unwrap().factor = Some(value);
                self.push_log(format!(
                    "Scale factor {value}; pick a direction (Copy=Yes keeps accepting directions)"
                ));
                self.command_input.clear();
                return true;
            }
            InteractiveCommand::Scale {
                kind,
                center: Some(center),
                ..
            } => format!("{} {} {value}", kind.name(), format_model_point(center)),
            InteractiveCommand::Rotate {
                center: Some(center),
                ..
            } => format!("Rotate {} {value}", format_model_point(center)),
            InteractiveCommand::Rotate3D {
                points: [Some(start), Some(end), None],
            } => format!(
                "Rotate3D {} {} {value}",
                format_model_point(start),
                format_model_point(end)
            ),
            InteractiveCommand::Shear {
                origin: Some(origin),
                reference: Some(reference),
            } => format!(
                "Shear {} {} {value}",
                format_model_point(origin),
                format_model_point(reference)
            ),
            _ => unreachable!(),
        };
        self.apply_transform_step(&script, command);
        self.command_input.clear();
        true
    }

    pub(super) fn transform_default_hint(&self) -> Option<String> {
        let command = self.active_command?;
        let session = self.transform_session.as_ref()?;
        if session.applied || session.factor.is_some() {
            return None;
        }
        let ready = matches!(
            command,
            InteractiveCommand::Scale {
                center: Some(_),
                reference: None,
                ..
            } | InteractiveCommand::Rotate {
                center: Some(_),
                reference: None
            } | InteractiveCommand::Rotate3D {
                points: [Some(_), Some(_), None]
            }
        );
        if !ready {
            return None;
        }
        self.commands
            .transform_scalar_default(command.name())
            .map(|value| format!("Enter accepts the default: {value}"))
    }

    /// Accepted edits already belong to ordinary history; dropping the token
    /// finishes the batch without rolling them back.
    pub(super) fn finish_transform_session(&mut self) -> bool {
        self.transform_session
            .take()
            .is_some_and(|session| session.applied)
    }
}
