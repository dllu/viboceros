//! Invert the existing normal geodesic family with bounded shooting/refinement.
use super::*;
use crate::{Point3, Vector3};

const MAX_THROUGH_QUERIES: usize = 8192;
const MAX_SEEDS: usize = 129;

/// A regular normal-geodesic witness, in the original surface and source charts.
/// The multistart search is bounded; it does not certify the global cut locus.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceCurveOffsetThroughPoint {
    pub signed_distance: Real,
    pub source_parameter: Real,
    pub point: Point3,
    pub residual: Real,
    pub geodesic_queries: usize,
}

/// Find a signed surface distance whose normal offset passes through `point`.
/// The point must already lie on the natural surface; UI face constraints and
/// planar endpoint extension are command policy. Inputs remain immutable.
pub fn surface_curve_offset_distance_through_point(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    point: Point3,
    tolerance: Tolerance,
) -> Result<SurfaceCurveOffsetThroughPoint, GeometryError> {
    let (surface, source, numerical) =
        normalized_sources(original_surface, original_source, 0., tolerance)?;
    let uv = surface.closest_parameters(point, numerical)?;
    let constrained = surface.evaluate(uv.0, uv.1)?;
    if constrained.distance_to(point)? > tolerance.absolute() {
        return Err(invalid("through point does not lie on the surface"));
    }
    // A regular two-coordinate inverse cannot identify a normal branch at a
    // singular target chart (for example a sphere pole).
    surface.normal_at(uv.0, uv.1)?;
    let mut sampler = OffsetSampler::new(&surface, &source, 0., numerical)?;
    let closest = source.closest_parameter(constrained, numerical)?;
    let mut breaks = Vec::new();
    for i in 0..=8 {
        breaks.push(i as Real / 8.);
    }
    for (_, b) in source.spans().take(MAX_SEEDS + 1) {
        breaks.push(b);
    }
    // Start with the Euclidean closest foot; retain other chart seeds to find
    // competing normal branches rather than accepting the first local root.
    breaks.sort_by(Real::total_cmp);
    breaks.dedup();
    if breaks.len() > MAX_SEEDS {
        return Err(GeometryError::SurfaceCurveOffsetWorkLimit { maximum: MAX_SEEDS });
    }
    let mut seeds = vec![(closest, [0., 1.])];
    for span in breaks.windows(2) {
        seeds.push((stable_lerp(span[0], span[1], 0.5)?, [span[0], span[1]]));
    }
    let closed = source.evaluate(0.)? == source.evaluate(1.)?;
    let mut best: Option<(Real, Real, SurfaceGeodesicPoint)> = None;
    let mut ambiguous = false;
    for (t, interval) in seeds {
        if let Some((_, distance, _)) = best
            && interval != [0., 1.]
            && excludes_shorter_foot(&source, interval, constrained, distance.abs(), tolerance)
        {
            continue;
        }
        let start = source.evaluate(t)?;
        let uv = sampler.uv.evaluate(t)?;
        let tangent = CurveRef::NurbsCurve(&source)
            .evaluate_with_tangent(t)?
            .tangent();
        let normal = surface.normal_at(uv.x(), uv.y())?;
        let left = normal
            .as_vector()
            .cross(tangent.as_vector())?
            .normalized_nonzero()?;
        let delta = start.vector_to(constrained)?;
        let initial = delta.dot(left.as_vector())?;
        let chord = delta.length()?;
        let initial_distances = if initial.abs() < chord * 0.1 {
            vec![initial, chord, -chord]
        } else {
            vec![initial]
        };
        for d in initial_distances {
            if let Some(candidate) =
                refine(&mut sampler, constrained, t, d, closed, interval, tolerance)?
            {
                let (t, d, _) = candidate;
                if let Some((old_t, old_d, _)) = best {
                    if (d.abs() - old_d.abs()).abs() <= tolerance.absolute()
                        && source.evaluate(t)?.distance_to(source.evaluate(old_t)?)?
                            > tolerance.absolute()
                    {
                        ambiguous = true;
                    }
                    if d.abs() + tolerance.absolute() < old_d.abs() {
                        best = Some(candidate);
                        ambiguous = false;
                    }
                } else {
                    best = Some(candidate);
                }
            }
        }
    }
    if ambiguous {
        return Err(GeometryError::AmbiguousCurveOffsetSide);
    }
    let (t, d, shot) = best.ok_or(GeometryError::OffsetThroughPointNoSolution)?;
    if shot.point.distance_to(point)? > tolerance.absolute() {
        return Err(GeometryError::OffsetThroughPointNoSolution);
    }
    let domain = original_source.domain();
    let parameter = stable_lerp(*domain.start(), *domain.end(), t)?;
    if original_source
        .evaluate(parameter)?
        .distance_to(source.evaluate(t)?)?
        > numerical.absolute()
    {
        return Err(invalid(
            "through-point source parameter cannot be represented",
        ));
    }
    Ok(SurfaceCurveOffsetThroughPoint {
        signed_distance: d,
        source_parameter: parameter,
        point: shot.point,
        residual: shot.point.distance_to(point)?,
        geodesic_queries: sampler.diagnostics.queries,
    })
}

