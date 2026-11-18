//! Shared geometry construction for scripted and interactive Pipe.
use super::*;

pub(super) enum BuiltPipe {
    Straight(Vec<Brep>),
    Sweep(Geometry),
}

impl BuiltPipe {
    pub(super) fn into_geometry(self) -> Vec<Geometry> {
        match self {
            Self::Straight(breps) => breps.into_iter().map(Geometry::Brep).collect(),
            Self::Sweep(geometry) => vec![geometry],
        }
    }
}

pub(super) fn build_walls(
    source: &Geometry,
    first: &Profile,
    second: Option<&Profile>,
    mut options: PipeOptions,
    use_extrusions: bool,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    let Some(second) = second else {
        return Ok(build(source, first, options, use_extrusions, tolerance)?.into_geometry());
    };
    let rail = source.curve_ref().ok_or(CommandError::Usage(USAGE))?;
    if rail.is_closed()? {
        options.cap = PipeCap::None;
    } else if options.cap == PipeCap::Round {
        options.cap = PipeCap::Flat;
    }
    let signed = second.endpoints[0] - first.endpoints[0];
    let same_thickness =
        |a: Real, b: Real| ((a + signed) - b).abs() <= 4. * Real::EPSILON * a.abs().max(b.abs());
    if same_thickness(first.endpoints[1], second.endpoints[1])
        && first
            .stations
            .iter()
            .zip(&second.stations)
            .all(|(a, b)| same_thickness(a.1, b.1))
    {
        let uniform = PipeOptions {
            wall_thickness: Some(signed),
            ..options
        };
        if options.cap != PipeCap::None {
            return Ok(build(source, first, uniform, use_extrusions, tolerance)?.into_geometry());
        }
        if let Some(outputs) = extrusion::straight_pipe(
            rail,
            first.endpoints,
            &first.stations,
            uniform,
            use_extrusions,
            tolerance,
        )? {
            return Ok(BuiltPipe::Straight(outputs).into_geometry());
        }
    }

    let make_wall = |profile: Profile| {
        let built = build(
            source,
            &profile,
            PipeOptions {
                cap: PipeCap::None,
                ..options
            },
            use_extrusions,
            tolerance,
        )?;
        match built {
            BuiltPipe::Sweep(Geometry::Brep(brep)) => Ok(brep),
            BuiltPipe::Sweep(Geometry::NurbsSurface(surface)) => {
                let [u, v] = surface.sampled_kink_parameters(tolerance.angular())?;
                let wall = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
                // A station sweep has the rail as U and the profile as V.
                Ok(if profile.stations.is_empty() {
                    wall
                } else {
                    wall.reversed()
                })
            }
            BuiltPipe::Straight(mut breps) => Ok(breps.pop().unwrap()),
            _ => Err(CommandError::Usage(USAGE)),
        }
    };
    let outer = make_wall(first.combine(second, Real::max))?;
    let inner = make_wall(first.combine(second, Real::min))?;
    if options.cap == PipeCap::None {
        return Ok(vec![
            Geometry::Brep(outer),
            Geometry::Brep(inner.reversed()),
        ]);
    }
    Ok(vec![Geometry::Brep(finish_two_walls(
        outer,
        inner,
        options.cap,
        tolerance,
    )?)])
}

pub(super) fn build(
    source: &Geometry,
    profile: &Profile,
    options: PipeOptions,
    use_extrusions: bool,
    tolerance: Tolerance,
) -> Result<BuiltPipe, CommandError> {
    let rail = source.curve_ref().ok_or(CommandError::Usage(USAGE))?;
    let [start_radius, end_radius] = profile.endpoints;
    let stations = &profile.stations;
    let PipeOptions {
        blend,
        cap,
        fit_rail,
        wall_thickness,
    } = options;
    let second_radii =
        wall_thickness.map(|thickness| [start_radius + thickness, end_radius + thickness]);
    if let Some(outputs) = extrusion::straight_pipe(
        rail,
        [start_radius, end_radius],
        stations,
        options,
        use_extrusions,
        tolerance,
    )? {
        return Ok(BuiltPipe::Straight(outputs));
    }
    let fitted_tapered_corner = fit_rail
        && stations.is_empty()
        && start_radius != end_radius
        && rail.to_nurbs()?.has_full_multiplicity_kink(tolerance)?;
    let result =
        match source {
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
                    stations,
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
                    stations,
                    options,
                    tolerance,
                )?)
            }
            source if !stations.is_empty() => station_pipe(
                source,
                rail,
                [start_radius, end_radius],
                stations,
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
    Ok(BuiltPipe::Sweep(result))
}
