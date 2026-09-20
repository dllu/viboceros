//! Rare fractional queries must not round a parameter before evaluating it.
use super::*;
use crate::nurbs::exact::{curve_controls, evaluate_at, rational, scalar};
use num_traits::Zero;

#[cold]
pub(super) fn point(
    curve: &NurbsCurve,
    start: Real,
    end: Real,
    fraction: Real,
    span: Option<usize>,
) -> Result<Point3, GeometryError> {
    let start = rational(start);
    let parameter = &start + (rational(end) - &start) * rational(fraction);
    // Whole-domain interior queries use the right side at exact knots. A
    // span query supplies its own side, without converting back to binary64.
    let span =
        span.unwrap_or_else(|| curve.knots.partition_point(|k| rational(*k) <= parameter) - 1);
    let h = evaluate_at(
        &curve.knots,
        curve.degree,
        span,
        &parameter,
        curve_controls(curve, span),
    )?;
    if h[3].is_zero() {
        return Err(GeometryError::ZeroWeightAtParameter);
    }
    Point3::try_new(
        scalar(&(&h[0] / &h[3]))?,
        scalar(&(&h[1] / &h[3]))?,
        scalar(&(&h[2] / &h[3]))?,
    )
}
