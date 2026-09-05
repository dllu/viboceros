//! Representation-aware joining and closure commands.

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

pub(super) struct JoinCommand;

impl Command for JoinCommand {
    fn name(&self) -> &'static str {
        "Join"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Join")?;
        let inputs = selected_join_curves(document)?;
        if inputs.len() < 2 {
            return Err(CommandError::NotEnoughCurvesToJoin);
        }
        let curves = inputs
            .iter()
            .map(|input| input.curve.clone())
            .collect::<Vec<_>>();
        let components = join_curves(
            &curves,
            CurveJoinOptions {
                tolerance: document.tolerance().absolute(),
                preserve_direction: false,
                style: viboceros_geometry::CurveJoinStyle::Seeded,
            },
            document.tolerance(),
        )?;
        let replacements = components
            .iter()
            .filter(|component| component.source_indices().len() > 1)
            .map(|component| {
                let source_indices = component.source_indices().to_vec();
                let attributes = inputs[source_indices[0]].attributes.clone();
                (source_indices, component.curve().clone(), attributes)
            })
            .collect::<Vec<_>>();
        if replacements.is_empty() {
            return Err(CommandError::NoJoinableCurves);
        }

        let joined_curve_count = replacements
            .iter()
            .map(|(sources, _, _)| sources.len())
            .sum::<usize>();
        let unchanged = inputs.len() - joined_curve_count;
        let unchanged_ids = components
            .iter()
            .filter(|component| component.source_indices().len() == 1)
            .map(|component| inputs[component.source_indices()[0]].id)
            .collect::<Vec<_>>();
        let mut result_ids = Vec::with_capacity(replacements.len());
        for (sources, curve, attributes) in replacements {
            let groups = document
                .groups()
                .filter(|group| {
                    group
                        .members()
                        .any(|member| member == inputs[sources[0]].id)
                })
                .map(|group| group.id())
                .collect::<Vec<_>>();
            let id = document.add_geometry_with_attributes(Geometry::from(curve), attributes)?;
            for group in groups {
                document.add_group_members(group, [id])?;
            }
            for source in sources {
                document.delete_object(inputs[source].id)?;
            }
            result_ids.push(id);
        }
        replace_selection(
            document,
            unchanged_ids.into_iter().chain(result_ids.iter().copied()),
        )?;
        Ok(format!(
            "Joined {joined_curve_count} curve(s) into {} curve(s); {unchanged} curve(s) unchanged",
            result_ids.len()
        ))
    }
}

#[derive(Clone)]
struct SelectedJoinCurve {
    id: ObjectId,
    curve: Curve3,
    attributes: ObjectAttributes,
}

fn selected_join_curves(document: &Document) -> Result<Vec<SelectedJoinCurve>, CommandError> {
    let mut inputs = Vec::new();
    for object in document.selected_objects() {
        let curve = object
            .geometry()
            .curve_ref()
            .ok_or(CommandError::UnsupportedJoinGeometry)?
            .to_owned();
        inputs.push(SelectedJoinCurve {
            id: object.id(),
            curve,
            attributes: object.attributes().clone(),
        });
    }
    if inputs.is_empty() {
        Err(CommandError::NoObjectsSelected)
    } else {
        Ok(inputs)
    }
}
