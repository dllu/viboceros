//! Circular-profile pipe construction from a selected centerline curve.

use super::*;
use viboceros_geometry::{
    Circle3, Frame3, FrameTransportOptions, NurbsSurface, SurfaceIso, Sweep1, SweepBlend,
    SweepFrameStyle, SweepSection, WeightedPoint3, join_breps,
};

const USAGE: &str = "Pipe [curve-id] start-radius [end-radius] [Stations=fraction:radius,...] [Cap=None|Flat|Round] [ShapeBlending=Local|Global] [Thick=Yes|No] [WallThickness=signed-distance]";

pub(super) struct PipeCommand;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipeCap {
    None,
    Flat,
    Round,
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
        if !document.is_object_selectable(source_id) {
            return Err(CommandError::Usage(USAGE));
        }
        let source = document
            .object(source_id)
            .map(|object| object.geometry())
            .ok_or(CommandError::Usage(USAGE))?;
        let rail = source.curve_ref().ok_or(CommandError::Usage(USAGE))?;
        let tolerance = document.tolerance();
        let result = match source {
            source if !stations.is_empty() => station_pipe(
                source,
                rail,
                [start_radius, end_radius],
                &stations,
                blend,
                cap,
                wall_thickness,
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

fn station_pipe(
    source: &Geometry,
    rail: CurveRef<'_>,
    endpoint_radii: [Real; 2],
    stations: &[(Real, Real)],
    blend: SweepBlend,
    cap: PipeCap,
    wall_thickness: Option<Real>,
    tolerance: Tolerance,
) -> Result<Geometry, CommandError> {
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
        if matches!(source, Geometry::Line(_)) {
            return straight_station_surface(frames[0], profile);
        }
        let sections = profile
            .iter()
            .zip(&frames)
            .map(|(&(parameter, radius), &frame)| {
                circular_section(parameter, frame, radius, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sweep = Sweep1::try_new(rail, &sections, SweepFrameStyle::Freeform, blend, tolerance)?;
        if matches!(source, Geometry::Line(_) | Geometry::Arc(_)) {
            Ok(sweep.fit_model_surface()?)
        } else {
            Ok(sweep.to_surface()?)
        }
    };
    let make_wall = |profile: &[(Real, Real)]| -> Result<Brep, CommandError> {
        let surface = make_surface(profile)?;
        let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        let wall = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
        Ok(if matches!(source, Geometry::Line(_)) {
            wall
        } else {
            wall.reversed()
        })
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

fn straight_station_surface(
    frame: Frame3,
    profile: &[(Real, Real)],
) -> Result<NurbsSurface, CommandError> {
    let start = profile[0].0;
    let height = profile.last().expect("endpoint profile").0 - start;
    let unit_cylinder = NurbsSurface::try_cylinder(frame, 1.0, 0.0, height)?;
    let circle_controls = &unit_cylinder.control_points()[..unit_cylinder.control_point_count_u()];
    let mut rows = Vec::with_capacity(3 * profile.len() - 2);
    for (index, pair) in profile.windows(2).enumerate() {
        let [(a, ra), (b, rb)] = [pair[0], pair[1]];
        for (j, (t, radius)) in [
            (a, ra),
            (a + (b - a) / 3.0, ra),
            (a + 2.0 * (b - a) / 3.0, rb),
            (b, rb),
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 && j == 0 {
                continue;
            }
            rows.push((t - start, radius));
        }
    }
    let mut controls = Vec::with_capacity(rows.len() * circle_controls.len());
    for (distance, radius) in rows {
        let offset = frame.z_axis().as_vector().scaled(distance)?;
        for control in circle_controls {
            let radial = frame.origin().vector_to(control.point())?;
            let point = frame
                .origin()
                .translated(radial.scaled(radius)?)?
                .translated(offset)?;
            controls.push(WeightedPoint3::try_new(point, control.weight())?);
        }
    }
    let mut knots = vec![0.0; 4];
    for &(t, _) in &profile[1..profile.len() - 1] {
        knots.extend([t - start; 3]);
    }
    knots.extend([height; 4]);
    Ok(NurbsSurface::try_new_rational(
        unit_cylinder.degree_u(),
        3,
        circle_controls.len(),
        3 * profile.len() - 2,
        controls,
        unit_cylinder.knots_u().to_vec(),
        knots,
    )?)
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
    let surface = if radii[0] == radii[1] {
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
    use viboceros_geometry::{CircularArc3, LineSegment, UnitVector3};

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
            let point = surface.evaluate(*surface.domain_u().start(), z).unwrap();
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
            let point = surface.evaluate(*surface.domain_u().start(), z).unwrap();
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
