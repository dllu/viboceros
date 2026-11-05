//! Curve-extrusion options shared by the command start and direction getters.
use super::*;

#[derive(Clone, Copy, Default)]
struct Options {
    both_sides: bool,
    delete_input: bool,
    solid: bool,
}

impl Options {
    fn parse(mut self, args: &[&str]) -> Option<Self> {
        let mut seen = std::collections::BTreeSet::new();
        for arg in args {
            let (name, value) = arg.split_once('=')?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let value = value.trim_start_matches('_');
            if !seen.insert(name.clone()) {
                return None;
            }
            if name == "output" {
                if !value.eq_ignore_ascii_case("Surface") {
                    return None;
                }
                continue;
            }
            let enabled = if value.eq_ignore_ascii_case("Yes") {
                true
            } else if value.eq_ignore_ascii_case("No") {
                false
            } else {
                return None;
            };
            match name.as_str() {
                "bothsides" => self.both_sides = enabled,
                "deleteinput" => self.delete_input = enabled,
                "solid" => self.solid = enabled,
                _ => return None,
            }
        }
        Some(self)
    }
    fn command(self, base: Option<Point3>) -> InteractiveCommand {
        InteractiveCommand::ExtrudeCurve {
            base,
            both_sides: self.both_sides,
            delete_input: self.delete_input,
            solid: self.solid,
        }
    }
}

pub(super) fn start(args: &[&str]) -> Option<InteractiveCommand> {
    Options::default().parse(args).map(|o| o.command(None))
}

impl VibocerosApp {
    pub(super) fn continue_curve_extrusion_options(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeCurve {
            base,
            both_sides,
            delete_input,
            solid,
        }) = self.active_command
        else {
            return false;
        };
        let args = input.split_whitespace().collect::<Vec<_>>();
        if !args.first().is_some_and(|a| {
            a.split_once('=').is_some_and(|(name, _)| {
                ["Solid", "BothSides", "DeleteInput", "Output"]
                    .iter()
                    .any(|k| name.trim_start_matches(['_', '-']).eq_ignore_ascii_case(k))
            })
        }) {
            return false;
        }
        if let Some(options) = (Options {
            both_sides,
            delete_input,
            solid,
        })
        .parse(&args)
        {
            self.active_command = Some(options.command(base));
            self.push_log(format!(
                "Solid={} BothSides={} DeleteInput={}",
                if options.solid { "Yes" } else { "No" },
                if options.both_sides { "Yes" } else { "No" },
                if options.delete_input { "Yes" } else { "No" }
            ));
        } else {
            self.push_log(
                "Enter Solid=Yes|No, BothSides=Yes|No, DeleteInput=Yes|No or Output=Surface".into(),
            );
        }
        self.command_input.clear();
        true
    }
}
