//! Selective transverse refinement of a source support, preserving its image
//! before each independent match. Accuracy checks compare geometric contacts.
use super::*;
use crate::SurfaceJet2;
use jets::Vec3;
fn vector(v: crate::Vector3) -> Vec3 {
    Vec3::from_row_slice(&v.to_array())
}
fn normal(j: SurfaceJet2) -> Result<Vec3, GeometryError> {
    let n = vector(j.derivative_u).cross(&vector(j.derivative_v));
    let length = n.norm();
    if length == 0. || !length.is_finite() {
        return Err(error("singular refinement normal"));
    }
    Ok(n / length)
}
pub(super) fn run<E: From<GeometryError>>(
    source: &NurbsSurface,
    target: &NurbsSurface,
    reverse: bool,
    options: SurfaceMatchOptions,
    tolerance: Tolerance,
    operations: &mut SurfaceMatchOperations<'_, E>,
) -> Result<NurbsSurface, E> {
    if !options.refine {
        return fit::fit(
            source,
            target,
            reverse,
            options,
            tolerance,
            operations.interpolate,
        );
    }
    let edge = target.isocurve_v(*target.domain_u().start())?;
    let curved = !edge.is_linear(tolerance)?;
    let mut prepared = source.clone();
    let mut mids = if curved {
        prepared.spans_v().map(|(a, b)| a + (b - a) * 0.5).collect()
    } else {
        Vec::new()
    };
    for level in 0..4 {
        for mid in mids {
            prepared = (operations.refine)(&prepared, mid)?;
        }
        if level == 3 {
            return Ok(prepared);
        }
        let result = fit::fit(
            &prepared,
            target,
            reverse,
            options,
            tolerance,
            operations.interpolate,
        )?;
        mids = failing_midpoints(&result, target, options.continuity, tolerance)?;
        if mids.is_empty() {
            return Ok(result);
        }
        prepared = result;
    }
    Err(error("surface match refinement did not reach requested tolerances").into())
}

fn failing_midpoints(
    source: &NurbsSurface,
    target: &NurbsSurface,
    continuity: SurfaceBlendContinuity,
    tolerance: Tolerance,
) -> Result<Vec<Real>, GeometryError> {
    let target_curve = target.isocurve_v(*target.domain_u().start())?;
    let closest = target_curve.prepare_closest_parameter_search()?;
    let search_tolerance = Tolerance::try_new(
        tolerance.absolute().min(1e-10),
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let mut failed = Vec::new();
    'span: for (a, b) in source.spans_v() {
        for i in 0..=8 {
            let t = a + (b - a) * i as Real / 8.;
            let sample = source.evaluate_with_second_derivatives(*source.domain_u().end(), t)?;
            let v = closest.closest_parameter(sample.point, search_tolerance)?;
            let reference =
                target.evaluate_with_second_derivatives(*target.domain_u().start(), v)?;
            if sample.point.distance_to(reference.point)? > tolerance.absolute() {
                failed.push(a + (b - a) * 0.5);
                continue 'span;
            }
            if continuity == SurfaceBlendContinuity::Position {
                continue;
            }
            let n = normal(sample)?;
            let mut m = normal(reference)?;
            if n.dot(&m) < 0. {
                m = -m;
            }
            let angle = n.cross(&m).norm().atan2(n.dot(&m));
            if angle > tolerance.angular() {
                failed.push(a + (b - a) * 0.5);
                continue 'span;
            }
            if continuity == SurfaceBlendContinuity::Curvature {
                let first = vector(sample.derivative_u);
                let u = vector(reference.derivative_u);
                let v = vector(reference.derivative_v);
                let e = u.dot(&u);
                let f = u.dot(&v);
                let g = v.dot(&v);
                let det = e * g - f * f;
                if det <= 0. {
                    return Err(error("singular curvature refinement frame"));
                }
                let a = (g * u.dot(&first) - f * v.dot(&first)) / det;
                let b = (e * v.dot(&first) - f * u.dot(&first)) / det;
                let desired = m.dot(
                    &(vector(reference.derivative_uu) * a * a
                        + vector(reference.derivative_uv) * (2. * a * b)
                        + vector(reference.derivative_vv) * b * b),
                );
                let actual = n.dot(&vector(sample.derivative_uu));
                if (actual - desired).abs()
                    > 0.0005 * actual.abs().max(desired.abs()).max(tolerance.absolute())
                {
                    failed.push(a + (b - a) * 0.5);
                    continue 'span;
                }
            }
        }
    }
    Ok(failed)
}
