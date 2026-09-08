//! Shared value parsing for Curve commands and interactive drafts.

use super::{CURVE_USAGE, CommandError, MAX_CURVE_COMMAND_DEGREE};
use viboceros_geometry::ControlPointCurveClosure;

/// Parses Curve's requested degree, including its 1–11 clamping policy.
pub fn parse_curve_degree(value: &str) -> Result<usize, CommandError> {
    let value = value.trim_start_matches('_');
    value
        .parse::<usize>()
        .map(|degree| degree.clamp(1, MAX_CURVE_COMMAND_DEGREE))
        .map_err(|_| CommandError::InvalidInteger(value.to_owned()))
}

/// Parses Curve's closure value, not the prompt's immediate Close action.
pub fn parse_curve_closure(value: &str) -> Result<ControlPointCurveClosure, CommandError> {
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("Open") || value.eq_ignore_ascii_case("No") {
        Ok(ControlPointCurveClosure::Open)
    } else if value.eq_ignore_ascii_case("Smooth") || value.eq_ignore_ascii_case("Yes") {
        Ok(ControlPointCurveClosure::Smooth)
    } else if value.eq_ignore_ascii_case("Sharp") {
        Ok(ControlPointCurveClosure::Sharp)
    } else {
        Err(CommandError::Usage(CURVE_USAGE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degrees_are_integer_values_with_explicit_limits() {
        for (input, expected) in [("0", 1), ("1", 1), ("_3", 3), ("11", 11), ("999", 11)] {
            assert_eq!(parse_curve_degree(input).unwrap(), expected);
        }
        for input in [
            "",
            "_",
            "-1",
            "1.5",
            "NaN",
            "1 2",
            " 3",
            "3 ",
            "999999999999999999999999999",
        ] {
            assert!(parse_curve_degree(input).is_err(), "{input}");
        }
    }

    #[test]
    fn closure_values_are_case_insensitive_and_reject_extra_input() {
        for (input, expected) in [
            ("Open", ControlPointCurveClosure::Open),
            ("_nO", ControlPointCurveClosure::Open),
            ("Smooth", ControlPointCurveClosure::Smooth),
            ("_yEs", ControlPointCurveClosure::Smooth),
            ("_sHaRp", ControlPointCurveClosure::Sharp),
        ] {
            assert_eq!(parse_curve_closure(input).unwrap(), expected);
        }
        for input in ["", "_", "Closed", "1", "Open extra", " Open", "Open "] {
            assert!(parse_curve_closure(input).is_err(), "{input}");
        }
    }
}
