//! UV inspection options and object-selection-to-point-input handoff.
use super::*;
use viboceros_command::{EvaluateUvOptions, ObjectSelectionFilter};

impl VibocerosApp {
    pub(super) fn evaluate_uv_can_pick(&self) -> bool {
        let mut selected = self.document.selected_objects();
        selected
            .next()
            .is_some_and(|object| ObjectSelectionFilter::SurfaceComponents.accepts_object(object))
            && selected.next().is_none()
    }
    pub(super) fn finish_evaluate_uv(&mut self, point: Point3, options: EvaluateUvOptions) -> bool {
        let input = format!("{} {}", options.command_line(), format_model_point(point));
        match self.commands.execute(&mut self.document, &input) {
            Ok(report) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(report);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
    pub(super) fn try_continue_evaluate_uv(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::EvaluateUv { options }) = self.active_command else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
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
                self.active_command = Some(InteractiveCommand::EvaluateUv { options });
                self.push_log(options.command_line());
            }
            _ => self.push_log("Error: expected Normalized=Yes|No or CreatePoint=Yes|No".into()),
        }
        self.command_input.clear();
        true
    }
}
