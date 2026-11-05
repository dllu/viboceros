//! Revolve axis getters share validated options with scripted execution.
use super::*;
use viboceros_command::revolve::Options;

pub(super) fn start(args: &[&str]) -> Option<InteractiveCommand> {
    Options::default()
        .updated(args)
        .ok()
        .map(|options| InteractiveCommand::Revolve {
            axis_start: None,
            options,
        })
}

impl VibocerosApp {
    pub(super) fn continue_revolve_options(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Revolve {
            axis_start,
            options,
        }) = self.active_command
        else {
            return false;
        };
        if input.is_empty() {
            if let Some(start) = axis_start {
                let direction = self.viewports[self.active_viewport]
                    .construction_plane()
                    .z_axis()
                    .as_vector();
                match start.translated(direction) {
                    Ok(end) => {
                        self.apply_drafting_point(end);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                return true;
            }
            return false;
        }
        let args = input.split_whitespace().collect::<Vec<_>>();
        if !args.first().is_some_and(|a| {
            a.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("FullCircle")
                || a.split_once('=').is_some_and(|(name, _)| {
                    [
                        "StartAngle",
                        "Angle",
                        "SweepAngle",
                        "FullCircle",
                        "DeleteInput",
                        "Deformable",
                        "PointCount",
                        "SegmentCount",
                        "Output",
                        "SplitAtTangents",
                    ]
                    .iter()
                    .any(|k| name.trim_start_matches(['_', '-']).eq_ignore_ascii_case(k))
                })
        }) {
            return false;
        }
        match options.updated(&args) {
            Ok(options) => {
                self.active_command = Some(InteractiveCommand::Revolve {
                    axis_start,
                    options,
                });
                self.push_log(format!(
                    "Angle={} {}",
                    options.sweep_degrees,
                    options.command_options()
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
