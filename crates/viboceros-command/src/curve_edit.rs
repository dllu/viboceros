//! Representation-aware curve closure command.

use super::*;

#[cfg(test)]
mod tests;

const CLOSE_CRV_USAGE: &str = "CloseCrv [CloseWideGapsWithLine=Yes|No] [Tolerance=value]";

pub(super) struct CloseCrvCommand;

impl Command for CloseCrvCommand {
    fn name(&self) -> &'static str {
        "CloseCrv"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_close_curve_arguments(arguments, document.tolerance().absolute())?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry().clone()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mut endpoint_moved = 0;
        let mut segment_added = 0;
        let mut unchanged = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in &selected {
            let curve = geometry
                .curve_ref()
                .ok_or(CommandError::UnsupportedCloseCurveGeometry)?
                .to_owned();
            let (closed, outcome) = curve.close(
                options.tolerance,
                options.close_wide_gaps_with_line,
                document.tolerance(),
            )?;
            match outcome {
                CurveClosure::EndpointMoved => {
                    endpoint_moved += 1;
                    replacements.push((*id, Geometry::from(closed)));
                }
                CurveClosure::SegmentAdded => {
                    segment_added += 1;
                    replacements.push((*id, Geometry::from(closed)));
                }
                CurveClosure::AlreadyClosed
                | CurveClosure::GapTooWide
                | CurveClosure::NotClosable => unchanged += 1,
            }
        }

        let closed = document.replace_object_geometries(replacements)?;
        debug_assert_eq!(closed, endpoint_moved + segment_added);
        Ok(format!(
            "Closed {closed} of {} selected curve(s): {segment_added} with a line, {endpoint_moved} by moving an endpoint; {unchanged} unchanged",
            selected.len()
        ))
    }
}

#[derive(Clone, Copy)]
struct CloseCurveOptions {
    close_wide_gaps_with_line: bool,
    tolerance: Real,
}

fn parse_close_curve_arguments(
    arguments: &[&str],
    default_tolerance: Real,
) -> Result<CloseCurveOptions, CommandError> {
    let mut options = CloseCurveOptions {
        close_wide_gaps_with_line: true,
        tolerance: default_tolerance,
    };
    let mut wide_gap_seen = false;
    let mut tolerance_seen = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else {
            let value = arguments
                .get(index + 1)
                .ok_or(CommandError::Usage(CLOSE_CRV_USAGE))?;
            (argument, *value, 2)
        };
        if name.eq_ignore_ascii_case("CloseWideGapsWithLine") && !wide_gap_seen {
            options.close_wide_gaps_with_line =
                parse_yes_no(value).ok_or(CommandError::Usage(CLOSE_CRV_USAGE))?;
            wide_gap_seen = true;
        } else if name.eq_ignore_ascii_case("Tolerance") && !tolerance_seen {
            options.tolerance = parse_finite_real(value)?;
            if options.tolerance < 0.0 {
                return Err(GeometryError::InvalidCurveClosureTolerance.into());
            }
            tolerance_seen = true;
        } else {
            return Err(CommandError::Usage(CLOSE_CRV_USAGE));
        }
        index += consumed;
    }
    Ok(options)
}
