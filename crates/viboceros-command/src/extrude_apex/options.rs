//! Atomic options shared by direct commands and the interactive apex getter.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub output: CurveExtrusionOutput,
    pub delete_input: bool,
    pub solid: bool,
    pub split_at_tangents: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            output: CurveExtrusionOutput::Surface,
            delete_input: false,
            solid: false,
            split_at_tangents: true,
        }
    }
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
                "splitattangents" => {
                    next.split_at_tangents = parse_yes_no(value)
                        .ok_or(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE))?
                }
                _ => return Err(CommandError::Usage(EXTRUDE_CURVE_TO_POINT_USAGE)),
            }
        }
        if next.output == CurveExtrusionOutput::SubD {
            next.split_at_tangents = false;
            next.solid = false;
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self) -> String {
        format!(
            "Output={} Solid={} DeleteInput={} SplitAtTangents={}",
            self.output.name(),
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" },
            if self.split_at_tangents { "Yes" } else { "No" }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apex_split_options_default_and_mode_transitions_are_atomic() {
        let mut options = Options::default();
        assert!(options.split_at_tangents);
        options.update(&["SplitAtTangents=No"]).unwrap();
        assert!(!options.split_at_tangents);
        options
            .update(&["SplitAtTangents=Yes", "Solid=Yes"])
            .unwrap();
        let original = options;
        for arguments in [
            vec!["SplitAtTangents=Yes", "SplitAtTangents=No"],
            vec!["Output=SubD", "SplitAtTangents=Maybe"],
        ] {
            assert!(options.update(&arguments).is_err());
            assert_eq!(options, original);
        }
        options.update(&["Output=SubD"]).unwrap();
        assert!(!options.split_at_tangents);
        options.update(&["Output=Surface"]).unwrap();
        assert!(!options.split_at_tangents);
        assert!(!options.solid);
        options.update(&["_SplitAtTangents=_Yes"]).unwrap();
        assert!(options.split_at_tangents);
        let mut replay = Options::default();
        replay
            .update(
                &options
                    .command_options()
                    .split_whitespace()
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        assert_eq!(replay, options);
    }
}
