//! Atomic options shared by direct commands and the interactive apex getter.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub output: CurveExtrusionOutput,
    pub delete_input: bool,
    pub solid: bool,
}
impl Options {
    pub fn update(&mut self, arguments: &[&str]) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = BTreeSet::new();
        for argument in arguments {
            let (name, value) = argument
                .split_once('=')
                .ok_or(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE))?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            if !seen.insert(name.clone()) {
                return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE));
            }
            match name.as_str() {
                "output" => {
                    next.output = CurveExtrusionOutput::parse(value)
                        .ok_or(CommandError::UnsupportedCurveToPointExtrusionOutput)?
                }
                "deleteinput" => {
                    next.delete_input = parse_yes_no(value)
                        .ok_or(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE))?
                }
                "solid" => {
                    next.solid = parse_yes_no(value)
                        .ok_or(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE))?
                }
                _ => return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE)),
            }
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        format!(
            "Output={} Solid={} DeleteInput={}",
            self.output.name(),
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" }
        )
    }
}

pub(super) fn parse(arguments: &[&str]) -> Result<(Point3, Options), CommandError> {
    let (flags, positional): (Vec<_>, Vec<_>) =
        arguments.iter().copied().partition(|a| a.contains('='));
    let mut options = Options::default();
    options.update(&flags)?;
    let (apex, consumed) = parse_point(&positional)?;
    require_consumed(&positional, consumed, EXTRUDE_CURVE_TO_POINT_USAGE)?;
    Ok((apex, options))
}
