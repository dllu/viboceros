//! Duplicate admission uses common positive bases, never sampled coincidence.
use super::*;
pub(super) fn curves(
    a: &NurbsCurve,
    b: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    if a.degree() != b.degree() || a.control_points().len() != b.control_points().len() {
        return Ok(false);
    }
    if basis(a, b, tolerance)? {
        return Ok(true);
    }
    // Rhino retains distinct seam/orientation encodings of polygon loops. Its
    // circular path instead coalesces equivalent full-circle encodings.
    let recognition = Tolerance::try_new(tolerance.absolute().min(1e-12), 1e-12, 1e-12)?;
    let circles = [a, b].map(|c| c.analytic_form(recognition));
    for c in circles {
        if !matches!(c?,Some(viboceros_geometry::CurveSegment3::Arc(arc))if arc.is_closed()) {
            return Ok(false);
        }
    }
    if basis(&a.reversed()?.try_reparameterized(0. ..=1.)?, b, tolerance)? {
        return Ok(true);
    }
    let target = b.evaluate(*b.domain().start())?;
    let bounds = b.control_point_bounds();
    let extent = bounds.min().distance_to(bounds.max())?;
    let epsilon = (64. * f64::EPSILON * extent).min(tolerance.absolute());
    let mut previous = None;
    for &t in a.knots().iter().filter(|&&t| a.domain().contains(&t)) {
        if previous == Some(t) {
            continue;
        }
        previous = Some(t);
        if a.evaluate(t)?.distance_to(target)? > epsilon {
            continue;
        }
        let shifted = a.try_change_closed_seam(t)?.try_reparameterized(0. ..=1.)?;
        if basis(&shifted, b, tolerance)?
            || basis(
                &shifted.reversed()?.try_reparameterized(0. ..=1.)?,
                b,
                tolerance,
            )?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn basis(a: &NurbsCurve, b: &NurbsCurve, tolerance: Tolerance) -> Result<bool, GeometryError> {
    if a.degree() != b.degree()
        || a.control_points().len() != b.control_points().len()
        || a.knots()[1..a.knots().len() - 1] != b.knots()[1..b.knots().len() - 1]
    {
        return Ok(false);
    }
    let gauges = [a, b].map(|c| {
        c.control_points()
            .iter()
            .map(|p| p.weight().abs())
            .fold(0_f64, f64::max)
            * c.control_points()[0].weight().signum()
    });
    let mut gap = 0_f64;
    let mut relative = 0_f64;
    for (x, y) in a.control_points().iter().zip(b.control_points()) {
        let wa = x.weight() / gauges[0];
        let wb = y.weight() / gauges[1];
        if wa <= 0. || wb <= 0. {
            return Ok(false);
        }
        gap = gap.max(x.point().distance_to(y.point())?);
        relative = relative.max((wa - wb).abs() / wa);
    }
    let bounds = b.control_point_bounds();
    let diameter = bounds.min().distance_to(bounds.max())?;
    // At the same parameter, positive rational coefficients are convex. The
    // control displacement plus diameter times relative weight perturbation
    // bounds the entire curve, including unsampled interiors.
    let bound = (gap + diameter * relative) * (1. + 32. * f64::EPSILON);
    Ok(bound.is_finite() && bound <= (64. * f64::EPSILON * diameter).min(tolerance.absolute()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loose_model_tolerance_does_not_merge_distinct_close_boundaries() {
        let a = NurbsCurve::try_new(
            1,
            [
                [0., 0., 0.],
                [4., 0., 0.],
                [4., 3., 0.],
                [0., 3., 0.],
                [0., 0., 0.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![0., 0., 0.25, 0.5, 0.75, 1., 1.],
        )
        .unwrap();
        let b = a
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(1e-8, 0., 0.).unwrap(),
            ))
            .unwrap();
        assert!(!curves(&a, &b, Tolerance::try_new(1e-3, 1e-12, 1e-6).unwrap()).unwrap());
        let shifted = a
            .try_change_closed_seam(0.25)
            .unwrap()
            .try_reparameterized(0. ..=1.)
            .unwrap();
        assert!(!curves(&a, &shifted, Tolerance::DEFAULT).unwrap());
        assert_eq!(a.control_points()[0].point().to_array(), [0., 0., 0.]);
    }
}
