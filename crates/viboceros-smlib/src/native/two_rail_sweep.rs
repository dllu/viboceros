//! Two-rail construction through the native rail-constrained skin producer.
use super::*;
use viboceros_geometry::{AffineTransform3, NurbsCurve2, Point2, SweepEndPoints};

unsafe extern "C" {
    fn vb_curves_two_rail_sweep(
        first: *const NativeCurve,
        second: *const NativeCurve,
        sections: *const *const NativeCurve,
        count: usize,
        parameters: *const f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

fn unsupported(message: &str) -> Error {
    Error::Kernel(format!("Two-rail sweep: {message}"))
}

fn prepare(curve: &NurbsCurve, tolerance: Tolerance) -> Result<NurbsCurve, Error> {
    if curve.degree() > 16 || curve.control_points().len() > 128 || curve.is_rational() {
        return Err(unsupported(
            "requires polynomial curves of degree at most 16 with at most 128 controls",
        ));
    }
    if curve
        .evaluate(*curve.domain().start())?
        .distance_to(curve.evaluate(*curve.domain().end())?)?
        <= tolerance.absolute()
        || curve.knots()[curve.degree() + 1..curve.control_points().len()]
            .windows(curve.degree() + 1)
            .any(|k| k.iter().all(|v| *v == k[0]))
    {
        return Err(unsupported(
            "requires open, continuous, noncollapsed curves",
        ));
    }
    Ok(NurbsCurve::try_new(
        curve.degree(),
        curve.control_points().iter().map(|p| p.point()).collect(),
        curve.knots().to_vec(),
    )?
    .try_reparameterized(0. ..=1.)?)
}

fn contact(rail: &NurbsCurve, point: Point3, tolerance: Tolerance) -> Result<Option<f64>, Error> {
    let domain = rail.domain();
    for t in [*domain.start(), *domain.end()] {
        if rail.evaluate(t)?.distance_to(point)? <= tolerance.absolute() {
            return Ok(Some(t));
        }
    }
    let t = rail.closest_parameter(point, tolerance)?;
    Ok((rail.evaluate(t)?.distance_to(point)? <= tolerance.absolute()).then_some(t))
}

/// Construct an open two-rail sweep from 2–32 open polynomial sections.
/// Section endpoints must contact both rails at matching normalized parameters;
/// first/last sections must span the complete rails. Directions and section
/// order are inferred. Every returned rail and section image has a continuous
/// deviation certificate within the requested absolute tolerance.
/// This does not certify injectivity or general Rhino interior-shape parity.
pub fn two_rail_sweep_surface(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    two_rail_sweep_surface_with_point_ends(
        first,
        second,
        sections,
        SweepEndPoints::default(),
        tolerance,
    )
}

/// Add exact collapsed sections where both rails reach the selected points.
/// Interior point contacts trim private copies. The same continuous
/// certificates compare the output against the original retained intervals.
pub fn two_rail_sweep_surface_with_point_ends(
    first: &NurbsCurve,
    second: &NurbsCurve,
    sections: &[NurbsCurve],
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let point_count = usize::from(ends.start.is_some()) + usize::from(ends.end.is_some());
    let row_count = sections.len().saturating_add(point_count);
    if sections.is_empty()
        || !(2..=32).contains(&row_count)
        || first.control_points().len()
            + second.control_points().len()
            + sections
                .iter()
                .map(|c| c.control_points().len())
                .sum::<usize>()
            + 2 * point_count
            > 1024
    {
        return Err(unsupported(
            "requires a real profile, 2–32 total sections including point ends, and at most 1024 total controls",
        ));
    }
    let (first, certificate_first) = prepare_rail(first, sections, ends, tolerance)?;
    let (mut second, mut certificate_second) = prepare_rail(second, sections, ends, tolerance)?;
    if first.degree().max(second.degree()) >= row_count {
        return Err(unsupported(
            "requires at least maximum rail degree plus one sections",
        ));
    }
    let origin = first.evaluate(0.)?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let to_local = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let local_rails = [first.transformed(to_local)?, second.transformed(to_local)?];
    let mut rows = Vec::with_capacity(row_count);
    for section in sections {
        let mut curve = prepare(section, tolerance)?;
        let local = curve.transformed(to_local)?;
        let endpoints = [local.evaluate(0.)?, local.evaluate(1.)?];
        let mut matches = Vec::new();
        for reverse in [false, true] {
            if let (Some(a), Some(b)) = (
                contact(&local_rails[0], endpoints[usize::from(reverse)], tolerance)?,
                contact(&local_rails[1], endpoints[usize::from(!reverse)], tolerance)?,
            ) {
                matches.push((a, b, reverse));
            }
        }
        let [(a, b, reverse)] = matches.as_slice() else {
            return Err(unsupported(
                "section endpoints must unambiguously contact the two rails",
            ));
        };
        let original = if *reverse {
            curve = curve.reversed()?;
            section.reversed()?
        } else {
            section.clone()
        };
        rows.push((*a, *b, curve, original));
    }
    for (parameter, point) in [(0., ends.start), (1., ends.end)] {
        if let Some(point) = point {
            let curve = NurbsCurve::try_new(1, vec![point; 2], vec![0., 0., 1., 1.])?;
            rows.push((parameter, parameter, curve.clone(), curve));
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    if rows[0].0 != 0. || rows.last().unwrap().0 != 1. {
        return Err(unsupported(
            "first and last sections must contact the rail ends",
        ));
    }
    if ends.is_empty() && rows[0].1 == 1. && rows.last().unwrap().1 == 0. {
        second = second.reversed()?;
        certificate_second = certificate_second.reversed()?;
        for row in &mut rows {
            row.1 = 1. - row.1;
        }
    }
    if rows.iter().any(|r| (r.0 - r.1).abs() > 1e-10)
        || rows.windows(2).any(|r| r[1].0 - r[0].0 <= 1e-10)
    {
        return Err(unsupported(
            "requires distinct sections at matching normalized rail parameters",
        ));
    }
    let world = std::iter::once(&first)
        .chain(std::iter::once(&second))
        .chain(rows.iter().map(|r| &r.2))
        .collect::<Vec<_>>();
    let native = world
        .iter()
        .map(|c| KernelCurve::from_nurbs_for_modeling(&c.transformed(to_local)?))
        .collect::<Result<Vec<_>, Error>>()?;
    let handles = native[2..]
        .iter()
        .map(|c| c.handle.as_ptr().cast_const())
        .collect::<Vec<_>>();
    let parameters = rows.iter().map(|r| r.0).collect::<Vec<_>>();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curves_two_rail_sweep(
            native[0].handle.as_ptr(),
            native[1].handle.as_ptr(),
            handles.as_ptr(),
            handles.len(),
            parameters.as_ptr(),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let mut surface = brep::surface_from_native(pointer)?
        .transformed(AffineTransform3::from_translation(zero.vector_to(origin)?))?;
    if !ends.is_empty() && surface.degree_u() < 3 {
        surface = super::change_surface_degree(&surface, 3, surface.degree_v(), false)?;
    }
    if !ends.is_empty() {
        surface = exact_pole_rows(surface, ends, tolerance)?;
    }
    let u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let v = [*surface.domain_v().start(), *surface.domain_v().end()];
    let mut constraints = vec![
        (&certificate_first, [[u[0], v[0]], [u[1], v[0]]]),
        (&certificate_second, [[u[0], v[1]], [u[1], v[1]]]),
    ];
    for (t, _, _, curve) in &rows {
        let s = u[0] + t * (u[1] - u[0]);
        constraints.push((curve, [[s, v[0]], [s, v[1]]]));
    }
    for (curve, endpoints) in constraints {
        let domain = curve.domain();
        let (a, b) = (*domain.start(), *domain.end());
        let uv = NurbsCurve2::try_new(
            1,
            vec![
                Point2::try_from(endpoints[0])?,
                Point2::try_from(endpoints[1])?,
            ],
            vec![a, a, b, b],
        )?;
        if surface
            .parameter_curve_deviation_bound(&uv, curve, tolerance.absolute())?
            .is_none()
        {
            return Err(unsupported(
                "native output could not certify a rail or section within tolerance",
            ));
        }
    }
    Ok(surface)
}

fn exact_pole_rows(
    surface: NurbsSurface,
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<NurbsSurface, Error> {
    let u_count = surface.control_point_count_u();
    let mut controls = surface.control_points().to_vec();
    for row in controls.chunks_exact_mut(u_count) {
        for (i, pole) in [(0, ends.start), (u_count - 1, ends.end)] {
            if let Some(point) = pole {
                if row[i].point().distance_to(point)? > tolerance.absolute() {
                    return Err(unsupported("native output did not preserve the point end"));
                }
                // Restoring the world transform can round the point. Make the
                // whole row identical before certifying the surface and its rails.
                row[i] = WeightedPoint3::try_new(point, row[i].weight())?;
            }
        }
    }
    Ok(NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        u_count,
        surface.control_point_count_v(),
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )?)
}

/// Locate the supplied point ends on one rail, in its original parameter chart.
pub fn two_rail_point_stations(
    curve: &NurbsCurve,
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<[Option<f64>; 2], Error> {
    let locate = |point: Option<Point3>| -> Result<_, Error> {
        point
            .map(|point| {
                contact(curve, point, tolerance)?
                    .ok_or_else(|| unsupported("point ends must lie on both rails"))
            })
            .transpose()
    };
    let stations = [locate(ends.start)?, locate(ends.end)?];
    if let [Some(start), Some(end)] = stations
        && start == end
    {
        return Err(unsupported("point ends must occupy distinct stations"));
    }
    Ok(stations)
}

fn rail_interval(
    curve: &NurbsCurve,
    sections: &[NurbsCurve],
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<[f64; 2], Error> {
    let [start, end] = two_rail_point_stations(curve, ends, tolerance)?;
    let domain = curve.domain();
    let endpoints = [*domain.start(), *domain.end()];
    let opposite = |station| -> Result<f64, Error> {
        if station == endpoints[0] {
            return Ok(endpoints[1]);
        }
        if station == endpoints[1] {
            return Ok(endpoints[0]);
        }
        // With an interior pole, the remaining real terminal section chooses
        // which side of the crossing to keep. Reject picks that straddle it.
        let world = [curve.evaluate(endpoints[0])?, curve.evaluate(endpoints[1])?];
        let mut contacted = [false; 2];
        for section in sections {
            let section_domain = section.domain();
            for t in [*section_domain.start(), *section_domain.end()] {
                let point = section.evaluate(t)?;
                for i in 0..2 {
                    contacted[i] |= world[i].distance_to(point)? <= tolerance.absolute();
                }
            }
        }
        match contacted {
            [true, false] => Ok(endpoints[0]),
            [false, true] => Ok(endpoints[1]),
            _ => Err(unsupported(
                "an interior point needs a cross section at the remaining rail end",
            )),
        }
    };
    match (start, end) {
        (Some(start), Some(end)) => Ok([start, end]),
        (Some(start), None) => Ok([start, opposite(start)?]),
        (None, Some(end)) => Ok([opposite(end)?, end]),
        (None, None) => Ok(endpoints),
    }
}

fn prepare_rail(
    curve: &NurbsCurve,
    sections: &[NurbsCurve],
    ends: SweepEndPoints,
    tolerance: Tolerance,
) -> Result<(NurbsCurve, NurbsCurve), Error> {
    if ends.is_empty() {
        return Ok((prepare(curve, tolerance)?, curve.clone()));
    }
    let [start, end] = rail_interval(curve, sections, ends, tolerance)?;
    let range = start.min(end)..=start.max(end);
    let mut original = if range == curve.domain() {
        curve.clone()
    } else {
        curve.try_trimmed(range)?
    };
    if end < start {
        original = original.reversed()?;
    }
    let prepared = super::knot_refinement::clamp_curve(&prepare(&original, tolerance)?)?;
    let mut points = prepared
        .control_points()
        .iter()
        .map(|control| control.point())
        .collect::<Vec<_>>();
    if let Some(point) = ends.start {
        points[0] = point;
    }
    if let Some(point) = ends.end {
        *points.last_mut().unwrap() = point;
    }
    Ok((
        NurbsCurve::try_new(prepared.degree(), points, prepared.knots().to_vec())?,
        original,
    ))
}
