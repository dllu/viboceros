//! Native curve cutting, projected intersections, and interval replacement.

use super::*;

#[cfg(test)]
mod tests;

pub(super) const TRIM_CURVE_USAGE: &str =
    "Trim point [ApparentIntersections=Yes|No] [ViewNormal=x,y,z]";

#[derive(Clone, Copy, Debug, PartialEq)]
struct TrimCurveOptions {
    pick: Point3,
    apparent_intersections: bool,
    view_normal: Vector3,
}

pub(super) struct TrimCurveCommand;

pub(super) enum CurveCutterInput {
    Curve(NurbsCurve),
    Surface(NurbsSurface),
    Brep(Brep),
}

pub(super) fn selected_curve_cutter_inputs(
    document: &Document,
) -> Result<Vec<(ObjectId, CurveCutterInput)>, GeometryError> {
    document
        .selected_objects()
        .filter_map(|object| {
            let input = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    Some(Ok(CurveCutterInput::Surface(surface.clone())))
                }
                Geometry::Brep(brep) => Some(Ok(CurveCutterInput::Brep(brep.clone()))),
                geometry => geometry
                    .nurbs_curve_representation()
                    .transpose()
                    .map(|curve| curve.map(CurveCutterInput::Curve)),
            };
            input.map(|input| input.map(|input| (object.id(), input)))
        })
        .collect()
}

