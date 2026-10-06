//! Circular-profile pipe construction from a selected centerline curve.

use super::*;
use viboceros_geometry::{
    Circle3, CurveSegment3, Frame3, FrameTransportOptions, NurbsSurface, PolyCurve3, SurfaceIso,
    Sweep1, SweepBlend, SweepFrameStyle, SweepSection, WeightedPoint3, join_breps,
};

const USAGE: &str = "Pipe [curve-id] start-radius [end-radius] [Stations=fraction:radius,...] [Cap=None|Flat|Round] [ShapeBlending=Local|Global] [FitRail=Yes|No] [Thick=Yes|No] [WallThickness=signed-distance]";

pub(super) struct PipeCommand;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipeCap {
    None,
    Flat,
    Round,
}

#[derive(Clone, Copy)]
struct PipeOptions {
    blend: SweepBlend,
    fit_rail: bool,
    cap: PipeCap,
    wall_thickness: Option<Real>,
}

impl Command for PipeCommand {
    fn name(&self) -> &'static str {
        "Pipe"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (source_id, arguments) = if let Some(id) = arguments
            .first()
            .and_then(|value| value.parse::<ObjectId>().ok())
        {
            (id, &arguments[1..])
        } else {
            let selected = document.selected_object_ids().collect::<Vec<_>>();
            let [id] = selected.as_slice() else {
                return Err(CommandError::Usage(USAGE));
            };
            (*id, arguments)
        };
        let Some(start_radius) = arguments.first() else {
            return Err(CommandError::Usage(USAGE));
        };
        let start_radius = positive_radius(start_radius)?;
        let mut next = 1;
        let end_radius = if let Some(value) = arguments.get(next)
            && !value.contains('=')
        {
            next += 1;
            positive_radius(value)?
        } else {
            start_radius
        };
        let mut cap = None;
        let mut blend = None;
        let mut fit_rail = None;
        let mut thick = None;
        let mut wall_thickness = None;
        let mut stations = None;
        for argument in &arguments[next..] {
            let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
            if option_name_eq(name, "Cap") && cap.is_none() {
                cap = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "none" => PipeCap::None,
                        "flat" => PipeCap::Flat,
                        "round" => PipeCap::Round,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "ShapeBlending") && blend.is_none() {
                blend = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "local" => SweepBlend::Local,
                        "global" => SweepBlend::Global,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
            } else if option_name_eq(name, "FitRail") && fit_rail.is_none() {
                fit_rail = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "Thick") && thick.is_none() {
                thick = Some(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?);
            } else if option_name_eq(name, "WallThickness") && wall_thickness.is_none() {
                let value = value
                    .parse::<Real>()
                    .map_err(|_| CommandError::InvalidNumber(value.to_owned()))?;
                if !value.is_finite() || value == 0.0 {
                    return Err(CommandError::Usage(USAGE));
                }
                wall_thickness = Some(value);
            } else if option_name_eq(name, "Stations") && stations.is_none() {
                let parsed = value
                    .split(',')
                    .map(|station| {
                        let (fraction, radius) =
                            station.split_once(':').ok_or(CommandError::Usage(USAGE))?;
                        let fraction = fraction
                            .parse::<Real>()
                            .map_err(|_| CommandError::InvalidNumber(fraction.to_owned()))?;
                        if !fraction.is_finite() || !(0.0..1.0).contains(&fraction) {
                            return Err(CommandError::Usage(USAGE));
                        }
                        Ok((fraction, positive_radius(radius)?))
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?;
                if parsed.is_empty()
                    || parsed.len() > 254
                    || parsed.windows(2).any(|pair| pair[0].0 >= pair[1].0)
                {
                    return Err(CommandError::Usage(USAGE));
                }
                stations = Some(parsed);
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        let cap = cap.unwrap_or(PipeCap::Flat);
        let blend = blend.unwrap_or(SweepBlend::Local);
        let fit_rail = fit_rail.unwrap_or(false);
        if thick == Some(false) && wall_thickness.is_some()
            || thick == Some(true) && wall_thickness.is_none()
        {
            return Err(CommandError::Usage(USAGE));
        }
        let second_radii = wall_thickness
            .map(|thickness| {
                let first = start_radius + thickness;
                let second = end_radius + thickness;
                if !first.is_finite() || !second.is_finite() || first <= 0.0 || second <= 0.0 {
                    return Err(CommandError::Usage(USAGE));
                }
                Ok([first, second])
            })
            .transpose()?;
        let stations = stations.unwrap_or_default();
        if let Some(thickness) = wall_thickness
            && stations
                .iter()
                .any(|(_, radius)| radius + thickness <= 0.0 || !(radius + thickness).is_finite())
        {
            return Err(CommandError::Usage(USAGE));
        }
        // Rhino 8's double-wall Pipe uses planar annular caps for Round too.
        let cap = if second_radii.is_some() && cap == PipeCap::Round {
            PipeCap::Flat
        } else {
            cap
        };
        let options = PipeOptions {
            blend,
            fit_rail,
            cap,
            wall_thickness,
        };
        if !document.is_object_selectable(source_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let source = document
            .object(source_id)
            .map(|object| object.geometry())
            .ok_or(CommandError::Usage(USAGE))?;
        let rail = source.curve_ref().ok_or(CommandError::Usage(USAGE))?;
        let tolerance = document.tolerance();
        let fitted_tapered_corner = fit_rail
            && stations.is_empty()
            && start_radius != end_radius
            && rail.to_nurbs()?.has_full_multiplicity_kink(tolerance)?;
        let result = match source {
            Geometry::Polyline(polyline)
                if stations.is_empty()
                    && (start_radius == end_radius || fitted_tapered_corner)
                    && polyline.vertices().len() >= 3 =>
            {
                let lines = polyline
                    .vertices()
                    .windows(2)
                    .map(|pair| {
                        Ok(CurveSegment3::Line(LineSegment::try_new(
                            pair[0], pair[1], tolerance,
                        )?))
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                let polycurve = PolyCurve3::try_new(lines)?;
                Geometry::Brep(segmented_polycurve_pipe(
                    rail,
                    &polycurve,
                    [start_radius, end_radius],
                    &stations,
                    options,
                    tolerance,
                )?)
            }
            Geometry::PolyCurve(_) if fit_rail && !stations.is_empty() => {
                return Err(CommandError::Usage(USAGE));
            }
            Geometry::PolyCurve(polycurve)
                if !fit_rail
                    || (stations.is_empty()
                        && (start_radius == end_radius || fitted_tapered_corner)
                        && polycurve.segments().len() >= 2
                        && polycurve
                            .segments()
                            .iter()
                            .all(|segment| matches!(segment, CurveSegment3::Line(_)))) =>
            {
                Geometry::Brep(segmented_polycurve_pipe(
                    rail,
                    polycurve,
                    [start_radius, end_radius],
                    &stations,
                    options,
                    tolerance,
                )?)
            }
            source if !stations.is_empty() => station_pipe(
                source,
                rail,
                [start_radius, end_radius],
                &stations,
                options,
                tolerance,
            )?,
            Geometry::Line(line) => {
                let frame = Frame3::try_from_normal(
                    line.start(),
                    line.start().vector_to(line.end())?,
                    tolerance,
                )?;
                let height = line.length()?;
                let constant = start_radius == end_radius;
                if cap == PipeCap::Round {
                    let end_frame = Frame3::try_from_x_and_normal(
                        line.end(),
                        frame.x_axis().as_vector(),
                        frame.z_axis().as_vector(),
                        tolerance,
                    )?;
                    Geometry::Brep(round_cap_single_wall(
                        straight_wall(frame, [start_radius, end_radius], height, blend, tolerance)?,
                        [frame, end_frame],
                        [start_radius, end_radius],
                        pipe_round_cap_slopes(rail, [start_radius, end_radius], blend, tolerance)?,
                        tolerance,
                    )?)
                } else if let Some(second) = second_radii {
                    if cap == PipeCap::Flat && constant {
                        Geometry::Brep(Brep::try_tube(
                            frame,
                            [start_radius, second[0]],
                            height,
                            tolerance,
                        )?)
                    } else {
                        let (outer, inner) = wall_radii(
                            [start_radius, end_radius],
                            second,
                            wall_thickness.expect("second wall requires a thickness"),
                        );
                        let outer = straight_wall(frame, outer, height, blend, tolerance)?;
                        let inner = straight_wall(frame, inner, height, blend, tolerance)?;
                        Geometry::Brep(finish_two_walls(outer, inner, cap, tolerance)?)
                    }
                } else {
                    match (cap, constant) {
                        (PipeCap::Flat, true) => Geometry::Brep(Brep::try_cylinder(
                            frame,
                            start_radius,
                            0.0,
                            height,
                            tolerance,
                        )?),
                        (PipeCap::Flat, false) if blend == SweepBlend::Local => {
                            Geometry::Brep(cap_wall(
                                straight_wall(
                                    frame,
                                    [start_radius, end_radius],
                                    height,
                                    blend,
                                    tolerance,
                                )?,
                                PipeCap::Flat,
                                tolerance,
                            )?)
                        }
                        (PipeCap::Flat, false) => Geometry::Brep(Brep::try_truncated_cone(
                            frame,
                            [start_radius, end_radius],
                            height,
                            tolerance,
                        )?),
                        (PipeCap::None, true) => Geometry::NurbsSurface(
                            NurbsSurface::try_cylinder(frame, start_radius, 0.0, height)?,
                        ),
                        (PipeCap::None, false) => Geometry::NurbsSurface(straight_surface(
                            frame,
                            [start_radius, end_radius],
                            height,
                            blend,
                        )?),
                        (PipeCap::Round, _) => unreachable!("round caps handled above"),
                    }
                }
            }
            Geometry::Circle(circle) => {
                if start_radius != end_radius {
                    return Err(CommandError::Usage(USAGE));
                }
                let frame = Frame3::try_from_x_and_normal(
                    circle.center(),
                    circle.x_axis().as_vector(),
                    circle.normal()?.as_vector(),
                    tolerance,
                )?;
                if let Some(second) = second_radii {
                    let (outer, inner) = wall_radii(
                        [start_radius, end_radius],
                        second,
                        wall_thickness.expect("second wall requires a thickness"),
                    );
                    if outer[0] >= circle.radius() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    let outer = torus_wall(frame, circle.radius(), outer[0], tolerance)?;
                    let inner = torus_wall(frame, circle.radius(), inner[0], tolerance)?;
                    Geometry::Brep(finish_two_walls(outer, inner, PipeCap::None, tolerance)?)
                } else {
                    if start_radius >= circle.radius() {
                        return Err(CommandError::Usage(USAGE));
                    }
                    Geometry::NurbsSurface(NurbsSurface::try_torus(
                        frame,
                        circle.radius(),
                        start_radius,
                    )?)
                }
            }
            _ => {
                if rail.is_closed()? {
                    return Err(CommandError::Usage(USAGE));
                }
                let first = [start_radius, end_radius];
                let frames = if cap == PipeCap::Round {
                    Some(pipe_rail_frames(rail, first, tolerance)?)
                } else {
                    None
                };
                let result = if let Some(second) = second_radii {
                    let (outer_radii, inner_radii) = wall_radii(
                        first,
                        second,
                        wall_thickness.expect("second wall requires a thickness"),
                    );
                    let outer_frames = match frames {
                        Some(frames) => frames,
                        None => pipe_rail_frames(rail, outer_radii, tolerance)?,
                    };
                    let inner_frames = match frames {
                        Some(frames) => frames,
                        None => pipe_rail_frames(rail, inner_radii, tolerance)?,
                    };
                    let outer =
                        swept_wall_with_frames(rail, outer_radii, blend, tolerance, outer_frames)?;
                    let inner =
                        swept_wall_with_frames(rail, inner_radii, blend, tolerance, inner_frames)?;
                    finish_two_walls(outer, inner, cap, tolerance)?
                } else {
                    let wall_frames = match frames {
                        Some(frames) => frames,
                        None => pipe_rail_frames(rail, first, tolerance)?,
                    };
                    let wall = swept_wall_with_frames(rail, first, blend, tolerance, wall_frames)?;
                    if let Some(frames) = frames {
                        round_cap_single_wall(
                            wall,
                            frames,
                            first,
                            pipe_round_cap_slopes(rail, first, blend, tolerance)?,
                            tolerance,
                        )?
                    } else {
                        cap_wall(wall, cap, tolerance)?
                    }
                };
                Geometry::Brep(result)
            }
        };
        let closed = matches!(&result, Geometry::Brep(brep) if brep.is_closed())
            || matches!(&result, Geometry::NurbsSurface(surface) if surface.is_closed_u()? && surface.is_closed_v()?);
        let id = document.add_geometry(result)?;
        Ok(format!(
            "Added {} pipe {id} (start radius {start_radius:.6}, end radius {end_radius:.6})",
            if closed { "closed" } else { "open" }
        ))
    }
}

fn positive_radius(value: &str) -> Result<Real, CommandError> {
    let radius = value
        .parse::<Real>()
        .map_err(|_| CommandError::InvalidNumber(value.to_owned()))?;
    if !radius.is_finite() || radius <= 0.0 {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(radius)
}

fn segmented_polycurve_pipe(
    rail: CurveRef<'_>,
    polycurve: &PolyCurve3,
    endpoint_radii: [Real; 2],
    stations: &[(Real, Real)],
    options: PipeOptions,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let PipeOptions {
        blend,
        fit_rail,
        cap,
        wall_thickness,
    } = options;
    if rail.is_closed()? {
        if fit_rail
            && stations.is_empty()
            && endpoint_radii[0] == endpoint_radii[1]
            && polycurve.segments().len() >= 3
            && polycurve
                .segments()
                .iter()
                .all(|segment| matches!(segment, CurveSegment3::Line(_)))
        {
            let lines = polycurve
                .segments()
                .iter()
                .filter_map(|segment| match segment {
                    CurveSegment3::Line(line) => Some(*line),
                    _ => None,
                })
                .collect::<Vec<_>>();
            // Rhino's fitted closed Pipe places its periodic seam halfway
            // along the first line, leaving no rail corner at that seam.
            let first = lines[0];
            let midpoint = first.point_at(0.5)?;
            let mut rotated = Vec::with_capacity(lines.len() + 1);
            rotated.push(LineSegment::try_new(midpoint, first.end(), tolerance)?);
            rotated.extend(lines.iter().skip(1).copied());
            rotated.push(LineSegment::try_new(first.start(), midpoint, tolerance)?);
            if let Some(pipe) = mitered_line_pipe_profile(
                &rotated,
                endpoint_radii,
                blend,
                cap,
                wall_thickness,
                true,
                tolerance,
            )? {
                return Ok(pipe);
            }
        }
        return Err(CommandError::Usage(USAGE));
    }
    if stations.is_empty()
        && (endpoint_radii[0] == endpoint_radii[1] || fit_rail)
        && polycurve.segments().len() >= 2
        && polycurve
            .segments()
            .iter()
            .all(|segment| matches!(segment, CurveSegment3::Line(_)))
    {
        let lines = polycurve
            .segments()
            .iter()
            .filter_map(|segment| match segment {
                CurveSegment3::Line(line) => Some(*line),
                _ => None,
            })
            .collect::<Vec<_>>();
        let pipe = if endpoint_radii[0] == endpoint_radii[1] {
            mitered_line_pipe(&lines, endpoint_radii[0], cap, wall_thickness, tolerance)?
        } else {
            // Rhino's thin Global round caps on sharp rails depend on the
            // rail's orientation relative to world Z. The spherical cap
            // construction agrees on horizontal rails; thick round pipes
            // use annular planar ends and do not need that fitted cap.
            if cap == PipeCap::Round
                && blend == SweepBlend::Global
                && wall_thickness.is_none()
                && lines
                    .iter()
                    .any(|line| (line.end().z() - line.start().z()).abs() > tolerance.absolute())
            {
                return Err(CommandError::Usage(USAGE));
            }
            mitered_line_pipe_profile(
                &lines,
                endpoint_radii,
                blend,
                cap,
                wall_thickness,
                false,
                tolerance,
            )?
        };
        if let Some(pipe) = pipe {
            return Ok(pipe);
        }
    }
    if !stations.is_empty() && blend == SweepBlend::Global {
        return Err(CommandError::Usage(USAGE));
    }
    let lengths = polycurve
        .segments()
        .iter()
        .map(|segment| segment.as_ref().length(tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let total = lengths.iter().sum::<Real>();
    if !total.is_finite() || total <= 0.0 {
        return Err(CommandError::Usage(USAGE));
    }
    let mut profile = Vec::with_capacity(stations.len() + 2);
    profile.push((0.0, endpoint_radii[0]));
    profile.extend_from_slice(stations);
    profile.push((1.0, endpoint_radii[1]));
    let radius_at = |fraction: Real| -> Real {
        let pair = profile
            .windows(2)
            .find(|pair| fraction <= pair[1].0)
            .expect("last profile station is the end");
        let f = (fraction - pair[0].0) / (pair[1].0 - pair[0].0);
        let f = f * f * (3.0 - 2.0 * f);
        pair[0].1.mul_add(1.0 - f, pair[1].1 * f)
    };
    let mut segment_profiles = Vec::with_capacity(lengths.len());
    let mut distance = 0.0;
    for (index, &length) in lengths.iter().enumerate() {
        let start = distance / total;
        distance += length;
        let end = distance / total;
        let segment = polycurve.segments()[index].as_ref();
        let mut segment_profile = vec![(*segment.domain().start(), radius_at(start))];
        for &(fraction, radius) in stations {
            if start < fraction && fraction < end {
                if !matches!(polycurve.segments()[index], CurveSegment3::Line(_)) {
                    return Err(CommandError::Usage(USAGE));
                }
                let parameter = segment.parameter_at((fraction - start) / (end - start))?;
                segment_profile.push((parameter, radius));
            }
        }
        segment_profile.push((*segment.domain().end(), radius_at(end)));
        segment_profiles.push(segment_profile);
    }
    let make_wall = |offset: Real| -> Result<Brep, CommandError> {
        let mut walls = Vec::with_capacity(polycurve.segments().len());
        for (segment, original_profile) in polycurve.segments().iter().zip(&segment_profiles) {
            let profile = original_profile
                .iter()
                .map(|&(parameter, radius)| (parameter, radius + offset))
                .collect::<Vec<_>>();
            let pair = [profile[0].1, profile.last().expect("end radius").1];
            let wall = match segment {
                _ if profile.len() > 2 => {
                    segmented_station_wall(segment.as_ref(), &profile, tolerance)?
                }
                CurveSegment3::Line(line) => {
                    let frame = Frame3::try_from_normal(
                        line.start(),
                        line.start().vector_to(line.end())?,
                        tolerance,
                    )?;
                    straight_wall(frame, pair, line.length()?, blend, tolerance)?
                }
                _ => {
                    let segment = segment.as_ref();
                    let frames = pipe_rail_frames(segment, pair, tolerance)?;
                    swept_wall_with_frames(segment, pair, blend, tolerance, frames)?
                }
            };
            walls.push(wall);
        }
        if walls.len() == 1 {
            return Ok(walls.pop().expect("one segment wall"));
        }
        let references = walls.iter().collect::<Vec<_>>();
        let mut joined = join_breps(&references, tolerance.absolute(), tolerance)?;
        if joined.len() != 1 {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(joined.pop().expect("one wall component").brep)
    };
    if let Some(thickness) = wall_thickness {
        let (outer, inner) = if thickness > 0.0 {
            (make_wall(thickness)?, make_wall(0.0)?)
        } else {
            (make_wall(0.0)?, make_wall(thickness)?)
        };
        return finish_two_walls(outer, inner, cap, tolerance);
    }
    let wall = make_wall(0.0)?;
    if cap == PipeCap::Round {
        return round_cap_single_wall(
            wall,
            pipe_rail_frames(rail, endpoint_radii, tolerance)?,
            endpoint_radii,
            pipe_round_cap_slopes(rail, endpoint_radii, blend, tolerance)?,
            tolerance,
        );
    }
    cap_wall(wall, cap, tolerance)
}

fn mitered_line_pipe(
    lines: &[LineSegment],
    radius: Real,
    cap: PipeCap,
    wall_thickness: Option<Real>,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    mitered_line_pipe_profile(
        lines,
        [radius; 2],
        SweepBlend::Local,
        cap,
        wall_thickness,
        false,
        tolerance,
    )
}

fn mitered_line_pipe_profile(
    lines: &[LineSegment],
    endpoint_radii: [Real; 2],
    blend: SweepBlend,
    cap: PipeCap,
    wall_thickness: Option<Real>,
    closed: bool,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    let directions = lines
        .iter()
        .map(|line| Ok(line.direction(tolerance)?.as_vector()))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut binormal = None;
    for pair in directions.windows(2) {
        let turn = pair[0].cross(pair[1])?;
        if turn.length()? > tolerance.angular() {
            binormal = Some(turn.normalized(tolerance)?.as_vector());
            break;
        }
    }
    let Some(binormal) = binormal else {
        return Ok(None);
    };
    let largest_radius =
        endpoint_radii[0].max(endpoint_radii[1]) + wall_thickness.unwrap_or(0.0).max(0.0);
    let mut reaches = directions
        .windows(2)
        .map(|pair| {
            let divisor = 1.0 + pair[0].dot(pair[1])?;
            if divisor <= tolerance.angular() {
                return Err(CommandError::Usage(USAGE));
            }
            Ok(largest_radius * pair[0].cross(pair[1])?.length()? / divisor)
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    if closed {
        let pair = [*directions.last().unwrap(), directions[0]];
        let divisor = 1.0 + pair[0].dot(pair[1])?;
        if divisor <= tolerance.angular() {
            return Err(CommandError::Usage(USAGE));
        }
        reaches.push(largest_radius * pair[0].cross(pair[1])?.length()? / divisor);
    }
    let lengths = lines
        .iter()
        .map(|line| line.length())
        .collect::<Result<Vec<_>, _>>()?;
    if lengths.iter().enumerate().any(|(index, &length)| {
        let start = if index == 0 {
            if closed {
                reaches[reaches.len() - 1]
            } else {
                0.0
            }
        } else {
            reaches[index - 1]
        };
        let end = reaches.get(index).copied().unwrap_or(0.0);
        start + end >= length
    }) {
        return Err(CommandError::Usage(USAGE));
    }
    let mut frame_x = binormal;
    let mut frames = Vec::with_capacity(lines.len());
    for (index, (line, direction)) in lines.iter().zip(&directions).enumerate() {
        if index > 0 {
            // Carry one circular seam through spatial bends so the two
            // parameterizations of each miter ellipse agree at the join.
            let rotation = AffineTransform3::try_rotation_between(
                directions[index - 1].normalized_nonzero()?,
                direction.normalized_nonzero()?,
                tolerance,
            )?;
            frame_x = rotation.transform_vector(frame_x)?;
        }
        frames.push(Frame3::try_from_x_and_normal(
            line.start(),
            frame_x,
            *direction,
            tolerance,
        )?);
    }
    let seam_rotation = if closed {
        let last_direction = *directions.last().expect("closed line loop has segments");
        let closing = AffineTransform3::try_rotation_between(
            last_direction.normalized_nonzero()?,
            directions[0].normalized_nonzero()?,
            tolerance,
        )?;
        let initial = frames[0].x_axis().as_vector();
        let returned = closing.transform_vector(
            frames
                .last()
                .expect("closed line loop has frames")
                .x_axis()
                .as_vector(),
        )?;
        let sine = directions[0].dot(initial.cross(returned)?)?;
        let cosine = initial.dot(returned)?;
        -sine.atan2(cosine)
    } else {
        0.0
    };
    let total = lengths.iter().sum::<Real>();
    let make_wall = |offset: Real| -> Result<Brep, CommandError> {
        let mut distance = 0.0;
        let surfaces = lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let start = distance / total;
                distance += lengths[index];
                let end = distance / total;
                let start_joint = index
                    .checked_sub(1)
                    .map(|i| [directions[i], directions[index]])
                    .or_else(|| closed.then(|| [*directions.last().unwrap(), directions[0]]));
                let end_joint = directions
                    .get(index + 1)
                    .map(|&next| [directions[index], next])
                    .or_else(|| closed.then(|| [*directions.last().unwrap(), directions[0]]));
                let surface = if closed {
                    mitered_line_closed_surface(
                        *line,
                        frames[index],
                        endpoint_radii[0] + offset,
                        [start, end],
                        seam_rotation,
                        start_joint,
                        end_joint,
                        tolerance,
                    )?
                } else if endpoint_radii[0] == endpoint_radii[1] {
                    mitered_line_surface(
                        *line,
                        frames[index],
                        endpoint_radii[0] + offset,
                        start_joint,
                        end_joint,
                        tolerance,
                    )?
                } else {
                    mitered_line_profile_surface(
                        *line,
                        frames[index],
                        endpoint_radii,
                        offset,
                        blend,
                        [start, end],
                        start_joint,
                        end_joint,
                        tolerance,
                    )?
                };
                Ok(surface)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if closed {
            // Rhino uses rail distance as U and the circular section as V.
            // Swapping axes reverses the natural normal, so reverse the B-rep.
            let surface =
                combine_closed_line_wall_surfaces(&surfaces, tolerance)?.try_swapped_uv()?;
            return Ok(Brep::try_surface_grid(&surface, &[], &[], tolerance)?.reversed());
        }
        let walls = surfaces
            .iter()
            .map(|surface| Ok(Brep::try_surface_grid(surface, &[], &[], tolerance)?))
            .collect::<Result<Vec<_>, CommandError>>()?;
        let references = walls.iter().collect::<Vec<_>>();
        let mut joined = join_breps(&references, tolerance.absolute(), tolerance)?;
        if joined.len() != 1 {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(joined.pop().expect("one mitered wall component").brep)
    };
    if let Some(thickness) = wall_thickness {
        let (outer, inner) = if thickness > 0.0 {
            (make_wall(thickness)?, make_wall(0.0)?)
        } else {
            (make_wall(0.0)?, make_wall(thickness)?)
        };
        return Ok(Some(finish_two_walls(
            outer,
            inner,
            if closed { PipeCap::None } else { cap },
            tolerance,
        )?));
    }
    let wall = make_wall(0.0)?;
    if closed {
        return Ok(Some(wall));
    }
    if cap == PipeCap::Round {
        let last = *lines.last().expect("at least two mitered lines");
        let end_frame = Frame3::try_from_x_and_normal(
            last.end(),
            frames
                .last()
                .expect("last mitered frame")
                .x_axis()
                .as_vector(),
            *directions.last().expect("last mitered direction"),
            tolerance,
        )?;
        return Ok(Some(round_cap_single_wall(
            wall,
            [frames[0], end_frame],
            endpoint_radii,
            if blend == SweepBlend::Global {
                [(endpoint_radii[1] - endpoint_radii[0]) / total; 2]
            } else {
                [0.0; 2]
            },
            tolerance,
        )?));
    }
    Ok(Some(cap_wall(wall, cap, tolerance)?))
}

fn combine_closed_line_wall_surfaces(
    surfaces: &[NurbsSurface],
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    let first = &surfaces[0];
    let width = first.control_point_count_u();
    let mut controls = first.control_points().to_vec();
    let mut knots_v = vec![0.0; 4];
    let mut distance = 0.0;
    for (index, surface) in surfaces.iter().enumerate() {
        if index > 0 {
            let previous = &controls[controls.len() - width..];
            let next = &surface.control_points()[..width];
            if previous.iter().zip(next).any(|(a, b)| {
                !a.point()
                    .distance_to(b.point())
                    .is_ok_and(|gap| gap <= tolerance.absolute())
                    || (a.weight() - b.weight()).abs() > 64.0 * Real::EPSILON
            }) {
                return Err(CommandError::Usage(USAGE));
            }
            controls.extend_from_slice(&surface.control_points()[width..]);
        }
        distance += *surface.domain_v().end() - *surface.domain_v().start();
        knots_v.extend(std::iter::repeat_n(
            distance,
            if index + 1 == surfaces.len() { 4 } else { 3 },
        ));
    }
    let surface = NurbsSurface::try_new_rational(
        first.degree_u(),
        3,
        width,
        controls.len() / width,
        controls,
        first.knots_u().to_vec(),
        knots_v,
    )?;
    if !surface.is_closed_u()? || !surface.is_closed_v()? {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(surface)
}

fn mitered_line_surface(
    line: LineSegment,
    frame: Frame3,
    radius: Real,
    start_joint: Option<[Vector3; 2]>,
    end_joint: Option<[Vector3; 2]>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    let length = line.length()?;
    let cylinder = NurbsSurface::try_cylinder(frame, radius, 0.0, length)?;
    let mut controls = cylinder.control_points().to_vec();
    let count = cylinder.control_point_count_u();
    let direction = line.direction(tolerance)?.as_vector();
    for (row, joint, pair) in [
        (0, line.start(), start_joint),
        (count, line.end(), end_joint),
    ] {
        let Some([first_direction, second_direction]) = pair else {
            continue;
        };
        let divisor = 1.0 + first_direction.dot(second_direction)?;
        for control in &mut controls[row..row + count] {
            let radial = joint.vector_to(control.point())?;
            // The joint plane has normal t_before + t_after. Move each
            // circular control along this segment's axis onto that plane.
            let shift = -(first_direction.dot(radial)? + second_direction.dot(radial)?) / divisor;
            let point = control.point().translated(direction.scaled(shift)?)?;
            *control = WeightedPoint3::try_new(point, control.weight())?;
        }
    }
    Ok(NurbsSurface::try_new_rational(
        cylinder.degree_u(),
        cylinder.degree_v(),
        cylinder.control_point_count_u(),
        cylinder.control_point_count_v(),
        controls,
        cylinder.knots_u().to_vec(),
        cylinder.knots_v().to_vec(),
    )?)
}

#[allow(clippy::too_many_arguments)]
fn mitered_line_closed_surface(
    line: LineSegment,
    frame: Frame3,
    radius: Real,
    interval: [Real; 2],
    seam_rotation: Real,
    start_joint: Option<[Vector3; 2]>,
    end_joint: Option<[Vector3; 2]>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    let length = line.length()?;
    let unit = NurbsSurface::try_cylinder(frame, 1.0, 0.0, length)?;
    let circle = &unit.control_points()[..unit.control_point_count_u()];
    let blend = |fraction: Real| fraction * fraction * (3.0 - 2.0 * fraction);
    let derivative = |fraction: Real| 6.0 * fraction * (1.0 - fraction);
    let [start, end] = interval;
    let span = end - start;
    let blend_controls = [
        blend(start),
        blend(start) + span * derivative(start) / 3.0,
        blend(end) - span * derivative(end) / 3.0,
        blend(end),
    ];
    let (sine, cosine) = seam_rotation.sin_cos();
    let axial = frame.z_axis().as_vector();
    let mut controls = Vec::with_capacity(4 * circle.len());
    for (row, blend) in blend_controls.into_iter().enumerate() {
        let real = 1.0 - blend + blend * cosine;
        let imaginary = blend * sine;
        let center = frame
            .origin()
            .translated(axial.scaled(row as Real * length / 3.0)?)?;
        for control in circle {
            let radial = frame.origin().vector_to(control.point())?;
            let tangent = axial.cross(radial)?;
            let point = center
                .translated(radial.scaled(radius * real)?)?
                .translated(tangent.scaled(radius * imaginary)?)?;
            controls.push(WeightedPoint3::try_new(point, control.weight())?);
        }
    }
    let count = circle.len();
    let direction = line.direction(tolerance)?.as_vector();
    for (row, joint, pair) in [
        (0, line.start(), start_joint),
        (3 * count, line.end(), end_joint),
    ] {
        let Some([first_direction, second_direction]) = pair else {
            continue;
        };
        let divisor = 1.0 + first_direction.dot(second_direction)?;
        for control in &mut controls[row..row + count] {
            let radial = joint.vector_to(control.point())?;
            let shift = -(first_direction.dot(radial)? + second_direction.dot(radial)?) / divisor;
            let point = control.point().translated(direction.scaled(shift)?)?;
            *control = WeightedPoint3::try_new(point, control.weight())?;
        }
    }
    Ok(NurbsSurface::try_new_rational(
        unit.degree_u(),
        3,
        count,
        4,
        controls,
        unit.knots_u().to_vec(),
        vec![0.0, 0.0, 0.0, 0.0, length, length, length, length],
    )?)
}

#[allow(clippy::too_many_arguments)]
fn mitered_line_profile_surface(
    line: LineSegment,
    frame: Frame3,
    endpoint_radii: [Real; 2],
    offset: Real,
    blend: SweepBlend,
    interval: [Real; 2],
    start_joint: Option<[Vector3; 2]>,
    end_joint: Option<[Vector3; 2]>,
    tolerance: Tolerance,
) -> Result<NurbsSurface, CommandError> {
    let length = line.length()?;
    let unit = NurbsSurface::try_cylinder(frame, 1.0, 0.0, length)?;
    let circle = &unit.control_points()[..unit.control_point_count_u()];
    let delta = endpoint_radii[1] - endpoint_radii[0];
    let radius = |fraction: Real| {
        endpoint_radii[0]
            + delta
                * if blend == SweepBlend::Local {
                    fraction * fraction * (3.0 - 2.0 * fraction)
                } else {
                    fraction
                }
    };
    let derivative = |fraction: Real| {
        delta
            * if blend == SweepBlend::Local {
                6.0 * fraction * (1.0 - fraction)
            } else {
                1.0
            }
    };
    let [start, end] = interval;
    let span = end - start;
    let first = radius(start);
    let last = radius(end);
    let profile = [
        first,
        first + span * derivative(start) / 3.0,
        last - span * derivative(end) / 3.0,
        last,
    ];
    let axial = frame.z_axis().as_vector();
    let mut controls = Vec::with_capacity(4 * circle.len());
    for (index, profile_radius) in profile.into_iter().enumerate() {
        let offset_vector = axial.scaled(index as Real * length / 3.0)?;
        for control in circle {
            let radial = frame.origin().vector_to(control.point())?;
            let point = frame
                .origin()
                .translated(radial.scaled(profile_radius + offset)?)?
                .translated(offset_vector)?;
            controls.push(WeightedPoint3::try_new(point, control.weight())?);
        }
    }
    let count = circle.len();
    let direction = line.direction(tolerance)?.as_vector();
    // Ease joint-plane shear through the fitted span. The interior control
    // fractions approximate Rhino's fitted wall; scaling from the opposite
    // endpoint radius prevents a taper from amplifying the shear early.
    for (joint, free_radius, rows) in [
        (start_joint, profile[3] + offset, [(1, 0.5), (2, 0.27)]),
        (end_joint, profile[0] + offset, [(1, 0.27), (2, 0.5)]),
    ] {
        let Some([first_direction, second_direction]) = joint else {
            continue;
        };
        let divisor = 1.0 + first_direction.dot(second_direction)?;
        for (row, factor) in rows {
            let center = frame
                .origin()
                .translated(axial.scaled(row as Real * length / 3.0)?)?;
            for control in &mut controls[row * count..(row + 1) * count] {
                let radial = center.vector_to(control.point())?;
                let shift = -(first_direction.dot(radial)? + second_direction.dot(radial)?)
                    / divisor
                    * free_radius
                    / (profile[row] + offset)
                    * factor;
                let point = control.point().translated(direction.scaled(shift)?)?;
                *control = WeightedPoint3::try_new(point, control.weight())?;
            }
        }
    }
    for (row, joint, pair) in [
        (0, line.start(), start_joint),
        (3 * count, line.end(), end_joint),
    ] {
        let Some([first_direction, second_direction]) = pair else {
            continue;
        };
        let divisor = 1.0 + first_direction.dot(second_direction)?;
        for control in &mut controls[row..row + count] {
            let radial = joint.vector_to(control.point())?;
            let shift = -(first_direction.dot(radial)? + second_direction.dot(radial)?) / divisor;
            let point = control.point().translated(direction.scaled(shift)?)?;
            *control = WeightedPoint3::try_new(point, control.weight())?;
        }
    }
    Ok(NurbsSurface::try_new_rational(
        unit.degree_u(),
        3,
        count,
        4,
        controls,
        unit.knots_u().to_vec(),
        vec![0.0, 0.0, 0.0, 0.0, length, length, length, length],
    )?)
}

fn segmented_station_wall(
    rail: CurveRef<'_>,
    profile: &[(Real, Real)],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let parameters = profile
        .iter()
        .map(|&(parameter, _)| parameter)
        .collect::<Vec<_>>();
    let max_radius = profile
        .iter()
        .map(|&(_, radius)| radius)
        .fold(0.0, Real::max);
    let angular_tolerance = (0.05 * tolerance.absolute() / max_radius).min(1e-10);
    let frames = rail.rotation_minimizing_frames(
        &parameters,
        None,
        FrameTransportOptions {
            angular_tolerance,
            ..Default::default()
        },
    )?;
    let sections = profile
        .iter()
        .zip(&frames)
        .map(|(&(parameter, radius), &frame)| circular_section(parameter, frame, radius, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let sweep = Sweep1::try_new(
        rail,
        &sections,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        tolerance,
    )?;
    let surface = sweep.to_surface()?;
    let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
    Ok(Brep::try_surface_grid(&surface, &u, &v, tolerance)?.reversed())
}

fn station_pipe(
    source: &Geometry,
    rail: CurveRef<'_>,
    endpoint_radii: [Real; 2],
    stations: &[(Real, Real)],
    options: PipeOptions,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
    let PipeOptions {
        blend,
        cap,
        wall_thickness,
        ..
    } = options;
    if rail.is_closed()? {
        return Err(CommandError::Usage(USAGE));
    }
    let mut profile = Vec::with_capacity(stations.len() + 2);
    if blend == SweepBlend::Global {
        return Err(CommandError::Usage(USAGE));
    }
    profile.push((*rail.domain().start(), endpoint_radii[0]));
    for &(fraction, radius) in stations {
        profile.push((rail.parameter_at(fraction)?, radius));
    }
    profile.push((*rail.domain().end(), endpoint_radii[1]));
    if profile.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(CommandError::Usage(USAGE));
    }
    let parameters = profile.iter().map(|&(t, _)| t).collect::<Vec<_>>();
    let max_radius = profile.iter().map(|&(_, r)| r).fold(0.0, Real::max);
    let angular_tolerance = (0.05 * (tolerance.absolute() / max_radius)).min(1e-10);
    let frames = rail.rotation_minimizing_frames(
        &parameters,
        None,
        FrameTransportOptions {
            angular_tolerance,
            ..Default::default()
        },
    )?;
    let make_surface = |profile: &[(Real, Real)]| -> Result<NurbsSurface, CommandError> {
        let sections = profile
            .iter()
            .zip(&frames)
            .map(|(&(parameter, radius), &frame)| {
                circular_section(parameter, frame, radius, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sweep = Sweep1::try_new(rail, &sections, SweepFrameStyle::Freeform, blend, tolerance)?;
        if matches!(source, Geometry::Arc(_)) {
            Ok(sweep.fit_model_surface()?)
        } else {
            Ok(sweep.to_surface()?)
        }
    };
    let make_wall = |profile: &[(Real, Real)]| -> Result<Brep, CommandError> {
        let surface = make_surface(profile)?;
        let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        let wall = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
        Ok(wall.reversed())
    };
    if let Some(thickness) = wall_thickness {
        let second = profile
            .iter()
            .map(|&(t, r)| (t, r + thickness))
            .collect::<Vec<_>>();
        let (outer, inner) = if thickness > 0.0 {
            (make_wall(&second)?, make_wall(&profile)?)
        } else {
            (make_wall(&profile)?, make_wall(&second)?)
        };
        return Ok(Geometry::Brep(finish_two_walls(
            outer, inner, cap, tolerance,
        )?));
    }
    if cap == PipeCap::None && matches!(source, Geometry::Line(_)) {
        return Ok(Geometry::NurbsSurface(make_surface(&profile)?));
    }
    let wall = make_wall(&profile)?;
    if cap == PipeCap::Round {
        return Ok(Geometry::Brep(round_cap_single_wall(
            wall,
            [frames[0], *frames.last().expect("endpoint frame")],
            endpoint_radii,
            [0.0; 2],
            tolerance,
        )?));
    }
    Ok(Geometry::Brep(cap_wall(wall, cap, tolerance)?))
}

fn circular_section(
    parameter: Real,
    frame: Frame3,
    radius: Real,
    tolerance: Tolerance,
) -> Result<SweepSection, CommandError> {
    let circle = Circle3::try_from_frame(
        frame.origin(),
        radius,
        frame.x_axis(),
        frame.z_axis(),
        tolerance,
    )?;
    Ok(SweepSection {
        parameter,
        curve: circle.to_nurbs()?,
    })
}

fn wall_radii(first: [Real; 2], second: [Real; 2], thickness: Real) -> ([Real; 2], [Real; 2]) {
    if thickness > 0.0 {
        (second, first)
    } else {
        (first, second)
    }
}

fn straight_wall(
    frame: Frame3,
    radii: [Real; 2],
    height: Real,
    blend: SweepBlend,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let surface = straight_surface(frame, radii, height, blend)?;
    Ok(Brep::try_surface_grid(&surface, &[], &[], tolerance)?)
}

fn straight_surface(
    frame: Frame3,
    radii: [Real; 2],
    height: Real,
    blend: SweepBlend,
) -> Result<NurbsSurface, CommandError> {
    if radii[0] == radii[1] {
        return Ok(NurbsSurface::try_cylinder(frame, radii[0], 0.0, height)?);
    }
    if blend == SweepBlend::Global {
        return Ok(NurbsSurface::try_truncated_cone(frame, radii, height)?);
    }
    // The cubic Bezier radii [r0, r0, r1, r1] give Rhino's Local
    // smoothstep profile while the axial controls keep z linear.
    let unit_cylinder = NurbsSurface::try_cylinder(frame, 1.0, 0.0, height)?;
    let circle_controls = &unit_cylinder.control_points()[..unit_cylinder.control_point_count_u()];
    let axial = frame.z_axis().as_vector();
    let mut controls = Vec::with_capacity(4 * circle_controls.len());
    for (radius, axial_distance) in [
        (radii[0], 0.0),
        (radii[0], height / 3.0),
        (radii[1], 2.0 * height / 3.0),
        (radii[1], height),
    ] {
        let offset = axial.scaled(axial_distance)?;
        for control in circle_controls {
            let radial = frame.origin().vector_to(control.point())?;
            let point = frame
                .origin()
                .translated(radial.scaled(radius)?)?
                .translated(offset)?;
            controls.push(WeightedPoint3::try_new(point, control.weight())?);
        }
    }
    Ok(NurbsSurface::try_new_rational(
        unit_cylinder.degree_u(),
        3,
        circle_controls.len(),
        4,
        controls,
        unit_cylinder.knots_u().to_vec(),
        vec![0.0, 0.0, 0.0, 0.0, height, height, height, height],
    )?)
}

fn torus_wall(
    frame: Frame3,
    major_radius: Real,
    minor_radius: Real,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let surface = NurbsSurface::try_torus(frame, major_radius, minor_radius)?;
    Ok(Brep::try_surface_grid(&surface, &[], &[], tolerance)?)
}

fn swept_wall_with_frames(
    rail: CurveRef<'_>,
    radii: [Real; 2],
    blend: SweepBlend,
    tolerance: Tolerance,
    frames: [Frame3; 2],
) -> Result<Brep, CommandError> {
    let domain = rail.domain();
    let parameters = [*domain.start(), *domain.end()];
    let mut sections = vec![circular_section(
        parameters[0],
        frames[0],
        radii[0],
        tolerance,
    )?];
    if radii[0] != radii[1] {
        sections.push(circular_section(
            parameters[1],
            frames[1],
            radii[1],
            tolerance,
        )?);
    }
    let sweep = Sweep1::try_new(rail, &sections, SweepFrameStyle::Freeform, blend, tolerance)?;
    // Rhino refits even a constant-radius cubic NURBS rail. Retaining its
    // four-control Bézier basis changes the measured flat Pipe volume.
    let surface = if radii[0] == radii[1] && !matches!(rail, CurveRef::NurbsCurve(_)) {
        sweep.to_rail_basis_surface()?
    } else if matches!(rail, CurveRef::Arc(_)) {
        // A direct continuous fit matches the measured Rhino arc Pipe volume
        // while avoiding a dense cubic rail refit.
        sweep.fit_model_surface()?
    } else {
        sweep.to_surface()?
    };
    let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
    Ok(Brep::try_surface_grid(&surface, &u, &v, tolerance)?.reversed())
}

fn pipe_rail_frames(
    rail: CurveRef<'_>,
    radii: [Real; 2],
    tolerance: Tolerance,
) -> Result<[Frame3; 2], CommandError> {
    let domain = rail.domain();
    let parameters = [*domain.start(), *domain.end()];
    let angular_tolerance = (0.05 * (tolerance.absolute() / radii[0].max(radii[1]))).min(1e-10);
    let frames = rail.rotation_minimizing_frames(
        &parameters,
        None,
        FrameTransportOptions {
            angular_tolerance,
            ..Default::default()
        },
    )?;
    frames.try_into().map_err(|_| CommandError::Usage(USAGE))
}

fn round_cap_single_wall(
    wall: Brep,
    frames: [Frame3; 2],
    radii: [Real; 2],
    cap_slopes: [Real; 2],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let start = round_cap_surface(frames[0], radii[0], cap_slopes[0], true, tolerance)?;
    let end = round_cap_surface(frames[1], radii[1], cap_slopes[1], false, tolerance)?;
    join_round_cap_parts(&wall, &start, &end, tolerance)
}

fn pipe_round_cap_slopes(
    rail: CurveRef<'_>,
    radii: [Real; 2],
    blend: SweepBlend,
    tolerance: Tolerance,
) -> Result<[Real; 2], CommandError> {
    if blend == SweepBlend::Local || radii[0] == radii[1] {
        return Ok([0.0; 2]);
    }
    let radial_rate = (radii[1] - radii[0]) / rail.length(tolerance)?;
    let domain = rail.domain();
    let mut slopes = [0.0; 2];
    for (index, parameter) in [*domain.start(), *domain.end()].into_iter().enumerate() {
        let curvature = rail.curvature_vector(parameter)?.length()?;
        // A round cap is tangent to the inside of the swept wall, whose
        // endpoint travel speed is (1 - curvature * radius) times rail speed.
        let inner_speed = 1.0 - curvature * radii[index];
        if inner_speed <= 0.0 {
            return Err(CommandError::Usage(USAGE));
        }
        slopes[index] = radial_rate / inner_speed;
    }
    Ok(slopes)
}

fn round_cap_surface(
    frame: Frame3,
    radius: Real,
    slope: Real,
    start: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let axis = frame.z_axis().as_vector();
    let center = frame.origin().translated(axis.scaled(-radius * slope)?)?;
    let shifted_frame =
        Frame3::try_from_x_and_normal(center, frame.x_axis().as_vector(), axis, tolerance)?;
    let sphere = NurbsSurface::try_sphere(shifted_frame, radius * slope.hypot(1.0))?;
    let half_pi = std::f64::consts::FRAC_PI_2;
    let cut = if slope == 0.0 {
        0.0
    } else {
        let target = radius * slope;
        let (mut low, mut high) = (-half_pi, half_pi);
        for _ in 0..64 {
            let middle = 0.5 * (low + high);
            let height = center.vector_to(sphere.evaluate(0.0, middle)?)?.dot(axis)?;
            if height < target {
                low = middle;
            } else {
                high = middle;
            }
        }
        0.5 * (low + high)
    };
    let trimmed = if start {
        sphere.try_trimmed_v(-half_pi..=cut)?
    } else {
        sphere.try_trimmed_v(cut..=half_pi)?
    };
    Ok(Brep::try_surface_grid(&trimmed, &[], &[], tolerance)?)
}

fn join_round_cap_parts(
    wall: &Brep,
    start: &Brep,
    end: &Brep,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    // These surfaces have known complete circular boundary pairs. Try exact
    // pair assembly before the general edge-discovery and cleanup algorithm.
    let wall_counts = wall.edge_use_counts();
    let boundary = |iso| {
        let mut edges = wall
            .faces()
            .iter()
            .flat_map(|face| face.loops())
            .flat_map(|loop_| loop_.trims())
            .filter(|trim| trim.iso() == iso)
            .filter_map(|trim| trim.edge())
            .filter(|&edge| wall_counts[edge] == 1);
        let edge = edges.next()?;
        edges.next().is_none().then_some(edge)
    };
    let lone_naked = |brep: &Brep| {
        let mut edges = brep
            .edge_use_counts()
            .into_iter()
            .enumerate()
            .filter_map(|(edge, uses)| (uses == 1).then_some(edge));
        let edge = edges.next()?;
        edges.next().is_none().then_some(edge)
    };
    if let (Some(west), Some(east), Some(start_edge), Some(end_edge)) = (
        boundary(SurfaceIso::West),
        boundary(SurfaceIso::East),
        lone_naked(start),
        lone_naked(end),
    ) {
        let combined =
            Brep::try_combine(vec![wall.clone(), start.clone(), end.clone()], tolerance)?;
        let offset = wall.edges().len();
        let pairs = [
            (west, offset + start_edge, false),
            (east, offset + start.edges().len() + end_edge, false),
        ];
        if let Ok(result) = combined.try_join_edge_pairs(&pairs, tolerance.absolute(), tolerance)
            && result.is_closed()
            && result.is_solid()
        {
            return Ok(result);
        }
    }
    let mut joined = join_breps(&[wall, start, end], tolerance.absolute(), tolerance)?;
    if joined.len() != 1 {
        return Err(CommandError::Usage(USAGE));
    }
    let brep = joined.pop().expect("one joined component was checked").brep;
    if !brep.is_closed() || !brep.is_solid() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(brep)
}

fn cap_wall(wall: Brep, cap: PipeCap, tolerance: Tolerance) -> Result<Brep, CommandError> {
    if cap == PipeCap::None || wall.is_closed() {
        return Ok(wall);
    }
    let capped = wall
        .try_cap_planar_holes(tolerance)?
        .ok_or(CommandError::Usage(USAGE))?;
    if !capped.is_closed() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(capped)
}

fn finish_two_walls(
    outer: Brep,
    inner: Brep,
    cap: PipeCap,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let combined = Brep::try_combine(vec![outer, inner.reversed()], tolerance)?;
    cap_wall(combined, cap, tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{
        CircularArc3, CurveSegment3, LineSegment, NurbsCurve, PolyCurve3, UnitVector3,
    };

    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn straight_round_pipe_matches_local_and_global_rhino_volumes() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        let slope = 0.2_f64;
        let sphere_scale = slope.hypot(1.0);
        let spherical_cap_volume = |radius: Real, segment_height: Real| {
            std::f64::consts::PI
                * segment_height.powi(2)
                * (radius * sphere_scale - segment_height / 3.0)
        };
        let global = std::f64::consts::PI * 5.0 * 7.0 / 3.0
            + spherical_cap_volume(1.0, sphere_scale + slope)
            + spherical_cap_volume(2.0, 2.0 * (sphere_scale - slope));
        for (arguments, expected) in [
            (
                "1",
                std::f64::consts::PI * 5.0 + 4.0 * std::f64::consts::PI / 3.0,
            ),
            (
                "1 2",
                std::f64::consts::PI * 5.0 * (2.0 + 13.0 / 35.0)
                    + 2.0 * std::f64::consts::PI * 9.0 / 3.0,
            ),
            (
                "2 1",
                std::f64::consts::PI * 5.0 * (2.0 + 13.0 / 35.0)
                    + 2.0 * std::f64::consts::PI * 9.0 / 3.0,
            ),
            ("1 2 ShapeBlending=Global", global),
            ("2 1 ShapeBlending=Global", global),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} {arguments} Cap=Round"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("round pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            assert_eq!(pipe.faces().len(), 3);
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() / expected < 1e-8,
                "{measured} vs {expected}"
            );
        }
    }

    #[test]
    fn straight_round_thick_pipe_matches_rhino_planar_caps() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        CommandRegistry::with_builtins()
            .execute(
                &mut document,
                &format!("Pipe {source} 1 WallThickness=0.5 Cap=Round"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("round thick pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        assert_eq!(pipe.faces().len(), 4);
        let expected = std::f64::consts::PI * (1.5_f64.powi(2) - 1.0) * 5.0;
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
    }

    #[test]
    fn straight_pipe_uses_local_cubic_and_global_conical_radii() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("Pipe {source} 1"))
            .unwrap();
        let Geometry::Brep(cylinder) = document.objects().last().unwrap().geometry() else {
            panic!("flat pipe should be a B-rep")
        };
        assert!(cylinder.is_closed());
        assert_eq!(cylinder.faces().len(), 3);
        registry
            .execute(&mut document, &format!("Pipe {source} 1 2 Cap=None"))
            .unwrap();
        let Geometry::NurbsSurface(local) = document.objects().last().unwrap().geometry() else {
            panic!("open local pipe should be a NURBS surface")
        };
        assert_eq!(local.degree_v(), 3);
        let sample = local
            .evaluate(*local.domain_u().start(), 0.2 * *local.domain_v().end())
            .unwrap();
        assert!((sample.z() - 1.0).abs() < 1e-10);
        assert!((sample.x().hypot(sample.y()) - 1.104).abs() < 1e-10);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 2 Cap=None ShapeBlending=Global"),
            )
            .unwrap();
        let Geometry::NurbsSurface(global) = document.objects().last().unwrap().geometry() else {
            panic!("open global pipe should be a NURBS surface")
        };
        assert_eq!(global.degree_v(), 1);
        let sample = global
            .evaluate(*global.domain_u().start(), 0.2 * *global.domain_v().end())
            .unwrap();
        assert!((sample.z() - 1.0).abs() < 1e-10);
        assert!((sample.x().hypot(sample.y()) - 1.2).abs() < 1e-10);
        registry
            .execute(&mut document, &format!("Pipe {source} 1 2 Cap=Flat"))
            .unwrap();
        let Geometry::Brep(frustum) = document.objects().last().unwrap().geometry() else {
            panic!("flat tapered pipe should be a B-rep")
        };
        assert!(frustum.is_closed());
        assert_eq!(frustum.faces().len(), 3);
    }

    #[test]
    fn straight_thick_pipe_has_annular_caps_and_signed_wall_thickness() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("Pipe {source} 1 WallThickness=0.5"))
            .unwrap();
        let Geometry::Brep(tube) = document.objects().last().unwrap().geometry() else {
            panic!("flat thick pipe should be a B-rep")
        };
        assert!(tube.is_closed());
        assert!(tube.is_solid());
        assert_eq!(tube.faces().len(), 4);
        let expected = std::f64::consts::PI * (1.5_f64.powi(2) - 1.0) * 5.0;
        assert!((tube.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-6);

        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 2 WallThickness=-0.25 Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(tapered) = document.objects().last().unwrap().geometry() else {
            panic!("flat tapered thick pipe should be a B-rep")
        };
        assert!(tapered.is_closed());
        assert!(tapered.is_solid());
        assert_eq!(tapered.faces().len(), 4);
        let frustum_volume =
            |a: Real, b: Real| std::f64::consts::PI * 5.0 / 3.0 * (a * a + a * b + b * b);
        let expected = frustum_volume(1.0, 2.0) - frustum_volume(0.75, 1.75);
        let measured = tapered.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 2 WallThickness=0.25 Cap=None"),
            )
            .unwrap();
        let Geometry::Brep(open) = document.objects().last().unwrap().geometry() else {
            panic!("open thick pipe should be a B-rep")
        };
        assert!(!open.is_closed());
        assert_eq!(open.faces().len(), 2);
    }

    #[test]
    fn circle_rail_makes_closed_torus_and_preserves_source() {
        let mut document = Document::default();
        let circle = Circle3::try_from_frame(
            p(4., -2., 1.),
            3.,
            UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT).unwrap(),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Circle(circle)).unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Pipe 0.5").unwrap();
        let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
            panic!("circle rail should create a torus surface")
        };
        assert!(surface.is_closed_u().unwrap());
        assert!(surface.is_closed_v().unwrap());
        assert_eq!(document.objects().count(), 2);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 WallThickness=0.2"),
            )
            .unwrap();
        let Geometry::Brep(thick) = document.objects().last().unwrap().geometry() else {
            panic!("thick circular pipe should be a B-rep")
        };
        assert!(thick.is_closed());
        assert!(thick.is_solid());
        assert_eq!(thick.faces().len(), 2);
        let expected =
            2.0 * std::f64::consts::PI.powi(2) * 3.0 * (0.7_f64.powi(2) - 0.5_f64.powi(2));
        let measured = thick.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
    }

    #[test]
    fn smooth_arc_rail_builds_capped_and_open_pipe() {
        let mut document = Document::default();
        let diagonal = 5. * std::f64::consts::FRAC_1_SQRT_2;
        let arc = CircularArc3::try_from_three_points(
            p(5., 0., 0.),
            p(diagonal, diagonal, 0.),
            p(0., 5., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, &format!("Pipe {source} 0.5"))
            .unwrap();
        let Geometry::Brep(capped) = document.objects().last().unwrap().geometry() else {
            panic!("arc pipe should be a B-rep")
        };
        assert!(capped.is_closed());
        assert!(capped.is_solid());
        let expected = 0.5 * std::f64::consts::PI.powi(2) * 5.0 * 0.5_f64.powi(2);
        let measured = capped.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
        let wall = capped.faces()[0].surface();
        let u = (*wall.domain_u().start() + *wall.domain_u().end()) * 0.5;
        let v = (*wall.domain_v().start() + *wall.domain_v().end()) * 0.5;
        let sample = wall.evaluate(u, v).unwrap();
        let rail = CurveRef::Arc(&arc);
        let nearest = rail
            .evaluate(rail.closest_parameter(sample, Tolerance::DEFAULT).unwrap())
            .unwrap();
        assert!((sample.distance_to(nearest).unwrap() - 0.5).abs() < 5e-3);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 0.8 Cap=None ShapeBlending=Global"),
            )
            .unwrap();
        let Geometry::Brep(open) = document.objects().last().unwrap().geometry() else {
            panic!("arc pipe should be a B-rep")
        };
        assert!(!open.is_closed());
        let wall = open.faces()[0].surface();
        let u = (*wall.domain_u().start() + *wall.domain_u().end()) * 0.5;
        let v = (*wall.domain_v().start() + *wall.domain_v().end()) * 0.5;
        let sample = wall.evaluate(u, v).unwrap();
        let nearest = rail
            .evaluate(rail.closest_parameter(sample, Tolerance::DEFAULT).unwrap())
            .unwrap();
        assert!((sample.distance_to(nearest).unwrap() - 0.65).abs() < 5e-3);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 WallThickness=0.2 Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(thick) = document.objects().last().unwrap().geometry() else {
            panic!("thick arc pipe should be a B-rep")
        };
        assert!(thick.is_closed());
        assert!(thick.is_solid());
        assert_eq!(thick.faces().len(), 6);
        let expected =
            0.5 * std::f64::consts::PI.powi(2) * 5.0 * (0.7_f64.powi(2) - 0.5_f64.powi(2));
        let measured = thick.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
        registry
            .execute(&mut document, &format!("Pipe {source} 0.5 Cap=Round"))
            .unwrap();
        let Geometry::Brep(round) = document.objects().last().unwrap().geometry() else {
            panic!("round arc pipe should be a B-rep")
        };
        assert!(round.is_closed());
        assert!(round.is_solid());
        let expected = 0.5 * std::f64::consts::PI.powi(2) * 5.0 * 0.5_f64.powi(2)
            + 4.0 * std::f64::consts::PI * 0.5_f64.powi(3) / 3.0;
        let measured = round.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 WallThickness=0.2 Cap=Round"),
            )
            .unwrap();
        let Geometry::Brep(round_thick) = document.objects().last().unwrap().geometry() else {
            panic!("round thick arc pipe should be a B-rep")
        };
        assert!(round_thick.is_closed());
        assert!(round_thick.is_solid());
        let expected =
            0.5 * std::f64::consts::PI.powi(2) * 5.0 * (0.7_f64.powi(2) - 0.5_f64.powi(2));
        let measured = round_thick.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
    }

    #[test]
    fn tapered_arc_round_pipe_matches_rhino_volume() {
        let mut document = Document::default();
        let diagonal = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
        let arc = CircularArc3::try_from_three_points(
            p(5.0, 0.0, 0.0),
            p(diagonal, diagonal, 0.0),
            p(0.0, 5.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (blend, reference_volume) in
            [("Local", 12.02855252569316), ("Global", 11.891434174037984)]
        {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.5 0.8 Cap=Round ShapeBlending={blend}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("tapered arc round pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - reference_volume).abs() < 5e-6,
                "{blend}: {measured} vs {reference_volume}"
            );
        }
    }

    #[test]
    fn spatial_nurbs_rail_builds_closed_thick_pipe() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut document,
                "InterpCrv 0,0,0 2,1,1 4,0,2 6,-1,3 Knots=Chord Close=Open",
            )
            .unwrap();
        let source = document.objects().next().unwrap().id();
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 WallThickness=0.1 Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(thick) = document.objects().last().unwrap().geometry() else {
            panic!("spatial thick pipe should be a B-rep")
        };
        assert!(thick.is_solid());
        assert!(thick.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 WallThickness=0.1 Cap=Round"),
            )
            .unwrap();
        let Geometry::Brep(round) = document.objects().last().unwrap().geometry() else {
            panic!("spatial round thick pipe should be a B-rep")
        };
        assert!(round.is_closed());
        assert!(round.is_solid());
        assert!(round.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 0.4 Cap=Round ShapeBlending=Global"),
            )
            .unwrap();
        let Geometry::Brep(tapered) = document.objects().last().unwrap().geometry() else {
            panic!("spatial tapered pipe should be a B-rep")
        };
        assert!(tapered.is_closed());
        assert!(tapered.is_solid());
        assert!(tapered.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);
    }

    #[test]
    fn straight_local_radius_station_matches_rhino_flat_and_thick_pipes() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        for (suffix, expected, faces) in [
            ("", 37.250313352254224, 3),
            ("WallThickness=0.5", 27.488936058139387, 4),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 1 Stations=0.5:2 Cap=Flat {suffix}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("station pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            assert_eq!(pipe.faces().len(), faces);
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 1e-6,
                "{measured} vs {expected}"
            );
        }
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 Stations=0.5:2 Cap=None"),
            )
            .unwrap();
        let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
            panic!("open station pipe should be a surface")
        };
        for (z, radius) in [(0.5, 1.104), (2.5, 2.0), (4.5, 1.104)] {
            let point = surface.evaluate(z, *surface.domain_v().start()).unwrap();
            assert!((point.x().hypot(point.y()) - radius).abs() < 1e-10);
        }
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 Stations=0.25:1.5,0.75:2 Cap=None"),
            )
            .unwrap();
        let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
            panic!("multi-station pipe should be a surface")
        };
        for (z, radius) in [(1.25, 1.5), (3.75, 2.0), (5.0, 1.0)] {
            let point = surface.evaluate(z, *surface.domain_v().start()).unwrap();
            assert!((point.x().hypot(point.y()) - radius).abs() < 1e-10);
        }
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 Stations=0.5:2 Cap=Round"),
            )
            .unwrap();
        let Geometry::Brep(round) = document.objects().last().unwrap().geometry() else {
            panic!("round station pipe should be a B-rep")
        };
        assert!(round.is_closed());
        assert!(round.is_solid());
        let expected = 41.439103495320765;
        let measured = round.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 1e-6,
            "{measured} vs {expected}"
        );
    }

    #[test]
    fn straight_local_stations_match_rhino_volumes() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        for (end_radius, stations, expected) in [
            (1.0, "0.5:2", 37.250313352254224),
            (1.2, "0.25:1.5,0.75:2", 40.99960133023945),
            (2.0, "0.25:1.4,0.75:1.8", 40.1960120376526),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!(
                        "Pipe {source} 1 {end_radius} Stations={stations} ShapeBlending=Local Cap=Flat"
                    ),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("station pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{stations}");
            assert!(pipe.is_solid(), "{stations}");
            assert_eq!(pipe.faces().len(), 3, "{stations}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{stations}: {measured} vs {expected}"
            );
        }
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 1.2 Stations=0.25:1.5,0.75:2 Cap=None"),
            )
            .unwrap();
        let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
            panic!("open station pipe should be a surface")
        };
        assert_eq!(surface.degree_u(), 3);
        assert_eq!(surface.control_point_count_u(), 10);
        for (index, expected) in [
            1.0,
            0.994613743,
            1.276931283,
            1.538819894,
            1.577445556,
            1.88731348,
            2.107178385,
            1.629006755,
            1.194198649,
            1.2,
        ]
        .into_iter()
        .enumerate()
        {
            let point = surface.control_point(index, 0).unwrap().point();
            assert!(
                (point.x().hypot(point.y()) - expected).abs() < 2e-5,
                "control {index}: {point:?} vs radius {expected}"
            );
        }
    }

    #[test]
    fn arc_local_radius_station_matches_rhino_flat_pipe() {
        let mut document = Document::default();
        let diagonal = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
        let arc = CircularArc3::try_from_three_points(
            p(5.0, 0.0, 0.0),
            p(diagonal, diagonal, 0.0),
            p(0.0, 5.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
        CommandRegistry::with_builtins()
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 Stations=0.5:0.8 Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("arc station pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        assert_eq!(pipe.faces().len(), 3);
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        let expected = 10.694421898601524;
        assert!(
            (measured - expected).abs() < 5e-6,
            "{measured} vs {expected}"
        );
    }

    #[test]
    fn bezier_rail_pipe_matches_rhino_flat_volumes() {
        let mut document = Document::default();
        let rail = NurbsCurve::try_new(
            3,
            vec![p(0., 0., 0.), p(2., 0., 0.), p(3., 2., 1.), p(5., 0., 2.)],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let source = document.add_geometry(Geometry::NurbsCurve(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (suffix, expected) in [
            ("", 1.6495215095113431),
            ("0.5", 3.021505338976187),
            ("0.5 ShapeBlending=Global", 2.9935770434779623),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 {suffix} Cap=Flat"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("Bézier pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{suffix}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn multispan_rail_pipe_matches_rhino_flat_volumes() {
        let mut document = Document::default();
        let rail = NurbsCurve::try_new(
            3,
            vec![
                p(0., 0., 0.),
                p(1., 1., 0.),
                p(2., -1., 1.),
                p(3., 2., 1.),
                p(5., 0., 2.),
            ],
            vec![0., 0., 0., 0., 0.5, 1., 1., 1., 1.],
        )
        .unwrap();
        let source = document.add_geometry(Geometry::NurbsCurve(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (suffix, expected) in [("", 1.7380221094021495), ("0.5", 3.183615713670431)] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 {suffix} Cap=Flat"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("multispan pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{suffix}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn tangent_line_arc_pipe_matches_rhino_fit_rail_modes() {
        let mut document = Document::default();
        let diagonal = 2.0_f64.sqrt();
        let line = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), Tolerance::DEFAULT).unwrap();
        let arc = CircularArc3::try_from_three_points(
            p(2., 0., 0.),
            p(2. + diagonal, 2. - diagonal, 0.),
            p(4., 2., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let rail =
            PolyCurve3::try_new(vec![CurveSegment3::Line(line), CurveSegment3::Arc(arc)]).unwrap();
        let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (options, expected, faces) in [
            ("", 1.4537510629312786, 4),
            ("FitRail=Yes", 1.4537510834790615, 3),
            ("0.5", 2.5796673434153528, 4),
            ("0.5 FitRail=Yes", 2.6629032304651, 3),
            ("0.5 ShapeBlending=Global", 2.5719585204790043, 4),
            (
                "0.5 ShapeBlending=Global FitRail=Yes",
                2.6382894558171768,
                3,
            ),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 {options} Cap=Flat"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("polycurve pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            assert_eq!(pipe.faces().len(), faces);
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
        for (options, expected, faces) in [
            ("Cap=Round", 2.8980153924831518, 4),
            ("WallThickness=0.2 Cap=Flat", 3.1964619294525423, 6),
        ] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 0.5 {options}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("polycurve pipe should be a B-rep")
            };
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            assert_eq!(pipe.faces().len(), faces);
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
        for (options, expected) in [
            ("Stations=0.25:0.65", 5.1070735157919955),
            ("Stations=0.38898452964834274:0.65", 4.801208532983822),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 0.5 {options} Cap=Flat"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("polycurve station pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            assert_eq!(pipe.faces().len(), 4, "{options}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
        let count = document.objects().count();
        assert!(
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 0.5 Stations=0.5:0.65 Cap=Flat"),
                )
                .is_err()
        );
        assert_eq!(document.objects().count(), count);
        assert!(
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 0.5 Stations=0.5:0.65 FitRail=Yes Cap=Flat"),
                )
                .is_err()
        );
        assert_eq!(document.objects().count(), count);
    }

    #[test]
    fn sharp_two_line_pipe_matches_rhino_miter() {
        let mut document = Document::default();
        let first = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), Tolerance::DEFAULT).unwrap();
        let second =
            LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), Tolerance::DEFAULT).unwrap();
        let rail = PolyCurve3::try_new(vec![
            CurveSegment3::Line(first),
            CurveSegment3::Line(second),
        ])
        .unwrap();
        let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (options, expected, faces) in [
            ("Cap=Flat", 1.1309737513007323, 4),
            ("Cap=Round", 1.2440711643189903, 4),
            ("WallThickness=0.2 Cap=Flat", 2.010619896220259, 6),
        ] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("mitered pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            assert_eq!(pipe.faces().len(), faces, "{options}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn oblique_three_dimensional_miter_matches_rhino_volume() {
        let mut document = Document::default();
        let first = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), Tolerance::DEFAULT).unwrap();
        let second =
            LineSegment::try_new(p(2., 0., 0.), p(3., 1., 1.), Tolerance::DEFAULT).unwrap();
        let rail = PolyCurve3::try_new(vec![
            CurveSegment3::Line(first),
            CurveSegment3::Line(second),
        ])
        .unwrap();
        let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (cap, expected) in [("Flat", 1.0552124780965215), ("Round", 1.1683098049457938)] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 Cap={cap}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("oblique pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{cap}");
            assert!(pipe.is_solid(), "{cap}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{cap}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn sharp_polyline_pipe_matches_rhino_miter() {
        let mut document = Document::default();
        let rail = Polyline3::try_new(
            vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (cap, expected) in [("Flat", 1.1309737513007323), ("Round", 1.2440711643189903)] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 Cap={cap}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("mitered polyline pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{cap}");
            assert!(pipe.is_solid(), "{cap}");
            assert_eq!(pipe.faces().len(), 4, "{cap}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{cap}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn three_segment_polyline_pipe_matches_rhino_miters() {
        let mut document = Document::default();
        let rail = Polyline3::try_new(
            vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(4., 2., 0.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (options, expected, faces) in [
            ("Cap=Flat", 1.6964605759770885, 5),
            ("Cap=Round", 1.8095579066239338, 5),
            ("WallThickness=0.2 Cap=Flat", 3.0159299051156543, 8),
        ] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("three-segment pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            assert_eq!(pipe.faces().len(), faces, "{options}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn spatial_three_segment_polyline_pipe_matches_rhino_miters() {
        let mut document = Document::default();
        let rail = Polyline3::try_new(
            vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(2., 2., 2.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (options, expected, faces) in [
            ("Cap=Flat", 1.6964609638052046, 5),
            ("Cap=Round", 1.8095583636292623, 5),
            ("WallThickness=0.2 Cap=Flat", 3.0159304945160126, 8),
        ] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("spatial three-segment pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            assert_eq!(pipe.faces().len(), faces, "{options}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn skew_spatial_polyline_pipe_matches_rhino_volume() {
        let mut document = Document::default();
        let rail = Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(2., 0., 0.),
                p(3., 1., 0.5),
                p(4., 1.3, 1.8),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (options, expected) in [
            ("Cap=Flat", 1.461029031976258),
            ("Cap=Round", 1.574126366134179),
            ("WallThickness=0.2 Cap=Flat", 2.5973849459504827),
        ] {
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("skew spatial pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
    }

    #[test]
    fn fitted_sharp_constant_radius_pipes_match_rhino_volume() {
        let tolerance = Tolerance::DEFAULT;
        let first = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap();
        let second = LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), tolerance).unwrap();
        let two_lines = PolyCurve3::try_new(vec![
            CurveSegment3::Line(first),
            CurveSegment3::Line(second),
        ])
        .unwrap();
        let rails = [
            (
                "two lines",
                Geometry::PolyCurve(two_lines),
                [1.130973357832259, 1.2440706907864723, 2.0106193005915816],
            ),
            (
                "planar polyline",
                Geometry::Polyline(
                    Polyline3::try_new(
                        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(4., 2., 0.)],
                        tolerance,
                    )
                    .unwrap(),
                ),
                [1.696460042103737, 1.8095573727505831, 3.0159289603031096],
            ),
            (
                "orthogonal spatial polyline",
                Geometry::Polyline(
                    Polyline3::try_new(
                        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(2., 2., 2.)],
                        tolerance,
                    )
                    .unwrap(),
                ),
                [1.696460048805896, 1.8095573844488695, 3.0159289406129766],
            ),
            (
                "skew spatial polyline",
                Geometry::Polyline(
                    Polyline3::try_new(
                        vec![
                            p(0., 0., 0.),
                            p(2., 0., 0.),
                            p(3., 1., 0.5),
                            p(4., 1.3, 1.8),
                        ],
                        tolerance,
                    )
                    .unwrap(),
                ),
                [1.4610290445769174, 1.5741263764973636, 2.5973849605325876],
            ),
        ];
        let registry = CommandRegistry::with_builtins();
        let options = ["Cap=Flat", "Cap=Round", "WallThickness=0.2 Cap=Flat"];
        for (name, rail, expected) in rails {
            let mut document = Document::default();
            let source = document.add_geometry(rail).unwrap();
            for (option, volume) in options.into_iter().zip(expected) {
                registry
                    .execute(
                        &mut document,
                        &format!("Pipe {source} 0.3 FitRail=Yes {option}"),
                    )
                    .unwrap();
                let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                    panic!("{name}: fitted pipe should be a B-rep")
                };
                assert!(pipe.is_closed(), "{name}: {option}");
                assert!(pipe.is_solid(), "{name}: {option}");
                let measured = pipe.signed_volume(tolerance).unwrap();
                assert!(
                    (measured - volume).abs() < 5e-6,
                    "{name}: {option}: {measured} vs {volume}"
                );
            }
        }
    }

    #[test]
    fn tapered_sharp_fit_profile_matches_rhino_volume() {
        let tolerance = Tolerance::DEFAULT;
        let mut mismatches = Vec::new();
        for (vertices, expected) in [
            (
                vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.)],
                [
                    2.0716563894655327,
                    2.0525075453971313,
                    2.3900044399127722,
                    2.5132743510996836,
                    2.5132743406284277,
                ],
            ),
            (
                vec![p(0., 0., 0.), p(2., 0., 0.), p(3., 1., 1.)],
                [
                    1.9328814905368568,
                    1.9150153216013213,
                    2.251229540274963,
                    2.34491674771564,
                    2.3449167040785506,
                ],
            ),
            (
                vec![
                    p(0., 0., 0.),
                    p(2., 0., 0.),
                    p(3., 1., 0.5),
                    p(4., 1.3, 1.8),
                ],
                [
                    2.676234466580245,
                    2.6514974679478795,
                    2.9945825138115723,
                    3.2467313600092877,
                    3.2467315788085314,
                ],
            ),
        ] {
            let lines = vertices
                .windows(2)
                .map(|pair| LineSegment::try_new(pair[0], pair[1], tolerance).unwrap())
                .collect::<Vec<_>>();
            let options = [
                (SweepBlend::Local, PipeCap::Flat, None),
                (SweepBlend::Global, PipeCap::Flat, None),
                (SweepBlend::Local, PipeCap::Round, None),
                (SweepBlend::Local, PipeCap::Flat, Some(0.2)),
                (SweepBlend::Global, PipeCap::Flat, Some(0.2)),
            ];
            for ((blend, cap, thickness), expected) in options.into_iter().zip(expected) {
                let pipe = mitered_line_pipe_profile(
                    &lines,
                    [0.3, 0.5],
                    blend,
                    cap,
                    thickness,
                    false,
                    tolerance,
                )
                .unwrap()
                .unwrap();
                assert!(pipe.is_closed());
                assert!(pipe.is_solid());
                let measured = pipe.signed_volume(tolerance).unwrap();
                if (measured - expected).abs() >= 5e-6 {
                    mismatches.push(format!(
                        "{blend:?} {cap:?} {thickness:?}: {measured} vs {expected}"
                    ));
                }
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }

    #[test]
    fn fitted_tapered_miter_tracks_rhino_wall_at_matched_axial_positions() {
        let tolerance = Tolerance::DEFAULT;
        let lines = [
            LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap(),
            LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), tolerance).unwrap(),
        ];
        let directions = lines.map(|line| line.direction(tolerance).unwrap().as_vector());
        let seam = p(0., 0., 0.).vector_to(p(0., 0., 1.)).unwrap();
        let first_frame =
            Frame3::try_from_x_and_normal(lines[0].start(), seam, directions[0], tolerance)
                .unwrap();
        let second_frame =
            Frame3::try_from_x_and_normal(lines[1].start(), seam, directions[1], tolerance)
                .unwrap();
        let surfaces = [
            mitered_line_profile_surface(
                lines[0],
                first_frame,
                [0.3, 0.5],
                0.0,
                SweepBlend::Local,
                [0.0, 0.5],
                None,
                Some(directions),
                tolerance,
            )
            .unwrap(),
            mitered_line_profile_surface(
                lines[1],
                second_frame,
                [0.3, 0.5],
                0.0,
                SweepBlend::Local,
                [0.5, 1.0],
                Some(directions),
                None,
                tolerance,
            )
            .unwrap(),
        ];
        let references = [
            [
                (0.4382538993558626, 0.30859375),
                (0.8632822913683168, 0.33125),
                (1.2566695376966126, 0.36328125),
            ],
            [
                (0.7923368998234105, 0.43671875),
                (1.1936672523437442, 0.46875),
                (1.5981639786922057, 0.49140625),
            ],
        ];
        for (index, surface) in surfaces.iter().enumerate() {
            let circle_parameter = 3.0 * std::f64::consts::FRAC_PI_2;
            for (axial, expected_radius) in references[index] {
                let mut low = 0.0;
                let mut high = 2.0;
                for _ in 0..60 {
                    let middle = 0.5 * (low + high);
                    let point = surface.evaluate(circle_parameter, middle).unwrap();
                    let measured_axial = point.to_array()[index];
                    if measured_axial < axial {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                let point = surface
                    .evaluate(circle_parameter, 0.5 * (low + high))
                    .unwrap();
                let measured_radius = if index == 0 {
                    point.y()
                } else {
                    2.0 - point.x()
                };
                assert!(
                    (measured_radius - expected_radius).abs() < 1e-4,
                    "segment {index}, axial {axial}: {measured_radius} vs {expected_radius}"
                );
            }
        }
    }

    #[test]
    fn fitted_tapered_sharp_pipe_command_uses_miter_profile() {
        let tolerance = Tolerance::DEFAULT;
        let mut document = Document::default();
        let lines = [
            LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap(),
            LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), tolerance).unwrap(),
        ];
        let rail =
            PolyCurve3::try_new(lines.into_iter().map(CurveSegment3::Line).collect()).unwrap();
        let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (options, expected) in [
            ("Cap=Flat", 2.0716563894655327),
            ("ShapeBlending=Global Cap=Flat", 2.0525075453971313),
            ("Cap=Round", 2.3900044399127722),
            ("ShapeBlending=Global Cap=Round", 2.3566305015018547),
            ("WallThickness=0.2 Cap=Flat", 2.5132743510996836),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 0.5 FitRail=Yes {options}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("fitted tapered pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            let measured = pipe.signed_volume(tolerance).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
        let short_rail = PolyCurve3::try_new(vec![
            CurveSegment3::Line(
                LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap(),
            ),
            CurveSegment3::Line(
                LineSegment::try_new(
                    p(2., 0., 0.),
                    p(3., std::f64::consts::SQRT_2, 0.),
                    tolerance,
                )
                .unwrap(),
            ),
        ])
        .unwrap();
        let short_source = document
            .add_geometry(Geometry::PolyCurve(short_rail))
            .unwrap();
        registry
            .execute(
                &mut document,
                &format!("Pipe {short_source} 0.3 0.5 FitRail=Yes ShapeBlending=Global Cap=Round"),
            )
            .unwrap();
        let Geometry::Brep(short_pipe) = document.objects().last().unwrap().geometry() else {
            panic!("short horizontal pipe should be a B-rep")
        };
        assert!(short_pipe.is_solid());
        let measured = short_pipe.signed_volume(tolerance).unwrap();
        assert!((measured - 2.2182048402524526).abs() < 5e-6);

        let skew = Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(2., 0., 0.),
                p(3., 1., 0.5),
                p(4., 1.3, 1.8),
            ],
            tolerance,
        )
        .unwrap();
        let skew_id = document.add_geometry(Geometry::Polyline(skew)).unwrap();
        for (options, expected) in [
            ("Cap=Round", 2.9945825138115723),
            (
                "ShapeBlending=Global WallThickness=0.2 Cap=Flat",
                3.2467315788085314,
            ),
            (
                "ShapeBlending=Global WallThickness=0.2 Cap=Round",
                3.2467315788085314,
            ),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {skew_id} 0.3 0.5 FitRail=Yes {options}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("fitted spatial polyline pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{options}");
            assert!(pipe.is_solid(), "{options}");
            let measured = pipe.signed_volume(tolerance).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{options}: {measured} vs {expected}"
            );
        }
        let count = document.objects().count();
        assert!(
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {skew_id} 0.3 0.5 FitRail=Yes ShapeBlending=Global Cap=Round"),
                )
                .is_err()
        );
        assert_eq!(document.objects().count(), count);
    }

    #[test]
    fn fitted_closed_rectangle_pipe_matches_rhino_volume() {
        let tolerance = Tolerance::DEFAULT;
        let rail = Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(3., 0., 0.),
                p(3., 2., 0.),
                p(0., 2., 0.),
                p(0., 0., 0.),
            ],
            tolerance,
        )
        .unwrap();
        let mut document = Document::default();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        let registry = CommandRegistry::with_builtins();
        for (option, expected) in [
            ("Cap=None", 2.8274333878128886),
            ("Cap=Flat", 2.8274333878128886),
            ("Cap=Round", 2.8274333878128886),
            ("WallThickness=0.2 Cap=None", 5.02654824276054),
            ("WallThickness=0.2 Cap=Flat", 5.02654824276054),
            ("WallThickness=0.2 Cap=Round", 5.02654824276054),
        ] {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 FitRail=Yes {option}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("closed rectangular pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{option}");
            assert!(pipe.is_solid(), "{option}");
            assert_eq!(
                pipe.faces().len(),
                if option.contains("WallThickness") {
                    2
                } else {
                    1
                }
            );
            let wall = pipe.faces()[0].surface();
            assert_eq!((wall.degree_u(), wall.degree_v()), (3, 2));
            assert!(wall.is_closed_u().unwrap());
            assert!(wall.is_closed_v().unwrap());
            assert_eq!(
                (wall.control_point_count_u(), wall.control_point_count_v()),
                (16, 9)
            );
            let measured = pipe.signed_volume(tolerance).unwrap();
            assert!(
                (measured - expected).abs() < 5e-6,
                "{option}: {measured} vs {expected}"
            );
        }
        let count = document.objects().count();
        assert!(
            registry
                .execute(&mut document, &format!("Pipe {source} 0.3 Cap=Flat"))
                .is_err()
        );
        assert_eq!(document.objects().count(), count);
    }

    #[test]
    fn fitted_closed_triangle_and_spatial_loop_make_solid_pipes() {
        let tolerance = Tolerance::DEFAULT;
        let registry = CommandRegistry::with_builtins();
        for (name, vertices, rhino_volumes, planar) in [
            (
                "triangle",
                vec![p(0., 0., 0.), p(3., 0., 0.), p(1.5, 2.5, 0.), p(0., 0., 0.)],
                [2.4968928233655134, 4.438920567236832],
                true,
            ),
            (
                "spatial loop",
                vec![
                    p(0., 0., 0.),
                    p(3., 0., 0.),
                    p(3., 2., 1.),
                    p(0., 2., 0.),
                    p(0., 0., 0.),
                ],
                [2.9073922697381915, 5.168697873404391],
                false,
            ),
        ] {
            let perimeter = vertices
                .windows(2)
                .map(|pair| pair[0].distance_to(pair[1]).unwrap())
                .sum::<Real>();
            let mut document = Document::default();
            let rail = Polyline3::try_new(vertices, tolerance).unwrap();
            let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
            for (index, (options, area)) in [
                ("", std::f64::consts::PI * 0.3_f64.powi(2)),
                (
                    "WallThickness=0.2",
                    std::f64::consts::PI * (0.5_f64.powi(2) - 0.3_f64.powi(2)),
                ),
            ]
            .into_iter()
            .enumerate()
            {
                registry
                    .execute(
                        &mut document,
                        &format!("Pipe {source} 0.3 FitRail=Yes Cap=Flat {options}"),
                    )
                    .unwrap();
                let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                    panic!("closed loop pipe should be a B-rep")
                };
                assert!(pipe.is_closed(), "{name} {options}");
                assert!(pipe.is_solid(), "{name} {options}");
                assert_eq!(pipe.faces().len(), if index == 0 { 1 } else { 2 });
                let wall = pipe.faces()[0].surface();
                assert_eq!((wall.degree_u(), wall.degree_v()), (3, 2));
                assert_eq!(wall.control_point_count_v(), 9);
                let measured = pipe.signed_volume(tolerance).unwrap();
                if planar {
                    assert!(
                        (measured - perimeter * area).abs() < 5e-6,
                        "{name} {options}: {measured} vs analytic {}",
                        perimeter * area,
                    );
                }
                assert!(
                    (measured - rhino_volumes[index]).abs() < 5e-6,
                    "{name} {options}: {measured} vs Rhino {}",
                    rhino_volumes[index]
                );
            }
        }
    }

    #[test]
    fn fitted_closed_spatial_pipe_tracks_rhino_wall_samples() {
        let tolerance = Tolerance::DEFAULT;
        let rail = Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(3., 0., 0.),
                p(3., 2., 1.),
                p(0., 2., 0.),
                p(0., 0., 0.),
            ],
            tolerance,
        )
        .unwrap();
        let mut document = Document::default();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        CommandRegistry::with_builtins()
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 FitRail=Yes Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("closed spatial pipe should be a B-rep")
        };
        let samples = [
            p(2.580779172514338, 0.29944294122962345, 0.003748513965577651),
            p(
                2.905837102170516,
                0.003748513965577929,
                -0.29944294122962267,
            ),
            p(2.727387704892325, 1.0338568689426977, 0.3815722284741339),
            p(3.2726122951076744, 0.933142250924031, 0.6019273314592302),
            p(1.6303572709321312, 1.7353380686061608, 0.4019940648509084),
            p(1.5936022690395901, 2.264661931393839, 0.6726591151396656),
            p(
                0.29797433174408344,
                1.0756291590617515,
                -0.01363095987482699,
            ),
            p(
                -0.2979743317440834,
                1.1235398334008513,
                0.013630959874826981,
            ),
            p(
                0.44352697699132704,
                0.29944294122962295,
                -0.0037485139655774264,
            ),
            p(
                -0.0430983639715959,
                -0.29944294122962295,
                0.003748513965577389,
            ),
        ];
        for sample in samples {
            let nearest = pipe
                .faces()
                .iter()
                .filter_map(|face| {
                    let (u, v) = face.surface().closest_parameters(sample, tolerance).ok()?;
                    let candidate = face.surface().evaluate(u, v).ok()?;
                    sample.distance_to(candidate).ok()
                })
                .fold(Real::INFINITY, Real::min);
            assert!(nearest < 5e-4, "{sample:?}: gap {nearest}");
        }
    }

    #[test]
    fn invalid_inputs_leave_document_unchanged() {
        let mut document = Document::default();
        let source = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        let originals = document.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        for command in [
            "Pipe",
            "Pipe 0",
            "Pipe -1",
            "Pipe NaN",
            "Pipe 1 Cap=Rounding",
            "Pipe 1 Cap=Flat Cap=None",
            "Pipe 1 ShapeBlending=Other",
            "Pipe 1 FitRail=Maybe",
            "Pipe 1 FitRail=Yes FitRail=No",
            "Pipe 1 WallThickness=0",
            "Pipe 1 WallThickness=-1",
            "Pipe 1 WallThickness=NaN",
            "Pipe 1 WallThickness=inf",
            "Pipe 1 Thick=Yes",
            "Pipe 1 Thick=No WallThickness=0.2",
            "Pipe 1 Stations=",
            "Pipe 1 Stations=0:2",
            "Pipe 1 Stations=1:2",
            "Pipe 1 Stations=0.5:-2",
            "Pipe 1 Stations=0.5:2,0.25:3",
            "Pipe 1 Stations=0.5:2,0.5:3",
            "Pipe 1 Stations=0.5:2 Stations=0.75:3",
            "Pipe 1 Stations=0.5:2 ShapeBlending=Global",
            "Pipe 1 Stations=0.5:0.3 WallThickness=-0.5",
        ] {
            assert!(
                registry.execute(&mut document, command).is_err(),
                "{command}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), originals);
        }
    }
}
