//! Circular-profile pipe construction from a selected centerline curve.

use super::*;
mod extrusion;
use viboceros_geometry::{
    Circle3, CurveSegment3, Frame3, FrameTransportOptions, NurbsSurface, PolyCurve3, SurfaceIso,
    SweepBlend, SweepFrameStyle, SweepSection, WeightedPoint3, join_breps,
};

const USAGE: &str = "Pipe [curve-id] start-radius [end-radius] [Stations=fraction:radius,...] [Cap=None|Flat|Round] [ShapeBlending=Local|Global] [FitRail=Yes|No] [Thick=Yes|No] [WallThickness=signed-distance]";

pub(super) struct PipeCommand(pub use_extrusions::Preference);

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
        if let Some(outputs) = extrusion::straight_pipe(
            rail,
            [start_radius, end_radius],
            &stations,
            options,
            &self.0,
            tolerance,
        )? {
            let count = outputs.len();
            for brep in outputs {
                document.add_geometry(Geometry::Brep(brep))?;
            }
            return Ok(format!(
                "Added {count} {} pipe object(s)",
                if cap == PipeCap::Flat {
                    "closed"
                } else {
                    "open"
                }
            ));
        }
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
    let sweep = crate::sweep::model(
        rail,
        &sections,
        SweepFrameStyle::Freeform,
        SweepBlend::Local,
        tolerance,
    )?;
    let surface = crate::sweep::surface(&sweep, true)?;
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
        let sweep =
            crate::sweep::model(rail, &sections, SweepFrameStyle::Freeform, blend, tolerance)?;
        if matches!(source, Geometry::Arc(_)) {
            Ok(crate::sweep::fitted_model_surface(&sweep)?)
        } else {
            Ok(crate::sweep::surface(&sweep, true)?)
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
    let sweep = crate::sweep::model(rail, &sections, SweepFrameStyle::Freeform, blend, tolerance)?;
    // Rhino refits even a constant-radius cubic NURBS rail. Retaining its
    // four-control Bézier basis changes the measured flat Pipe volume.
    let surface = if radii[0] == radii[1] && !matches!(rail, CurveRef::NurbsCurve(_)) {
        crate::sweep::surface(&sweep, false)?
    } else if matches!(rail, CurveRef::Arc(_)) {
        // A direct continuous fit matches the measured Rhino arc Pipe volume
        // while avoiding a dense cubic rail refit.
        crate::sweep::fitted_model_surface(&sweep)?
    } else {
        crate::sweep::surface(&sweep, true)?
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
        surface_segments::trimmed_direction(&sphere, SurfaceKnotDirection::V, -half_pi..=cut)?
    } else {
        surface_segments::trimmed_direction(&sphere, SurfaceKnotDirection::V, cut..=half_pi)?
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
mod tests;