pub(super) fn split_curve_with_cutters(
    document: &mut Document,
    candidates: &[(ObjectId, CurveCutterInput)],
    source_index: usize,
    source_id: ObjectId,
    source: &NurbsCurve,
) -> Result<String, CommandError> {
    let native = document
        .object(source_id)
        .and_then(|object| object.geometry().curve_ref())
        .expect("a cutting Split curve source has native geometry")
        .to_owned();
    let mut intersections = Vec::new();
    for (index, (_, cutter)) in candidates.iter().enumerate() {
        if index == source_index {
            continue;
        }
        append_curve_cutter_intersections(
            &mut intersections,
            CurveCutIntersectionContext {
                source,
                intersection_source: source,
                projection: None,
                tolerance: document.tolerance(),
                limit: CurveCutIntersectionLimit::Split,
            },
            cutter,
        )?;
    }
    intersections.sort_by(|left, right| left.0.total_cmp(&right.0));
    let parameters = intersections
        .into_iter()
        .map(|(parameter, _)| native.as_ref().parameter_from_nurbs(parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let closed = native.as_ref().is_closed()?;
    if parameters.is_empty() || (closed && parameters.len() < 2) {
        document.select_objects_direct([source_id], SelectionMode::Replace)?;
        return Ok("No cutting intersection was available to split the curve".to_owned());
    }
    let mut pieces = Vec::new();
    if closed {
        for index in 0..parameters.len() {
            pieces.push(curve_cut_piece(
                &native,
                parameters[index],
                parameters[(index + 1) % parameters.len()],
            )?);
        }
    } else {
        let domain = native.as_ref().domain();
        let mut start = *domain.start();
        for end in parameters.iter().copied().chain([*domain.end()]) {
            pieces.push(native.try_trimmed(start..=end)?);
            start = end;
        }
    }
    replace_curve_split_pieces(
        document,
        source_id,
        pieces,
        parameters.len(),
        CurveSplitReplacement::ReplaceAll,
    )
}

/// Interactive cutting commands retain the original closed representation by
/// relocating its seam before trimming a wrapped interval. The generic Rhino
/// Curve.Trim/Curve.Split API instead returns a two-part polycurve at the seam.
fn curve_cut_piece(source: &Curve3, start: Real, end: Real) -> Result<Curve3, GeometryError> {
    if start < end {
        return source.try_trimmed(start..=end);
    }
    let domain = source.as_ref().domain();
    if end == *domain.start() && start > end {
        return source.try_trimmed(start..=*domain.end());
    }
    let moved = source.try_change_closed_seam(start)?;
    if start == end {
        return Ok(moved);
    }
    moved.try_trimmed(start..=(end + (domain.end() - domain.start())))
}

impl Command for TrimCurveCommand {
    fn name(&self) -> &'static str {
        "Trim"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_trim_curve_options(arguments)?;

        let candidates = selected_curve_cutter_inputs(document)?;
        if candidates.len() < 2 {
            return Err(CommandError::TrimRequiresSourceAndCuttingObjects {
                actual: candidates.len(),
            });
        }

        let (projection, intersection_pick) = if options.apparent_intersections {
            let origin = Point3::try_new(0.0, 0.0, 0.0)?;
            let normal = options.view_normal.normalized(document.tolerance())?;
            let projection = AffineTransform3::try_planar_projection(Plane::new(origin, normal))?;
            (Some(projection), projection.transform_point(options.pick)?)
        } else {
            (None, options.pick)
        };

        let mut picked = None;
        for (index, (_, input)) in candidates.iter().enumerate() {
            let CurveCutterInput::Curve(curve) = input else {
                continue;
            };
            let intersection_curve = if let Some(projection) = projection {
                curve.transformed(projection)?
            } else {
                curve.clone()
            };
            let parameter =
                intersection_curve.closest_parameter(intersection_pick, document.tolerance())?;
            let distance = intersection_curve
                .evaluate(parameter)?
                .distance_to(intersection_pick)?;
            if picked
                .as_ref()
                .is_none_or(|(_, _, _, nearest_distance)| distance < *nearest_distance)
            {
                picked = Some((index, intersection_curve, parameter, distance));
            }
        }
        let Some((source_index, intersection_source, picked_parameter, _)) = picked else {
            return Err(CommandError::TrimRequiresCurveSource);
        };
        let (source_id, CurveCutterInput::Curve(source)) = &candidates[source_index] else {
            unreachable!("only selected curves participate in Trim source picking")
        };
        let native = document
            .object(*source_id)
            .and_then(|object| object.geometry().curve_ref())
            .expect("a Trim curve source has native geometry")
            .to_owned();
        let picked_parameter = native.as_ref().parameter_from_nurbs(picked_parameter)?;

        let domain = source.domain();
        let domain_start = *domain.start();
        let domain_end = *domain.end();
        let mut intersections = Vec::new();
        for (index, (_, cutter)) in candidates.iter().enumerate() {
            if index == source_index {
                continue;
            }
            append_curve_cutter_intersections(
                &mut intersections,
                CurveCutIntersectionContext {
                    source,
                    intersection_source: &intersection_source,
                    projection,
                    tolerance: document.tolerance(),
                    limit: CurveCutIntersectionLimit::Trim,
                },
                cutter,
            )?;
        }
        intersections.sort_by(|left, right| left.0.total_cmp(&right.0));
        let parameters = intersections
            .into_iter()
            .map(|(parameter, _)| native.as_ref().parameter_from_nurbs(parameter))
            .collect::<Result<Vec<_>, _>>()?;

        let closed = source.is_closed()?;
        if parameters.is_empty() || (closed && parameters.len() < 2) {
            document.select_objects_direct([*source_id], SelectionMode::Replace)?;
            return Ok("No bounded curve interval was available to trim".to_owned());
        }
        let next_index = parameters.partition_point(|parameter| *parameter <= picked_parameter);
        let kept = if closed {
            let previous = parameters[(next_index + parameters.len() - 1) % parameters.len()];
            let next = parameters[next_index % parameters.len()];
            vec![curve_cut_piece(&native, next, previous)?]
        } else {
            let mut kept = Vec::with_capacity(2);
            if let Some(previous) = next_index.checked_sub(1).map(|index| parameters[index]) {
                kept.push(native.try_trimmed(domain_start..=previous)?);
            }
            if let Some(next) = parameters.get(next_index).copied() {
                kept.push(native.try_trimmed(next..=domain_end)?);
            }
            kept
        };
        debug_assert!(
            !kept.is_empty(),
            "a bounded trim retains at least one piece"
        );

        let source_object = document
            .object(*source_id)
            .expect("selected Trim source belongs to the document");
        let attributes = source_object.attributes().clone();
        let group_ids = document
            .groups()
            .filter(|group| group.members().any(|member| member == *source_id))
            .map(|group| group.id())
            .collect::<Vec<_>>();

        let output_ids = if let [piece] = kept.as_slice() {
            document.replace_object_geometries([(*source_id, Geometry::from(piece.clone()))])?;
            vec![*source_id]
        } else {
            let mut output_ids = Vec::with_capacity(kept.len());
            for piece in kept {
                output_ids.push(
                    document
                        .add_geometry_with_attributes(Geometry::from(piece), attributes.clone())?,
                );
            }
            for group_id in group_ids {
                document.add_group_members(group_id, output_ids.iter().copied())?;
            }
            document.delete_object(*source_id)?;
            output_ids
        };
        let output_count = output_ids.len();
        document.select_objects_direct(output_ids, SelectionMode::Replace)?;
        Ok(format!(
            "Trimmed one curve interval and retained {output_count} exact curve piece(s)"
        ))
    }
}

#[derive(Clone, Copy)]
enum CurveCutIntersectionLimit {
    Split,
    Trim,
}

#[derive(Clone, Copy)]
struct CurveCutIntersectionContext<'a> {
    source: &'a NurbsCurve,
    intersection_source: &'a NurbsCurve,
    projection: Option<AffineTransform3>,
    tolerance: Tolerance,
    limit: CurveCutIntersectionLimit,
}

