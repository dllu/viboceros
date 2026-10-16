//! Two-rail construction through the native rail-constrained skin producer.
use super::*;
use viboceros_geometry::{AffineTransform3, NurbsCurve2, Point2};

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
    for t in [0., 1.] {
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
    if !(2..=32).contains(&sections.len())
        || first.control_points().len()
            + second.control_points().len()
            + sections
                .iter()
                .map(|c| c.control_points().len())
                .sum::<usize>()
            > 1024
    {
        return Err(unsupported(
            "requires 2–32 sections and at most 1024 total controls",
        ));
    }
    let certificate_first = first.clone();
    let mut certificate_second = second.clone();
    let first = prepare(first, tolerance)?;
    let mut second = prepare(second, tolerance)?;
    if first.degree().max(second.degree()) >= sections.len() {
        return Err(unsupported(
            "requires at least maximum rail degree plus one sections",
        ));
    }
    let origin = first.evaluate(0.)?;
    let zero = Point3::try_new(0., 0., 0.)?;
    let to_local = AffineTransform3::from_translation(origin.vector_to(zero)?);
    let local_rails = [first.transformed(to_local)?, second.transformed(to_local)?];
    let mut rows = Vec::with_capacity(sections.len());
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
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    if rows[0].0 != 0. || rows.last().unwrap().0 != 1. {
        return Err(unsupported(
            "first and last sections must contact the rail ends",
        ));
    }
    if rows[0].1 == 1. && rows.last().unwrap().1 == 0. {
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
    let surface = brep::surface_from_native(pointer)?
        .transformed(AffineTransform3::from_translation(zero.vector_to(origin)?))?;
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
