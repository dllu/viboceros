//! Coordinate queries keep failed point input open for correction.
use super::*;

pub(super) fn start_command(arguments: &[&str]) -> Option<InteractiveCommand> {
    let valid = match arguments {
        [] => true,
        [option] => option.split_once('=').is_some_and(|(key, value)| {
            key.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Label")
                && value.trim_start_matches('_').eq_ignore_ascii_case("No")
        }),
        _ => false,
    };
    valid.then_some(InteractiveCommand::EvaluatePoint)
}

impl VibocerosApp {
    pub(super) fn finish_evaluate_point(&mut self, point: Point3, plane: Frame3) -> bool {
        let input = format!("EvaluatePt {}", format_model_point(point));
        match self.commands.execute_in_context(
            &mut self.document,
            &input,
            viboceros_command::CommandContext {
                construction_plane: plane,
            },
        ) {
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
}
