//! Shared flags retain the direction base across atomic option edits.
use super::*;
use viboceros_command::CurveExtrusionOptions;

pub(super) fn start(
    args: &[&str],
    split_available: bool,
    split_default: bool,
) -> Option<InteractiveCommand> {
    let mut options = CurveExtrusionOptions {
        split_at_tangents: split_default,
        ..Default::default()
    };
    options.update(args, split_available).ok()?;
    Some(InteractiveCommand::ExtrudeCurve {
        base: None,
        options,
        split_available,
    })
}
impl VibocerosApp {
    pub(super) fn continue_curve_extrusion_options(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeCurve {
            base,
            mut options,
            split_available,
        }) = self.active_command
        else {
            return false;
        };
        let args = input.split_whitespace().collect::<Vec<_>>();
        if !args.first().is_some_and(|a| {
            a.split_once('=').is_some_and(|(n, _)| {
                [
                    "Solid",
                    "BothSides",
                    "DeleteInput",
                    "Output",
                    "SplitAtTangents",
                ]
                .iter()
                .any(|k| n.trim_start_matches(['_', '-']).eq_ignore_ascii_case(k))
            })
        }) {
            return false;
        }
        if options.update(&args, split_available).is_ok() {
            self.active_command = Some(InteractiveCommand::ExtrudeCurve {
                base,
                options,
                split_available,
            });
            self.push_log(options.command_options(split_available));
        } else {
            self.push_log("Invalid extrusion option group".into());
        }
        self.command_input.clear();
        true
    }
}
