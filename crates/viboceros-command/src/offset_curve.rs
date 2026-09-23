//! Analytic, smooth, and planar polyline offsets with corner and layer choices.

use super::*;

#[cfg(test)]
mod multiple_tests;
#[cfg(test)]
mod tests;

const USAGE: &str = "Offset distance side-point [BothSides=Yes|No] [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input] | Offset distance BothSides=Yes [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input] | Offset ThroughPoint=point [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input]";

pub(super) struct OffsetCommand;
pub(super) struct OffsetMultipleCommand;

const MULTIPLE_USAGE: &str = "OffsetMultiple distance side-point [OffsetCount=2] [Corner=Sharp|Chamfer|Round|None] [OutputLayer=Current|Input]";
const MAX_OFFSET_OUTPUTS: usize = 100_000;

impl Command for OffsetMultipleCommand {
    fn name(&self) -> &'static str {
        "OffsetMultiple"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let options = parse_multiple(arguments)?;
        let selected_count = document.selected_object_count();
        if selected_count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if selected_count
            .checked_mul(options.count)
            .is_none_or(|total| total > MAX_OFFSET_OUTPUTS)
        {
            return Err(CommandError::Usage(MULTIPLE_USAGE));
        }
        let normal = context.construction_plane.z_axis();
        let tolerance = document.tolerance();
        let sources = document
            .selected_objects()
            .map(|object| {
                let curve = match object.geometry() {
                    Geometry::Line(line) => Curve3::Line(*line),
                    Geometry::Circle(circle) => Curve3::Circle(*circle),
                    Geometry::Arc(arc) => Curve3::Arc(*arc),
                    Geometry::Ellipse(ellipse) => Curve3::Ellipse(*ellipse),
                    Geometry::Polyline(polyline) => Curve3::Polyline(polyline.clone()),
                    Geometry::NurbsCurve(curve) => Curve3::NurbsCurve(curve.clone()),
                    Geometry::PolyCurve(curve) => Curve3::PolyCurve(curve.clone()),
                    _ => return Err(CommandError::UnsupportedOffsetGeometry),
                };
                let attributes = ObjectAttributes::on_layer(if options.input_layer {
                    object.attributes().layer_id()
                } else {
                    document.current_layer_id()
                });
                Ok((curve, attributes))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let inward = sources
            .iter()
            .map(|(curve, _)| {
                curve
                    .offset_region_inward_sign(normal, tolerance)
                    .map_err(map_offset_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let closed = inward
            .iter()
            .enumerate()
            .filter_map(|(i, sign)| sign.map(|_| i))
            .collect::<Vec<_>>();
        // Region nesting has an unambiguous depth only when selected boundaries
        // do not cross or touch. Check this before creating any document objects.
        let mut nurbs: Vec<Option<NurbsCurve>> = vec![None; closed.len()];
        for i in 0..closed.len() {
            for j in i + 1..closed.len() {
                let first = &sources[closed[i]].0;
                let second = &sources[closed[j]].0;
                let intersects = if let Some(intersects) =
                    first.offset_region_boundary_relation(second, tolerance)?
                {
                    intersects
                } else {
                    if nurbs[i].is_none() {
                        nurbs[i] = Some(first.as_ref().to_nurbs().map_err(map_offset_error)?);
                    }
                    if nurbs[j].is_none() {
                        nurbs[j] = Some(second.as_ref().to_nurbs().map_err(map_offset_error)?);
                    }
                    !nurbs[i]
                        .as_ref()
                        .expect("cached first curve")
                        .intersection_events_with_curve(
                            nurbs[j].as_ref().expect("cached second curve"),
                            tolerance,
                        )?
                        .is_empty()
                };
                if intersects {
                    return Err(GeometryError::IntersectingOffsetRegions.into());
                }
            }
        }
        let mut inside_any = false;
        for &i in &closed {
            inside_any |= sources[i]
                .0
                .offset_region_contains(options.side, normal, tolerance)?
                .unwrap_or(false);
        }
        let mut depths = vec![0_usize; sources.len()];
        for &i in &closed {
            let witness = sources[i]
                .0
                .offset_region_boundary_point()?
                .expect("closed region witness");
            for &j in &closed {
                if i != j
                    && sources[j]
                        .0
                        .offset_region_contains(witness, normal, tolerance)?
                        .unwrap_or(false)
                {
                    depths[i] += 1;
                }
            }
        }
        let mut outputs = Vec::with_capacity(selected_count * options.count);
        for (i, (curve, attributes)) in sources.iter().enumerate() {
            let sign = if let Some(inward_sign) = inward[i] {
                inward_sign
                    * if inside_any { 1.0 } else { -1.0 }
                    * if depths[i] % 2 == 0 { 1.0 } else { -1.0 }
            } else {
                curve
                    .offset_side(options.side, normal, tolerance)
                    .map_err(map_offset_error)?
            };
            for step in 1..=options.count {
                let distance = sign * options.distance * step as Real;
                let parts = curve
                    .try_offset_parts(distance, normal, tolerance, options.corner)
                    .map_err(map_offset_error)?;
                if outputs
                    .len()
                    .checked_add(parts.len())
                    .is_none_or(|total| total > MAX_OFFSET_OUTPUTS)
                {
                    return Err(CommandError::Usage(MULTIPLE_USAGE));
                }
                outputs.extend(
                    parts
                        .into_iter()
                        .map(|part| (Geometry::from(part), attributes.clone())),
                );
            }
        }
        let count = outputs.len();
        let mut ids = Vec::with_capacity(count);
        for (geometry, attributes) in outputs {
            ids.push(document.add_geometry_with_attributes(geometry, attributes)?);
        }
        document.select_objects_direct(ids, SelectionMode::Replace)?;
        Ok(format!("Created {count} offset curve(s)"))
    }
}

#[derive(Clone, Copy)]
struct MultipleOptions {
    distance: Real,
    side: Point3,
    count: usize,
    corner: CurveOffsetCornerStyle,
    input_layer: bool,
}

fn parse_multiple(arguments: &[&str]) -> Result<MultipleOptions, CommandError> {
    let distance = parse_finite_real(
        arguments
            .first()
            .ok_or(CommandError::Usage(MULTIPLE_USAGE))?,
    )?;
    if distance <= 0.0 {
        return Err(GeometryError::InvalidCurveOffsetDistance.into());
    }
    let (side, consumed) = parse_point(
        arguments
            .get(1..)
            .ok_or(CommandError::Usage(MULTIPLE_USAGE))?,
    )?;
    let mut options = MultipleOptions {
        distance,
        side,
        count: 2,
        corner: CurveOffsetCornerStyle::Sharp,
        input_layer: false,
    };
    let mut seen = [false; 3];
    let mut index = 1 + consumed;
    while index < arguments.len() {
        let (name, value, consumed) = if let Some((name, value)) = arguments[index].split_once('=')
        {
            (name, value, 1)
        } else {
            (
                arguments[index],
                *arguments
                    .get(index + 1)
                    .ok_or(CommandError::Usage(MULTIPLE_USAGE))?,
                2,
            )
        };
        let slot = if name.eq_ignore_ascii_case("OffsetCount") {
            0
        } else if name.eq_ignore_ascii_case("Corner") {
            1
        } else if name.eq_ignore_ascii_case("OutputLayer") {
            2
        } else {
            return Err(CommandError::Usage(MULTIPLE_USAGE));
        };
        if seen[slot] {
            return Err(CommandError::Usage(MULTIPLE_USAGE));
        }
        seen[slot] = true;
        match slot {
            0 => {
                options.count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n > 0 && n <= MAX_OFFSET_OUTPUTS)
                    .ok_or(CommandError::Usage(MULTIPLE_USAGE))?
            }
            1 => {
                options.corner = if value.eq_ignore_ascii_case("Sharp") {
                    CurveOffsetCornerStyle::Sharp
                } else if value.eq_ignore_ascii_case("Chamfer") {
                    CurveOffsetCornerStyle::Chamfer
                } else if value.eq_ignore_ascii_case("Round") {
                    CurveOffsetCornerStyle::Round
                } else if value.eq_ignore_ascii_case("None") {
                    CurveOffsetCornerStyle::None
                } else {
                    return Err(CommandError::Usage(MULTIPLE_USAGE));
                }
            }
            _ => {
                options.input_layer = if value.eq_ignore_ascii_case("Input") {
                    true
                } else if value.eq_ignore_ascii_case("Current") {
                    false
                } else {
                    return Err(CommandError::Usage(MULTIPLE_USAGE));
                }
            }
        }
        index += consumed;
    }
    Ok(options)
}

impl Command for OffsetCommand {
    fn name(&self) -> &'static str {
        "Offset"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let options = parse(arguments)?;
        let selected_count = document.selected_object_count();
        if selected_count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let normal = context.construction_plane.z_axis();
        let mut outputs =
            Vec::with_capacity(selected_count.saturating_mul(1 + usize::from(options.both_sides)));
        for object in document.selected_objects() {
            let curve = match object.geometry() {
                Geometry::Line(line) => Curve3::Line(*line),
                Geometry::Circle(circle) => Curve3::Circle(*circle),
                Geometry::Arc(arc) => Curve3::Arc(*arc),
                Geometry::Ellipse(ellipse) => Curve3::Ellipse(*ellipse),
                Geometry::Polyline(polyline) => Curve3::Polyline(polyline.clone()),
                Geometry::NurbsCurve(curve) => Curve3::NurbsCurve(curve.clone()),
                Geometry::PolyCurve(curve) => Curve3::PolyCurve(curve.clone()),
                _ => return Err(CommandError::UnsupportedOffsetGeometry),
            };
            let output_attributes = if options.input_layer {
                ObjectAttributes::on_layer(object.attributes().layer_id())
            } else {
                ObjectAttributes::on_layer(document.current_layer_id())
            };
            // Offset creates fresh objects. A source's name and overrides are
            // not silently propagated along with its layer.
            match options.mode {
                OffsetMode::Distance(distance) => {
                    let sign = if options.both_sides {
                        1.0
                    } else {
                        curve
                            .offset_side(
                                options.side.expect("validated side point"),
                                normal,
                                document.tolerance(),
                            )
                            .map_err(map_offset_error)?
                    };
                    let distances = [sign * distance, -distance];
                    let count = 1 + usize::from(options.both_sides);
                    for &signed_distance in &distances[..count] {
                        let parts = curve
                            .try_offset_parts(
                                signed_distance,
                                normal,
                                document.tolerance(),
                                options.corner,
                            )
                            .map_err(map_offset_error)?;
                        if outputs
                            .len()
                            .checked_add(parts.len())
                            .is_none_or(|total| total > MAX_OFFSET_OUTPUTS)
                        {
                            return Err(CommandError::Usage(USAGE));
                        }
                        outputs.extend(
                            parts
                                .into_iter()
                                .map(|part| (Geometry::from(part), output_attributes.clone())),
                        );
                    }
                }
                OffsetMode::ThroughPoint(point) => {
                    let (_, parts) = curve
                        .try_offset_through_point(
                            point,
                            normal,
                            document.tolerance(),
                            options.corner,
                        )
                        .map_err(map_offset_error)?;
                    if outputs
                        .len()
                        .checked_add(parts.len())
                        .is_none_or(|total| total > MAX_OFFSET_OUTPUTS)
                    {
                        return Err(CommandError::Usage(USAGE));
                    }
                    outputs.extend(
                        parts
                            .into_iter()
                            .map(|part| (Geometry::from(part), output_attributes.clone())),
                    );
                }
            }
        }
        let count = outputs.len();
        let mut ids = Vec::with_capacity(count);
        for (geometry, attributes) in outputs {
            ids.push(document.add_geometry_with_attributes(geometry, attributes)?);
        }
        document.select_objects_direct(ids, SelectionMode::Replace)?;
        Ok(match options.mode {
            OffsetMode::Distance(distance) => {
                format!("Created {count} offset curve(s) at distance {distance}",)
            }
            OffsetMode::ThroughPoint(point) => format!(
                "Created {count} offset curve(s) through {}",
                format_point(point),
            ),
        })
    }
}

fn map_offset_error(error: GeometryError) -> CommandError {
    if error == GeometryError::UnsupportedCurveOffset {
        CommandError::UnsupportedOffsetGeometry
    } else {
        error.into()
    }
}

#[derive(Clone, Copy)]
struct Options {
    mode: OffsetMode,
    side: Option<Point3>,
    both_sides: bool,
    input_layer: bool,
    corner: CurveOffsetCornerStyle,
}

#[derive(Clone, Copy)]
enum OffsetMode {
    Distance(Real),
    ThroughPoint(Point3),
}

fn parse(arguments: &[&str]) -> Result<Options, CommandError> {
    let first = *arguments.first().ok_or(CommandError::Usage(USAGE))?;
    let (mode, first_consumed) = if first.eq_ignore_ascii_case("ThroughPoint") {
        let (point, consumed) = parse_point(&arguments[1..])?;
        (OffsetMode::ThroughPoint(point), 1 + consumed)
    } else if let Some((name, value)) = first.split_once('=') {
        if !name.eq_ignore_ascii_case("ThroughPoint") {
            return Err(CommandError::Usage(USAGE));
        }
        let (point, _) = parse_point(&[value])?;
        (OffsetMode::ThroughPoint(point), 1)
    } else {
        let distance = parse_finite_real(first)?;
        if distance <= 0.0 {
            return Err(GeometryError::InvalidCurveOffsetDistance.into());
        }
        (OffsetMode::Distance(distance), 1)
    };
    let mut options = Options {
        mode,
        side: None,
        both_sides: false,
        input_layer: false,
        corner: CurveOffsetCornerStyle::Sharp,
    };
    let mut both_seen = false;
    let mut layer_seen = false;
    let mut corner_seen = false;
    let mut index = first_consumed;
    while index < arguments.len() {
        let argument = arguments[index];
        let (name, value, consumed) = if let Some((name, value)) = argument.split_once('=') {
            (name, value, 1)
        } else if argument.eq_ignore_ascii_case("BothSides")
            || argument.eq_ignore_ascii_case("OutputLayer")
            || argument.eq_ignore_ascii_case("Corner")
        {
            (
                argument,
                *arguments.get(index + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        } else {
            if options.side.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            let (point, consumed) = parse_point(&arguments[index..])?;
            options.side = Some(point);
            index += consumed;
            continue;
        };
        if name.eq_ignore_ascii_case("BothSides") && !both_seen {
            options.both_sides = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            both_seen = true;
        } else if name.eq_ignore_ascii_case("OutputLayer") && !layer_seen {
            options.input_layer = if value.eq_ignore_ascii_case("Input") {
                true
            } else if value.eq_ignore_ascii_case("Current") {
                false
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            layer_seen = true;
        } else if name.eq_ignore_ascii_case("Corner") && !corner_seen {
            options.corner = if value.eq_ignore_ascii_case("Sharp") {
                CurveOffsetCornerStyle::Sharp
            } else if value.eq_ignore_ascii_case("Chamfer") {
                CurveOffsetCornerStyle::Chamfer
            } else if value.eq_ignore_ascii_case("Round") {
                CurveOffsetCornerStyle::Round
            } else if value.eq_ignore_ascii_case("None") {
                CurveOffsetCornerStyle::None
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            corner_seen = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        index += consumed;
    }
    match options.mode {
        OffsetMode::Distance(_) if options.both_sides == options.side.is_some() => {
            return Err(CommandError::Usage(USAGE));
        }
        OffsetMode::ThroughPoint(_) if options.both_sides || options.side.is_some() => {
            return Err(CommandError::Usage(USAGE));
        }
        _ => {}
    }
    Ok(options)
}
