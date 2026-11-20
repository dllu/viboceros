//! Rounded Bézier controls for an exact straight region.
use crate::exact_scalar::{Rational, rational, scalar};
use crate::{GeometryError, NurbsCurve, Point3, Real, WeightedPoint3};
use num_traits::Signed;

pub(crate) fn cubic_segment_controls(
    start: &[Rational; 3],
    end: &[Rational; 3],
    tolerance: Real,
) -> Result<Vec<WeightedPoint3>, GeometryError> {
    let delta = std::array::from_fn::<_, 3, _>(|i| &end[i] - &start[i]);
    let limit = rational(tolerance);
    let mut result = Vec::with_capacity(4);
    for index in 0..=3 {
        let fraction = rational(f64::from(index)) / rational(3.);
        let exact = std::array::from_fn::<_, 3, _>(|i| &start[i] + &fraction * &delta[i]);
        let [x, y, z] = exact.each_ref().map(scalar);
        let xyz = [x?, y?, z?];
        // L1 bounds Euclidean distance; convex combinations bound the complete
        // cubic against the ideal uniformly parameterized endpoint segment.
        let error = (0..3)
            .map(|i| (rational(xyz[i]) - &exact[i]).abs())
            .sum::<Rational>();
        if error > limit {
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "cubic handle rounding exceeds the requested tolerance",
            });
        }
        result.push(WeightedPoint3::try_new(Point3::try_from(xyz)?, 1.)?);
    }
    Ok(result)
}

pub(crate) fn cubic_polygon(
    vertices: &[[Rational; 3]],
    limit: Real,
) -> Result<NurbsCurve, GeometryError> {
    let mut controls = Vec::with_capacity(3 * (vertices.len() - 1) + 1);
    let mut end = 0.;
    let mut knots = vec![end; 4];
    for pair in vertices.windows(2) {
        let segment = cubic_segment_controls(&pair[0], &pair[1], limit)?;
        let next = end + segment[0].point().distance_to(segment[3].point())?;
        if !next.is_finite() || next <= end {
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "cubic polygon region lengths are not distinguishable",
            });
        }
        controls.extend(segment.into_iter().skip(usize::from(!controls.is_empty())));
        knots.extend([next; 3]);
        end = next;
    }
    knots.push(end);
    NurbsCurve::try_new_rational(3, controls, knots)
}
