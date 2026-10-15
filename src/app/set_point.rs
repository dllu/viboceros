//! Interactive target and option editing for the typed SetPt command.

use super::*;

pub(super) struct SetPointSources {
    objects: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
    preview: Option<viboceros_geometry::AffineTransform3>,
}

impl SetPointSources {
    pub(super) fn affine_preview(
        &self,
        command: Option<InteractiveCommand>,
    ) -> Option<crate::viewport::AffinePreview<'_>> {
        let InteractiveCommand::SetPoint { options } = command? else {
            return None;
        };
        Some(crate::viewport::AffinePreview {
            sources: &self.objects,
            grips: &self.grips,
            definition: viboceros_command::point_transform::PointTransform::SetPoint(options),
            frame: None,
            copy: options.copy,
            last_transform: self.preview,
            rigid_layout: None,
        })
    }
}

impl VibocerosApp {
    pub(super) fn start_set_point_sources(&mut self, picked: Option<Vec<ObjectId>>) {
        self.set_point_sources = Some(SetPointSources {
            postselected: picked.is_some(),
            preview: None,
            objects: picked.unwrap_or_else(|| self.document.selected_object_ids().collect()),
            grips: self
                .document
                .selected_control_points()
                .map(|(id, _)| id)
                .collect(),
        });
    }

    pub(super) fn set_point_preview(&self) -> Option<crate::viewport::AffinePreview<'_>> {
        self.set_point_sources
            .as_ref()?
            .affine_preview(self.active_command)
    }

    pub(super) fn update_set_point_preview(
        &mut self,
        preview: Option<viboceros_geometry::AffineTransform3>,
    ) -> bool {
        let Some(sources) = self.set_point_sources.as_mut() else {
            return false;
        };
        if sources.preview == preview {
            return false;
        }
        sources.preview = preview;
        true
    }

    pub(super) fn apply_set_point(
        &mut self,
        point: Point3,
        options: viboceros_command::set_point::SetPointOptions,
    ) -> bool {
        let Some(sources) = self.set_point_sources.take() else {
            return false;
        };
        let objects = if sources.objects.is_empty() {
            String::new()
        } else {
            format!(
                "{}={}",
                if sources.postselected {
                    "PickedSources"
                } else {
                    "Sources"
                },
                sources
                    .objects
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let grips = transform_sources::grip_argument(&sources.grips, sources.postselected);
        let succeeded = self.try_execute_command(&format!(
            "SetPt {} {} {objects} {grips}",
            format_model_point(point),
            options.command_options()
        ));
        if !succeeded || options.copy {
            self.set_point_sources = Some(sources);
        }
        succeeded
    }

    pub(super) fn try_continue_set_point_option(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::SetPoint { options }) = self.active_command else {
            return false;
        };
        if !input.contains('=') {
            return false;
        }
        let mut updated = options;
        if let Err(error) = updated.update(input) {
            self.push_log(format!("Error: {error}"));
            return true;
        }
        if !updated.axes.contains(&true) {
            self.push_log("Error: SetPt needs at least one enabled axis".into());
            return true;
        }
        if (updated.axes != options.axes || updated.align_to_cplane != options.align_to_cplane)
            && let Some(sources) = self.set_point_sources.as_mut()
        {
            sources.preview = None;
        }
        let command = InteractiveCommand::SetPoint { options: updated };
        self.active_command = Some(command);
        self.push_log(format!("SetPt options: {}", updated.command_options()));
        self.push_log(command.prompt().to_owned());
        self.command_input.clear();
        true
    }

    pub(super) fn try_finish_set_point(&mut self, input: &str) -> bool {
        if !input.is_empty()
            || !matches!(
                self.active_command,
                Some(InteractiveCommand::SetPoint { options }) if options.copy
            )
        {
            return false;
        }
        self.cancel_interactive_command(false);
        self.push_log("Finished SetPt".into());
        self.command_input.clear();
        true
    }
}
