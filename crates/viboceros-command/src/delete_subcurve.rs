//! Curve portion deletion reuses native-domain trimming and numeric getters.
use super::*;
use crate::curve_domain::{SubcurveLocation, parse_subcurve_location};
pub const USAGE: &str = "DeleteSubCrv start_point end_point | Parameter=start,end | Numeric=anchor,length,confirmation [Locked=Forward|Backward]";

pub(super) struct DeleteSubcurveCommand;
impl Command for DeleteSubcurveCommand {
    fn name(&self) -> &'static str {
        "DeleteSubCrv"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["Break"]
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (location, mut index) = parse_subcurve_location(args, USAGE)?;
        let mut locked = None;
        while index < args.len() {
            let (name, value, consumed) = orient_option(args, index, USAGE)?;
            if !option_name_eq(name, "Locked") || locked.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            locked = Some(if option_name_eq(value, "Forward") {
                true
            } else if option_name_eq(value, "Backward") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            });
            index += consumed;
        }
        let sources = doc
            .selected_objects()
            .filter(|o| doc.is_object_selectable(o.id()))
            .filter_map(|o| {
                o.geometry()
                    .curve_ref()
                    .map(|c| (o.id(), c.to_owned(), o.geometry_user_text().clone()))
            })
            .collect::<Vec<_>>();
        let [(id, curve, text)] = sources.as_slice() else {
            return Err(CommandError::SubcurveRequiresOneCurve {
                actual: sources.len(),
            });
        };
        let pieces = prepare(curve, location, locked, doc.tolerance())?;
        let Some(pieces) = pieces else {
            doc.clear_selection();
            return Ok("No curve portion deleted".into());
        };
        let count = pieces.len();
        let plans = pieces
            .into_iter()
            .map(|(curve, keep_text)| {
                (
                    *id,
                    Geometry::from(curve),
                    if keep_text {
                        text.clone()
                    } else {
                        Default::default()
                    },
                )
            })
            .collect::<Vec<_>>();
        // New pieces inherit source attributes and groups. Source selection is
        // transient even when the input was selected before command startup.
        doc.release_command_selection_on_history_replay([*id])?;
        doc.clear_selection();
        doc.copy_object_pieces_with_metadata_into_source_groups(plans)?;
        doc.delete_object(*id)?;
        Ok(format!("Deleted curve portion; retained {count} piece(s)"))
    }
}

fn prepare(
    curve: &Curve3,
    location: SubcurveLocation,
    locked: Option<bool>,
    tolerance: Tolerance,
) -> Result<Option<Vec<(Curve3, bool)>>, CommandError> {
    let source = curve.as_ref();
    let domain = source.domain();
    let closed = source.is_closed()?;
    let interval = match location {
        SubcurveLocation::Numeric([anchor, length, confirmation]) => {
            if !domain.contains(&anchor) || !domain.contains(&confirmation) {
                return Err(GeometryError::InvalidCurveTrimInterval.into());
            }
            if length == 0. {
                return Ok(None);
            }
            let total = source.length(tolerance)?;
            if length.abs() > total + tolerance.absolute() {
                return Err(GeometryError::InvalidCurveTrimInterval.into());
            }
            let distance = length.abs();
            if closed && (distance - total).abs() <= tolerance.absolute() {
                if matches!(curve, Curve3::Circle(_) | Curve3::Arc(_)) {
                    return Ok(Some(vec![]));
                }
                if distance == total {
                    if anchor == *domain.start() || anchor == *domain.end() {
                        return Ok(Some(vec![(curve.clone(), true)]));
                    }
                    return Ok(Some(
                        curve_segments::split(curve, &[anchor])?
                            .into_iter()
                            .map(|c| (c, false))
                            .collect(),
                    ));
                }
                // Near a complete traversal, the scalar getter's remainder
                // is an open cut on the opposite side of the near-full arc.
                // Resolve that short remainder directly so the length solver
                // cannot stop at the already-coincident full-cycle endpoint.
                let remainder = (distance - total).abs();
                let forward = locked.unwrap_or(subcurve_input::forward(
                    source,
                    anchor,
                    confirmation,
                    tolerance,
                )?);
                let precise = Tolerance::try_new(
                    tolerance
                        .absolute()
                        .min(remainder * 1e-4)
                        .max(Real::MIN_POSITIVE),
                    tolerance.relative(),
                    tolerance.angular(),
                )?;
                let signed = if forward { -remainder } else { remainder };
                let Some((_, endpoint)) =
                    curve_segments::length_with_endpoint(curve, anchor, signed, precise)?
                else {
                    return Err(GeometryError::InvalidCurveTrimInterval.into());
                };
                let keep = curve_segments::directed(curve, endpoint, anchor)?;
                let wraps =
                    endpoint > anchor && anchor > *domain.start() && endpoint < *domain.end();
                return Ok(Some(vec![(keep, !wraps)]));
            }
            if closed {
                let forward = if let Some(forward) = locked {
                    forward
                } else if matches!(curve, Curve3::Polyline(_) | Curve3::PolyCurve(_)) {
                    subcurve_input::forward(source, anchor, confirmation, tolerance)?
                } else {
                    true
                };
                subcurve_input::interval_in_direction(source, anchor, distance, forward, tolerance)?
            } else if let Some(forward) = locked {
                subcurve_input::interval_in_direction(source, anchor, distance, forward, tolerance)?
            } else {
                subcurve_input::interval(source, anchor, confirmation, distance, tolerance)?
            }
        }
        SubcurveLocation::Parameters([a, b]) => {
            subcurve_input::point_parameters(source, a, b, locked, false)?
        }
        SubcurveLocation::Points(points) => subcurve_input::point_parameters(
            source,
            source.closest_parameter(points[0], tolerance)?,
            source.closest_parameter(points[1], tolerance)?,
            locked,
            true,
        )?,
    };
    let Some([mut start, mut end]) = interval else {
        return Ok(None);
    };
    if !domain.contains(&start) || !domain.contains(&end) {
        return Err(GeometryError::InvalidCurveTrimInterval.into());
    }
    if start == end {
        return Ok(None);
    }
    if !closed {
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        let mut pieces = Vec::with_capacity(2);
        if start > *domain.start() {
            pieces.push((
                curve_segments::trimmed(curve, *domain.start()..=start)?,
                true,
            ));
        }
        if end < *domain.end() {
            pieces.push((curve_segments::trimmed(curve, end..=*domain.end())?, true));
        }
        return Ok(Some(pieces));
    }
    if start == *domain.start() && end == *domain.end() {
        return Ok(Some(vec![]));
    }
    let wraps = end > start && start > *domain.start() && end < *domain.end();
    Ok(Some(vec![(
        curve_segments::directed(curve, end, start)?,
        !wraps,
    )]))
}

#[cfg(test)]
mod tests;