// An intrinsic path is at least as long as its Euclidean chord. Same-sign
// subcurve control hulls prune distant parameter cells once a witness exists.
// Failed bounds and mixed-sign weights retain the cell. The rounding margin
// deliberately disables pruning when world-coordinate precision is limiting.
fn excludes_shorter_foot(
    source: &NurbsCurve,
    interval: [Real; 2],
    target: Point3,
    radius: Real,
    tolerance: Tolerance,
) -> bool {
    let source_controls = source.control_points();
    if source_controls
        .iter()
        .any(|p| p.weight().is_sign_positive() != source_controls[0].weight().is_sign_positive())
    {
        return false;
    }
    let Ok(part) = source.try_trimmed(interval[0]..=interval[1]) else {
        return false;
    };
    let controls = part.control_points();
    if controls
        .iter()
        .any(|p| p.weight().is_sign_positive() != controls[0].weight().is_sign_positive())
    {
        return false;
    }
    let Ok(bounds) = crate::BoundingBox3::from_points(controls.iter().map(|p| p.point())) else {
        return false;
    };
    let lo = bounds.min().to_array();
    let hi = bounds.max().to_array();
    let p = target.to_array();
    let Ok(nearest) = Point3::try_from(std::array::from_fn(|i| p[i].clamp(lo[i], hi[i]))) else {
        return false;
    };
    let scale = source_controls
        .iter()
        .flat_map(|p| p.point().to_array())
        .map(Real::abs)
        .fold(0., Real::max);
    let padding =
        256. * (source.degree() + 1).pow(2) as Real * Real::EPSILON * scale + tolerance.absolute();
    nearest
        .distance_to(target)
        .is_ok_and(|d| d > radius + padding)
}

fn shoot(
    sampler: &mut OffsetSampler<'_>,
    t: Real,
    d: Real,
) -> Result<Option<SurfaceGeodesicPoint>, GeometryError> {
    if sampler.diagnostics.queries >= MAX_THROUGH_QUERIES {
        return Err(GeometryError::SurfaceCurveOffsetWorkLimit {
            maximum: MAX_THROUGH_QUERIES,
        });
    }
    sampler.distance = d;
    // A trial may reach a chart boundary or pole. It is not a shortened shot;
    // backtracking must find a complete valid shot before accepting the trial.
    Ok(sampler
        .sample(
            t,
            if t == 1. {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            },
        )
        .ok())
}
fn parameter(t: Real, closed: bool) -> Real {
    if closed {
        t.rem_euclid(1.)
    } else {
        t.clamp(0., 1.)
    }
}
fn derivative(
    sampler: &mut OffsetSampler<'_>,
    t: Real,
    d: Real,
    closed: bool,
    point: Point3,
) -> Result<Option<Vector3>, GeometryError> {
    let mut h = Real::EPSILON.cbrt();
    for _ in 0..12 {
        let a = if closed { t - h } else { (t - h).max(0.) };
        let b = if closed { t + h } else { (t + h).min(1.) };
        let left = shoot(sampler, parameter(a, closed), d)?;
        let right = shoot(sampler, parameter(b, closed), d)?;
        let pair = match (left, right) {
            (Some(left), Some(right)) => Some((left.point, right.point, b - a)),
            (Some(left), None) if t > a => Some((left.point, point, t - a)),
            (None, Some(right)) if b > t => Some((point, right.point, b - t)),
            _ => None,
        };
        if let Some((a, b, span)) = pair {
            return Ok(Some(a.vector_to(b)?.scaled(1. / span)?));
        }
        h *= 0.5;
    }
    Ok(None)
}