fn append_curve_cutter_intersections(
    intersections: &mut Vec<(Real, Point3)>,
    context: CurveCutIntersectionContext<'_>,
    cutter: &CurveCutterInput,
) -> Result<(), CommandError> {
    let CurveCutIntersectionContext {
        source,
        intersection_source,
        projection,
        tolerance,
        limit,
    } = context;
    match cutter {
        CurveCutterInput::Curve(cutter) => {
            let intersection_cutter = if let Some(projection) = projection {
                cutter.transformed(projection)?
            } else {
                cutter.clone()
            };
            for intersection in
                intersection_source.intersections_with_curve(&intersection_cutter, tolerance)?
            {
                push_curve_cut_intersection(
                    intersections,
                    source,
                    intersection.first_parameter(),
                    tolerance,
                    limit,
                )?;
            }
        }
        CurveCutterInput::Surface(surface) => {
            let transformed_surface;
            let intersection_surface = if let Some(projection) = projection {
                transformed_surface = surface.transformed(projection)?;
                &transformed_surface
            } else {
                surface
            };
            for event in curve_surface_intersection_events(
                intersection_source,
                intersection_surface,
                tolerance,
            )? {
                match event {
                    CurveSurfaceIntersectionEvent::Point(intersection) => {
                        push_curve_cut_intersection(
                            intersections,
                            source,
                            intersection.curve_parameter(),
                            tolerance,
                            limit,
                        )?;
                    }
                    CurveSurfaceIntersectionEvent::Overlap(overlap) => {
                        for intersection in [overlap.start(), overlap.end()] {
                            push_curve_cut_intersection(
                                intersections,
                                source,
                                intersection.curve_parameter(),
                                tolerance,
                                limit,
                            )?;
                        }
                    }
                }
            }
        }
        CurveCutterInput::Brep(brep) => {
            let events = if let Some(projection) = projection {
                transformed_curve_brep_intersection_events(source, brep, projection, tolerance)?
            } else {
                curve_brep_intersection_events(source, brep, tolerance)?
            };
            for event in events {
                match event {
                    CurveBrepIntersectionEvent::Point(intersection) => {
                        push_curve_cut_intersection(
                            intersections,
                            source,
                            intersection.curve_parameter(),
                            tolerance,
                            limit,
                        )?;
                    }
                    CurveBrepIntersectionEvent::Overlap(overlap) => {
                        for intersection in [overlap.start(), overlap.end()] {
                            push_curve_cut_intersection(
                                intersections,
                                source,
                                intersection.curve_parameter(),
                                tolerance,
                                limit,
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn push_curve_cut_intersection(
    intersections: &mut Vec<(Real, Point3)>,
    source: &NurbsCurve,
    parameter: Real,
    tolerance: Tolerance,
    limit: CurveCutIntersectionLimit,
) -> Result<(), CommandError> {
    let domain = source.domain();
    let start = (*domain.start(), source.evaluate(*domain.start())?);
    let end = (*domain.end(), source.evaluate(*domain.end())?);
    let mut intersection = (parameter, source.evaluate(parameter)?);
    // Finite NURBS endpoints can have an unrepresentable full-width difference.
    let half_span = domain.end() * 0.5 - domain.start() * 0.5;
    let at_start = trim_intersections_near(intersection, start, half_span, tolerance);
    let at_end = trim_intersections_near(intersection, end, half_span, tolerance);
    if at_start || at_end {
        if !source.is_closed()? {
            return Ok(());
        }
        // A closed seam is a real cut station, not an open endpoint to discard.
        // The two parameter endpoints refer to that one station.
        intersection = start;
    }
    if intersections
        .iter()
        .any(|existing| trim_intersections_near(*existing, intersection, half_span, tolerance))
    {
        return Ok(());
    }
    match limit {
        CurveCutIntersectionLimit::Split
            if intersections.len().saturating_add(1) >= MAX_SPAN_OUTPUT_OBJECTS =>
        {
            return Err(too_many_span_outputs("Split"));
        }
        CurveCutIntersectionLimit::Trim if intersections.len() == MAX_CURVE_TRIM_INTERSECTIONS => {
            return Err(CommandError::TooManyTrimIntersections {
                maximum: MAX_CURVE_TRIM_INTERSECTIONS,
            });
        }
        CurveCutIntersectionLimit::Split | CurveCutIntersectionLimit::Trim => {}
    }
    intersections.push(intersection);
    Ok(())
}

fn parse_trim_curve_options(arguments: &[&str]) -> Result<TrimCurveOptions, CommandError> {
    let (pick, consumed) = parse_point(arguments).map_err(|error| match error {
        CommandError::Usage(_) => CommandError::Usage(TRIM_CURVE_USAGE),
        error => error,
    })?;
    let mut options = TrimCurveOptions {
        pick,
        apparent_intersections: true,
        view_normal: Vector3::try_new(0.0, 0.0, 1.0).expect("the world Z direction is finite"),
    };
    let mut seen = BTreeSet::new();
    let mut index = consumed;
    while index < arguments.len() {
        let (name, value, option_consumed) = orient_option(arguments, index, TRIM_CURVE_USAGE)?;
        let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
        if !seen.insert(name.clone()) {
            return Err(CommandError::Usage(TRIM_CURVE_USAGE));
        }
        match name.as_str() {
            "apparentintersections" | "apparent" => {
                options.apparent_intersections =
                    parse_yes_no(value).ok_or(CommandError::Usage(TRIM_CURVE_USAGE))?;
            }
            "viewnormal" => {
                options.view_normal = Vector3::try_from(
                    parse_single_option_point(value, TRIM_CURVE_USAGE)?.to_array(),
                )?;
            }
            _ => return Err(CommandError::Usage(TRIM_CURVE_USAGE)),
        }
        index += option_consumed;
    }
    Ok(options)
}

fn trim_intersections_near(
    left: (Real, Point3),
    right: (Real, Point3),
    domain_half_span: Real,
    tolerance: Tolerance,
) -> bool {
    let roundoff = Real::EPSILON * left.0.abs().max(right.0.abs()) * 8.0;
    let parameter_epsilon = (16.0 * Real::EPSILON.sqrt() * domain_half_span).max(roundoff);
    if (left.0 - right.0).abs() > parameter_epsilon {
        return false;
    }
    model_points_near(left.1, right.1, tolerance)
}
