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

pub(super) enum Direction {
    Numeric(Real),
    Explicit(Vector3),
}
pub(super) struct Options {
    pub(super) output: Output,
    pub(super) direction: Direction,
    pub(super) delete_input: bool,
    pub(super) both_sides: bool,
    pub(super) solid: bool,
}

pub(super) fn parse_curve_extrusion(
    arguments: &[&str],
    tolerance: Tolerance,
) -> Result<Options, CommandError> {
    let mut positional = Vec::new();
    let mut both_sides = None;
    let mut delete_input = None;
    let mut output = None;
    let mut solid = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            positional.push(*argument);
            continue;
        };
        if option_name_eq(name, "BothSides") && both_sides.is_none() {
            both_sides = parse_yes_no(value);
            if both_sides.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = parse_yes_no(value);
            if delete_input.is_none() {
                return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
            }
        } else if option_name_eq(name, "Output") && output.is_none() {
            output =
                Some(Output::parse(value).ok_or(CommandError::UnsupportedCurveExtrusionOutput)?);
        } else if option_name_eq(name, "Solid") && solid.is_none() {
            solid = Some(parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_USAGE))?);
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_USAGE));
        }
    }

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
    Ok(Options {
        output: output.unwrap_or_default(),
        direction,
        delete_input: delete_input.unwrap_or(false),
        both_sides: both_sides.unwrap_or(false),
        solid: solid.unwrap_or(false),
    })
}
