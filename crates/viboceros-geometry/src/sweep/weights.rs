//! Sufficient, bounded denominator-positivity checks for fixed-basis sweeps.

use super::*;

const MAX_SUBDIVISIONS: usize = 4096;
const MAX_DEPTH: usize = 20;

/// Negative control weights need not imply a rational pole. For each V
/// control trajectory, bound its scalar U weight function by subdivision.
/// Positive trajectory weights imply a positive tensor denominator for all V.
/// This is sufficient, not necessary: unresolved cases are rejected, and the
/// original surface's degree, knots and geometry are never changed.
pub(super) fn require_positive_denominator(surface: &NurbsSurface) -> Result<(), GeometryError> {
    let mut subdivisions = 0;
    for controls in surface
        .control_points()
        .chunks_exact(surface.control_point_count_u())
    {
        if controls.iter().all(|c| c.weight() > 0.0) {
            continue;
        }
        let scale = controls
            .iter()
            .map(|c| c.weight().abs())
            .fold(0.0, Real::max);
        let scalar = NurbsCurve::try_new(
            surface.degree_u(),
            controls
                .iter()
                .map(|c| Point3::try_new(c.weight() / scale, 0.0, 0.0))
                .collect::<Result<Vec<_>, _>>()?,
            surface.knots_u().to_vec(),
        )?;
        let margin = 256.0 * Real::EPSILON * (surface.degree_u() + 1) as Real;
        let mut stack = vec![(scalar, 0)];
        while let Some((curve, depth)) = stack.pop() {
            if curve
                .control_points()
                .iter()
                .all(|c| c.point().x() > margin)
            {
                continue;
            }
            let domain = curve.domain();
            if depth == MAX_DEPTH
                || subdivisions == MAX_SUBDIVISIONS
                || curve.evaluate(*domain.start())?.x() <= margin
                || curve.evaluate(*domain.end())?.x() <= margin
            {
                return Err(invalid(
                    "positive sweep denominator could not be established",
                ));
            }
            let middle = 0.5 * domain.start() + 0.5 * domain.end();
            if !(middle > *domain.start() && middle < *domain.end()) {
                return Err(invalid("sweep denominator exhausted parameter resolution"));
            }
            let (left, right) = curve.try_split(middle)?;
            subdivisions += 1;
            stack.push((right, depth + 1));
            stack.push((left, depth + 1));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_denominator_is_distinguished_from_crossing_and_tangent_poles() {
        for (middle, positive) in [
            (-0.25, true),
            (-1.0 + 1e-12, true),
            (-1.0, false),
            (-2.0, false),
        ] {
            for scale in [1e-280, 1.0, 1e280] {
                let controls = (0..2)
                    .flat_map(|j| {
                        [1.0, middle, 1.0]
                            .into_iter()
                            .enumerate()
                            .map(move |(i, weight)| {
                                WeightedPoint3::try_new(
                                    Point3::try_new(i as Real, j as Real, 0.0).unwrap(),
                                    weight * scale,
                                )
                                .unwrap()
                            })
                    })
                    .collect();
                let surface = NurbsSurface::try_new_rational(
                    2,
                    1,
                    3,
                    2,
                    controls,
                    vec![0., 0., 0., 1., 1., 1.],
                    vec![0., 0., 1., 1.],
                )
                .unwrap();
                // W(u) = (1-u)^2 + 2*middle*u*(1-u) + u^2;
                // its minimum is exactly (1+middle)/2 at u=1/2.
                assert_eq!(require_positive_denominator(&surface).is_ok(), positive);
            }
        }
    }
}
