//! Interactive target and option editing for the typed SetPt command.

use super::*;

pub(super) struct SetPointSources {
    objects: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
}

impl VibocerosApp {
    pub(super) fn start_set_point_sources(&mut self, picked: Option<Vec<ObjectId>>) {
        self.set_point_sources = Some(SetPointSources {
            postselected: picked.is_some(),
            objects: picked.unwrap_or_else(|| self.document.selected_object_ids().collect()),
            grips: self
                .document
                .selected_control_points()
                .map(|(id, _)| id)
                .collect(),
        });
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
