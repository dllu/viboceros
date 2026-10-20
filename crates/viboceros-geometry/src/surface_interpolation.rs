//! UV interpolation and certified images, including covering-space seams.
use crate::{
    CurveInterpolationOptions, CurveKnotSpacing, GeometryError, InterpolatedCurveClosure,
    MAX_CURVE_INTERPOLATION_POINTS, NurbsCurve, NurbsCurve2, NurbsSurface, Plane, Point2, Point3,
    Real, Tolerance, Vector3, WeightedPoint2, WeightedPoint3,
    interpolation::try_interpolate_periodic_with_drift, intersection::curve_plane_root_parameters,
    remap_scalar, surface_geodesic::chart_closes,
};
const MAX_SEAMS: usize = 1024;
#[derive(Clone, Debug)]
pub struct SurfaceInterpolatedCurve {
    pub curve: NurbsCurve,
    /// A base-chart curve with independent one-sided UV values at seam knots.
    pub parameters: NurbsCurve2,
    pub surface_image_bound: Real,
    pub seam_count: usize,
    pub maximum_interpolation_error: Real,
    /// Temporary natural extension used for a certified image of an
    /// overshooting UV interpolant. The caller's surface is never edited.
    pub extended_support: Option<NurbsSurface>,
}
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceInterpolation { context }
}
fn axis_domain(s: &NurbsSurface, axis: usize) -> [Real; 2] {
    let d = if axis == 0 {
        s.domain_u()
    } else {
        s.domain_v()
    };
    [*d.start(), *d.end()]
}
fn native_point(surface: &NurbsSurface, q: [Real; 2]) -> Result<Point3, GeometryError> {
    Point3::try_new(
        remap_scalar(q[0], [0., 1.], axis_domain(surface, 0))?,
        remap_scalar(q[1], [0., 1.], axis_domain(surface, 1))?,
        0.,
    )
}
fn closed_delta(mut d: [Real; 2], closed: [bool; 2]) -> [Real; 2] {
    for axis in 0..2 {
        if closed[axis] {
            // Preserve the sign of an exact half-period tie.
            if d[axis] > 0.5 {
                d[axis] -= 1.;
            } else if d[axis] < -0.5 {
                d[axis] += 1.;
            }
        }
    }
    d
}
/// Interpolate native UV locations using chord-spaced cubic interpolation.
/// Exact periodic chart closures are lifted before interpolation and split
/// back into certified base-chart images afterwards. Face trims are not inputs.
pub fn try_interpolate_curve_on_surface(
    surface: &NurbsSurface,
    points: &[Point2],
    closed: bool,
    tolerance: Tolerance,
) -> Result<SurfaceInterpolatedCurve, GeometryError> {
    if points.len() < 2 || points.len() > MAX_CURVE_INTERPOLATION_POINTS {
        return Err(invalid("requires two through 256 interpolation locations"));
    }
    let periodic = [chart_closes(surface, 0)?, chart_closes(surface, 1)?];
    let canonical = points
        .iter()
        .map(|p| surface.normalized_parameters(p.x(), p.y()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut lifted = vec![canonical[0]];
    for i in 1..canonical.len() {
        let delta = closed_delta(
            std::array::from_fn(|j| canonical[i][j] - canonical[i - 1][j]),
            periodic,
        );
        let previous = *lifted.last().unwrap();
        lifted.push(std::array::from_fn(|j| previous[j] + delta[j]));
    }
    let same_end = canonical
        .first()
        .zip(canonical.last())
        .is_some_and(|(a, b)| {
            (0..2).all(|i| a[i] == b[i] || periodic[i] && (a[i] - b[i]).abs() == 1.)
        });
    if closed && same_end {
        lifted.pop();
    }
    if lifted.len() < 2 || closed && lifted.len() < 3 {
        return Err(invalid(
            "open interpolation needs two locations; closed interpolation needs three",
        ));
    }
    let native = lifted
        .iter()
        .map(|p| native_point(surface, *p))
        .collect::<Result<Vec<_>, _>>()?;
    let interpolant = if closed {
        let first = lifted[0];
        let last = *lifted.last().unwrap();
        let end_delta = closed_delta(
            std::array::from_fn(|j| first[j] - last[j].rem_euclid(1.)),
            periodic,
        );
        // Only periodic axes choose another copy. Open axes close to the
        // original parameter, including their exact high domain endpoint.
        let end = std::array::from_fn(|j| {
            if periodic[j] {
                last[j] + end_delta[j]
            } else {
                first[j]
            }
        });
        let endpoint = native_point(surface, end)?;
        let drift = native[0].vector_to(endpoint)?;
        let mut intervals = native
            .windows(2)
            .map(|p| p[0].distance_to(p[1]))
            .collect::<Result<Vec<_>, _>>()?;
        intervals.push(native.last().unwrap().distance_to(endpoint)?);
        try_interpolate_periodic_with_drift(&native, &intervals, drift)?
    } else {
        NurbsCurve::try_interpolate_for_command(
            &native,
            CurveInterpolationOptions::new(
                3,
                CurveKnotSpacing::Chord,
                InterpolatedCurveClosure::Open,
            ),
            Tolerance::NUMERICAL_VALIDATION,
        )?
    };
    let normalized = NurbsCurve::try_new_rational(
        interpolant.degree(),
        interpolant
            .control_points()
            .iter()
            .map(|p| {
                let q = p.point();
                WeightedPoint3::try_new(
                    Point3::try_new(
                        remap_scalar(q.x(), axis_domain(surface, 0), [0., 1.])?,
                        remap_scalar(q.y(), axis_domain(surface, 1), [0., 1.])?,
                        0.,
                    )?,
                    p.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        interpolant.knots().to_vec(),
    )?;
    // The native producer permits an interpolant to overshoot an open natural
    // boundary. Extend a private support with the shared exact control-curve
    // extension producer, and certify against that support. The conservative
    // control hull may request more support than the final locus actually uses.
    let mut support = surface.clone();
    let mut extended = false;
    for (axis, is_periodic) in periodic.into_iter().enumerate() {
        if is_periodic {
            continue;
        }
        let mut range = [0_f64, 1_f64];
        for p in normalized.control_points() {
            let value = p.point().to_array()[axis];
            range[0] = range[0].min(value);
            range[1] = range[1].max(value);
        }
        if range != [0., 1.] {
            let lo = remap_scalar(range[0], [0., 1.], axis_domain(surface, axis))?;
            let hi = remap_scalar(range[1], [0., 1.], axis_domain(surface, axis))?;
            support = if axis == 0 {
                support.try_extended_u(lo..=hi)?
            } else {
                support.try_extended_v(lo..=hi)?
            };
            extended = true;
        }
    }
    let mut cuts = vec![*normalized.domain().start(), *normalized.domain().end()];
    for (axis, is_periodic) in periodic.into_iter().enumerate() {
        if !is_periodic {
            continue;
        }
        let coords = normalized
            .control_points()
            .iter()
            .map(|p| p.point().to_array()[axis]);
        let (mut lo, mut hi) = (Real::INFINITY, Real::NEG_INFINITY);
        for c in coords {
            lo = lo.min(c);
            hi = hi.max(c);
        }
        if !lo.is_finite() || !hi.is_finite() || hi.ceil() - lo.floor() > MAX_SEAMS as Real {
            return Err(invalid("too many covering-space seams"));
        }
        for cell in lo.ceil() as i32..=hi.floor() as i32 {
            let mut origin = [0.; 3];
            origin[axis] = cell as Real;
            let mut normal = [0.; 3];
            normal[axis] = 1.;
            let (roots, _) = curve_plane_root_parameters(
                &normalized,
                Plane::new(
                    Point3::try_from(origin)?,
                    Vector3::try_from(normal)?.normalized_nonzero()?,
                ),
            )?;
            cuts.extend(roots);
        }
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup();
    if cuts.len() > MAX_SEAMS + 2 {
        return Err(invalid("too many chart portions"));
    }
    let numerical = Tolerance::try_new(
        tolerance.absolute() * 0.1,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let mut images = Vec::new();
    let mut parameters = Vec::new();
    for interval in cuts.windows(2) {
        if interval[0] >= interval[1] {
            continue;
        }
        let portion = normalized.try_trimmed(interval[0]..=interval[1])?;
        let middle = portion
            .evaluate(interval[0].midpoint(interval[1]))?
            .to_array();
        let shift = std::array::from_fn::<_, 2, _>(|axis| {
            if periodic[axis] {
                middle[axis].floor()
            } else {
                0.
            }
        });
        let mut controls = Vec::new();
        for p in portion.control_points() {
            let q = p.point().to_array();
            let mut uv = std::array::from_fn::<_, 2, _>(|axis| q[axis] - shift[axis]);
            // Root isolation and endpoint encoding can place a seam endpoint
            // a few ulps beyond a chart. Snap only roundoff-sized excursions;
            // every complete image still passes the rational proof below.
            for value in &mut uv {
                if (-256. * Real::EPSILON..0.).contains(value) {
                    *value = 0.;
                }
                if (1. ..=1. + 256. * Real::EPSILON).contains(value) {
                    *value = 1.;
                }
            }
            let point = native_point(surface, uv)?;
            controls.push(WeightedPoint2::try_new(
                Point2::try_new(point.x(), point.y())?,
                p.weight(),
            )?);
        }
        let uv =
            NurbsCurve2::try_new_rational(portion.degree(), controls, portion.knots().to_vec())?;
        let image = support.try_pushup_curve_certified_cubic(&uv, numerical)?;
        parameters.push(uv);
        images.push(image);
    }
    let curve = if images.len() == 1 {
        images.pop().unwrap()
    } else {
        // A chart portion can itself be a complete lap. PolyCurve correctly
        // rejects closed child segments, but the covering-space image is a
        // valid spline with repeated laps and independent one-sided joins.
        let degree = images.iter().map(NurbsCurve::degree).max().unwrap();
        let mut controls = Vec::new();
        let mut knots = Vec::new();
        for (i, image) in images.iter().enumerate() {
            let image = image.try_change_degree(degree, false)?;
            controls.extend_from_slice(image.control_points());
            knots.extend_from_slice(&image.knots()[if i == 0 { 0 } else { degree + 1 }..]);
        }
        NurbsCurve::try_new_rational(degree, controls, knots)?
    };
    let mut controls = Vec::new();
    let mut knots = Vec::new();
    let degree = normalized.degree();
    for (i, uv) in parameters.iter().enumerate() {
        controls.extend_from_slice(uv.control_points());
        knots.extend_from_slice(&uv.knots()[if i == 0 { 0 } else { degree + 1 }..]);
    }
    let uv = NurbsCurve2::try_new_rational(degree, controls, knots)?;
    let bound = support
        .parameter_curve_deviation_bound(&uv, &curve, tolerance.absolute() * 0.5)?
        .ok_or(invalid(
            "complete interpolation image could not be certified",
        ))?;
    let mut maximum_interpolation_error = 0_f64;
    let mut parameter = *curve.domain().start();
    for i in 0..native.len() {
        if i > 0 {
            parameter += native[i - 1].distance_to(native[i])?;
        }
        let expected = surface.evaluate(points[i].x(), points[i].y())?;
        maximum_interpolation_error =
            maximum_interpolation_error.max(expected.distance_to(curve.evaluate(parameter)?)?);
    }
    if maximum_interpolation_error > tolerance.absolute() {
        return Err(invalid(
            "surface interpolation locations exceeded model tolerance",
        ));
    }
    if closed && !curve.is_closed()? {
        return Err(invalid("closed interpolation image has an unresolved seam"));
    }
    Ok(SurfaceInterpolatedCurve {
        curve,
        parameters: uv,
        surface_image_bound: bound,
        seam_count: parameters.len() - 1,
        maximum_interpolation_error,
        extended_support: extended.then_some(support),
    })
}

#[cfg(test)]
mod tests;