fn refine(
    sampler: &mut OffsetSampler<'_>,
    point: Point3,
    mut t: Real,
    mut d: Real,
    closed: bool,
    interval: [Real; 2],
    tolerance: Tolerance,
) -> Result<Option<(Real, Real, SurfaceGeodesicPoint)>, GeometryError> {
    let mut current = None;
    for _ in 0..12 {
        current = shoot(sampler, t, d)?;
        if current.is_some() {
            break;
        }
        d *= 0.5;
    }
    let Some(mut shot) = current else {
        return Ok(None);
    };
    for _ in 0..40 {
        let error = shot.point.distance_to(point)?;
        let Some(dt) = derivative(sampler, t, d, closed, shot.point)? else {
            return Ok(None);
        };
        let dd = shot
            .tangent
            .as_vector()
            .scaled(if d < 0. { -1. } else { 1. })?;
        let residual = shot.point.vector_to(point)?;
        let step = match crate::surface_geodesic::solve_tangent(dt, dd, residual) {
            Ok(step) => step,
            Err(_) => return Ok(None),
        };
        if error <= tolerance.absolute() * 0.05 {
            return Ok(Some((t, d, shot)));
        }
        let mut fraction = if step[0].abs() > 0.25 {
            0.25 / step[0].abs()
        } else {
            1.
        };
        let mut next = None;
        for _ in 0..24 {
            let trial = t + fraction * step[0];
            let nt = if closed && interval == [0., 1.] {
                parameter(trial, true)
            } else {
                trial.clamp(interval[0], interval[1])
            };
            let nd = d + fraction * step[1];
            if (nt == t && nd == d) || !nd.is_finite() {
                break;
            }
            if let Some(candidate) = shoot(sampler, nt, nd)?
                && candidate.point.distance_to(point)? < error
            {
                next = Some((nt, nd, candidate));
                break;
            }
            fraction *= 0.5;
        }
        let Some((nt, nd, candidate)) = next else {
            return Ok(None);
        };
        t = nt;
        d = nd;
        shot = candidate;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, LineSegment, UnitVector3};
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn tol() -> Tolerance {
        Tolerance::try_new(1e-7, 1e-12, 1e-8).unwrap()
    }
    fn frame() -> Frame3 {
        Frame3::try_from_unit_normal(
            p(0., 0., 0.),
            UnitVector3::try_new(0., 0., 1., tol()).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn planar_normal_inverse_preserves_sign_and_native_parameters() {
        for scale in [2f64.powi(-100), 1., 2f64.powi(100)] {
            let tolerance = Tolerance::try_new(scale * 1e-7, 1e-12, 1e-8).unwrap();
            let surface = NurbsSurface::try_bilinear([
                p(-10. * scale, -10. * scale, 0.),
                p(10. * scale, -10. * scale, 0.),
                p(10. * scale, 10. * scale, 0.),
                p(-10. * scale, 10. * scale, 0.),
            ])
            .unwrap();
            let curve =
                LineSegment::try_new(p(-4. * scale, 0., 0.), p(4. * scale, 0., 0.), tolerance)
                    .unwrap()
                    .to_nurbs()
                    .unwrap()
                    .try_reparameterized(-7. ..=11.)
                    .unwrap();
            for d in [-2., 0., 1.5] {
                let target = p(scale, d * scale, 0.);
                let result = surface_curve_offset_distance_through_point(
                    &surface, &curve, target, tolerance,
                )
                .unwrap();
                assert!((result.signed_distance / scale - d).abs() < 1e-7);
                assert!((result.source_parameter - 4.25).abs() < 1e-7);
                assert!(result.residual < tolerance.absolute());
            }
            assert!(
                surface_curve_offset_distance_through_point(
                    &surface,
                    &curve,
                    p(scale, scale, scale),
                    tolerance
                )
                .is_err()
            );
        }
    }
    #[test]
    fn cylindrical_inverse_uses_arc_distance_and_crosses_the_chart_seam() {
        let surface = NurbsSurface::try_cylinder(frame(), 3., 0., 6.).unwrap();
        let source = surface.isocurve_v(0.6).unwrap();
        let a = source.evaluate(source.parameter_at(0.5).unwrap()).unwrap();
        let angle = a.y().atan2(a.x());
        for end in [0., std::f64::consts::FRAC_PI_2] {
            let target = p(3. * end.cos(), 3. * end.sin(), 3.);
            let result =
                surface_curve_offset_distance_through_point(&surface, &source, target, tol())
                    .unwrap();
            // S_u x S_v points out; left of an upward generator is clockwise.
            assert!(
                (result.signed_distance + 3. * (end - angle)).abs() < 1e-7,
                "{result:?}"
            );
            assert!(result.residual < tol().absolute());
        }
    }
    #[test]
    fn spherical_inverse_finds_the_signed_meridian_distance() {
        let surface = NurbsSurface::try_sphere(frame(), 4.).unwrap();
        let source = surface.isocurve_u(0.2).unwrap();
        let p0 = source.evaluate(*source.domain().start()).unwrap();
        let latitude = p0.z().atan2(p0.x().hypot(p0.y()));
        let result =
            surface_curve_offset_distance_through_point(&surface, &source, p(4., 0., 0.), tol())
                .unwrap();
        assert!(
            (result.signed_distance + 4. * latitude).abs() < 1e-7,
            "{result:?}"
        );
        assert!(result.residual < tol().absolute());
        assert!(result.geodesic_queries < 512);
        assert!(
            surface_curve_offset_distance_through_point(&surface, &source, p(0., 0., 4.), tol())
                .is_err()
        );
    }
    #[test]
    fn planar_bezier_inverse_matches_an_independent_euclidean_normal_foot() {
        let surface = NurbsSurface::try_bilinear([
            p(-10., -10., 0.),
            p(10., -10., 0.),
            p(10., 10., 0.),
            p(-10., 10., 0.),
        ])
        .unwrap();
        let source = NurbsCurve::try_new(
            3,
            vec![
                p(-4., 0., 0.),
                p(-2., 3., 0.),
                p(2., -3., 0.),
                p(4., 0., 0.),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let target = p(0., 2., 0.);
        let closest = source.closest_parameter(target, tol()).unwrap();
        let expected = source
            .evaluate(closest)
            .unwrap()
            .distance_to(target)
            .unwrap();
        let result =
            surface_curve_offset_distance_through_point(&surface, &source, target, tol()).unwrap();
        assert!(
            (result.signed_distance - expected).abs() < 1e-7,
            "{result:?}"
        );
        assert!((result.source_parameter - closest).abs() < 1e-7);
        let reversed = source.reversed().unwrap();
        let other = surface_curve_offset_distance_through_point(&surface, &reversed, target, tol())
            .unwrap();
        assert!((result.signed_distance + other.signed_distance).abs() < 1e-7);
        assert!((result.source_parameter + other.source_parameter).abs() < 1e-7);
    }
    #[test]
    fn inverse_normalizes_both_surface_and_source_parameter_charts() {
        let scale = 2f64.powi(600);
        let surface = NurbsSurface::try_cylinder(frame(), 3., 0., 6.)
            .unwrap()
            .try_reparameterized(-scale..=scale, -1. / scale..=1. / scale)
            .unwrap();
        let u = surface.parameter_at_u(0.1).unwrap();
        let source = surface
            .isocurve_v(u)
            .unwrap()
            .try_reparameterized(-1. / scale..=1. / scale)
            .unwrap();
        let start = source.evaluate(0.).unwrap();
        let angle = start.y().atan2(start.x());
        let result =
            surface_curve_offset_distance_through_point(&surface, &source, p(3., 0., 3.), tol())
                .unwrap();
        assert!((result.signed_distance - 3. * angle).abs() < 1e-7);
        assert!(result.source_parameter.abs() < 1e-7 / scale);
    }
}
