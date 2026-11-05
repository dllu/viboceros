//! Straight-extrusion output and option parsing.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Output {
    #[default]
    Surface,
    SubD,
}
impl Output {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim_start_matches('_');
        if value.eq_ignore_ascii_case("Surface") {
            Some(Self::Surface)
        } else if value.eq_ignore_ascii_case("SubD") {
            Some(Self::SubD)
        } else {
            None
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Surface => "Surface",
            Self::SubD => "SubD",
        }
    }
}

/// Atomic flags shared by direct execution and both direction getters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub output: Output,
    pub delete_input: bool,
    pub both_sides: bool,
    pub solid: bool,
    pub split_at_tangents: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            output: Output::Surface,
            delete_input: false,
            both_sides: false,
            solid: false,
            split_at_tangents: true,
        }
    }
}
impl Options {
    pub fn update(
        &mut self,
        arguments: &[&str],
        split_available: bool,
    ) -> Result<(), CommandError> {
        let mut next = *self;
        let mut seen = std::collections::BTreeSet::new();
        for argument in arguments {
            let (name, value) = argument
                .split_once('=')
                .ok_or(CommandError::Usage(EXTRUDE_CURVE_USAGE))?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            if !seen.insert(name.clone()) {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
            if name == "output" {
                next.output =
                    Output::parse(value).ok_or(CommandError::UnsupportedCurveExtrusionOutput)?;
                continue;
            }
            let enabled = parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_USAGE))?;
            match name.as_str() {
                "bothsides" => next.both_sides = enabled,
                "deleteinput" => next.delete_input = enabled,
                "solid" => next.solid = enabled,
                "splitattangents" => next.split_at_tangents = enabled,
                _ => return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE)),
            }
        }
        if seen.contains("splitattangents") && (!split_available || next.output == Output::SubD) {
            return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
        }
        if next.output == Output::SubD {
            next.solid = false;
            next.split_at_tangents = false;
        }
        *self = next;
        Ok(())
    }
    pub fn command_options(self, split_available: bool) -> String {
        let yes = |b| if b { "Yes" } else { "No" };
        let mut options = format!(
            "Output={} BothSides={} DeleteInput={} Solid={}",
            self.output.name(),
            yes(self.both_sides),
            yes(self.delete_input),
            yes(self.solid)
        );
        if split_available && self.output == Output::Surface {
            options.push_str(&format!(" SplitAtTangents={}", yes(self.split_at_tangents)));
        }
        options
    }
}

pub(super) enum Direction {
    Numeric(Real),
    Explicit(Vector3),
}

pub(super) fn parse_curve_extrusion(
    arguments: &[&str],
    tolerance: Tolerance,
    split_available: bool,
    split_default: bool,
) -> Result<(Direction, Options), CommandError> {
    let (flags, positional): (Vec<_>, Vec<_>) =
        arguments.iter().copied().partition(|a| a.contains('='));
    let mut options = Options {
        split_at_tangents: split_default,
        ..Options::default()
    };
    options.update(&flags, split_available)?;
    let direction = if positional.len() == 1 && !positional[0].contains(',') {
        let distance = parse_finite_real(positional[0])?;
        Vector3::try_new(0., 0., distance)?.normalized(tolerance)?;
        Direction::Numeric(distance)
    } else {
        let (base, base_consumed) = parse_point(&positional)?;
        let (target, target_consumed) = parse_point(&positional[base_consumed..])?;
        require_consumed(
            &positional,
            base_consumed + target_consumed,
            EXTRUDE_CURVE_USAGE,
        )?;
        let direction = base.vector_to(target)?;
        direction.normalized(tolerance)?;
        Direction::Explicit(direction)
    };
    Ok((direction, options))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn straight_split_options_respect_availability_and_are_atomic() {
        let mut options = Options::default();
        assert!(options.split_at_tangents);
        options
            .update(&["BothSides=Yes", "Solid=Yes", "SplitAtTangents=No"], true)
            .unwrap();
        let before = options;
        for args in [
            vec!["SplitAtTangents=Maybe"],
            vec!["SplitAtTangents=No", "SplitAtTangents=Yes"],
            vec!["SplitAtTangents=Yes", "Output=Mesh"],
        ] {
            assert!(options.update(&args, true).is_err());
            assert_eq!(options, before);
        }
        assert!(options.update(&["SplitAtTangents=Yes"], false).is_err());
        assert_eq!(options, before);
        options.update(&["Output=SubD"], false).unwrap();
        assert!(!options.solid);
        assert!(!options.split_at_tangents);
        assert!(options.both_sides);
        options.update(&["Output=Surface"], true).unwrap();
        assert!(!options.split_at_tangents);
        let mut copy = Options::default();
        copy.update(
            &options
                .command_options(true)
                .split_whitespace()
                .collect::<Vec<_>>(),
            true,
        )
        .unwrap();
        assert_eq!(copy, options);
    }
}
