//! Repeated UV inspection with shared preferences and one marker undo record.
use super::*;
use viboceros_command::{EvaluateUvOptions, ObjectSelectionFilter, evaluate_surface_uv};
use viboceros_document::Geometry;

pub(super) struct EvaluateUvSession {
    marker_count: usize,
}

impl VibocerosApp {
    pub(super) fn evaluate_uv_can_pick(&self) -> bool {
        let mut selected = self.document.selected_objects();
        selected
            .next()
            .is_some_and(|object| ObjectSelectionFilter::SurfaceComponents.accepts_object(object))
            && selected.next().is_none()
    }
    pub(super) fn start_evaluate_uv(&self, input: &str) -> Option<InteractiveCommand> {
        let prompt = self.commands.object_selection_prompt(input).ok()??;
        if !self.evaluate_uv_can_pick() {
            return None;
        }
        let command_line = prompt.command_line();
        let arguments = command_line.split_whitespace().skip(1).collect::<Vec<_>>();
        let (options, _) = EvaluateUvOptions::default().parse(&arguments).ok()?;
        self.commands
            .accept_object_selection_options(&prompt)
            .ok()?;
        Some(InteractiveCommand::EvaluateUv { options })
    }

    pub(super) fn apply_evaluate_uv(&mut self, point: Point3, options: EvaluateUvOptions) -> bool {
        let input = format!("{} {}", options.command_line(), format_model_point(point));
        let result = match evaluate_surface_uv(&self.document, point, options) {
            Ok(result) => result,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        if let Some(marker) = result.marker {
            let starting = self.evaluate_uv_session.is_none();
            if starting && let Err(error) = self.document.begin_transaction("EvaluateUVPt") {
                self.push_log(format!("Error: {error}"));
                return false;
            }
            if let Err(error) = self.document.add_geometry(Geometry::Point(marker)) {
                if starting && let Err(rollback) = self.document.rollback_transaction() {
                    self.push_log(format!("Error rolling back EvaluateUVPt: {rollback}"));
                }
                self.push_log(format!("Error: {error}"));
                return false;
            }
            self.evaluate_uv_session
                .get_or_insert(EvaluateUvSession { marker_count: 0 })
                .marker_count += 1;
        }
        self.command_input.clear();
        self.push_log(format!("> {input}"));
        self.push_log(result.report);
        true
    }
    pub(super) fn try_continue_evaluate_uv(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::EvaluateUv { options }) = self.active_command else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        if input.trim().is_empty() {
            self.cancel_interactive_command(false);
            return true;
        }
        let tokens: Vec<_> = input.split_whitespace().collect();
        if !tokens.first().is_some_and(|token| {
            token.split_once('=').is_some_and(|(key, _)| {
                let key = key.trim_start_matches(['_', '-']);
                key.eq_ignore_ascii_case("Normalized") || key.eq_ignore_ascii_case("CreatePoint")
            })
        }) {
            return false;
        }
        match options.parse(&tokens) {
            Ok((options, None)) => {
                if let Err(error) = self
                    .commands
                    .accept_object_selection_input(&options.command_line())
                {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
                self.active_command = Some(InteractiveCommand::EvaluateUv { options });
                self.push_log(options.command_line());
            }
            _ => self.push_log("Error: expected Normalized=Yes|No or CreatePoint=Yes|No".into()),
        }
        self.command_input.clear();
        true
    }

    pub(super) fn finish_evaluate_uv_session(&mut self) {
        let Some(session) = self.evaluate_uv_session.take() else {
            return;
        };
        match self.document.commit_transaction() {
            Ok(_) => self.push_log(format!(
                "Finished EvaluateUVPt: {} point objects",
                session.marker_count
            )),
            Err(error) => self.push_log(format!("Error finishing EvaluateUVPt: {error}")),
        }
    }
}
