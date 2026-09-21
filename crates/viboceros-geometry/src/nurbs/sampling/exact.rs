//! Rare fractional queries must not round a parameter before evaluating it.
use super::*;
use crate::nurbs::exact::{Direction, curve_controls, evaluate_at, rational, scalar, vector};
use num_traits::Zero;

#[cold]
pub(super) fn first(
    curve: &NurbsCurve,
    start: Real,
    end: Real,
    fraction: Real,
    span: Option<usize>,
) -> Result<(Point3, Vector3), GeometryError> {
    let origin = rational(start);
    let width = rational(end) - &origin;
    let parameter = origin + &width * rational(fraction);
    let span = span.unwrap_or_else(|| {
        if fraction == 1. {
            curve.find_span(end)
        } else {
            curve.knots.partition_point(|k| rational(*k) <= parameter) - 1
        }
    });
    let controls = curve_controls(curve, span);
    let h = evaluate_at(
        &curve.knots,
        curve.degree,
        span,
        &parameter,
        controls.clone(),
    )?;
    if h[3].is_zero() {
        return Err(GeometryError::ZeroWeightAtParameter);
    }
    let direction = Direction {
        knots: &curve.knots,
        degree: curve.degree,
        span,
        parameter: start, // Only coefficient differentiation uses this descriptor.
    };
    let first = evaluate_at(
        &curve.knots[1..curve.knots.len() - 1],
        curve.degree - 1,
        span - 1,
        &parameter,
        direction.derivative_controls(&controls, controls.len(), true)?,
    )?;
    let coordinates: [_; 3] = std::array::from_fn(|i| &h[i] / &h[3]);
    let point = Point3::try_new(
        scalar(&coordinates[0])?,
        scalar(&coordinates[1])?,
        scalar(&coordinates[2])?,
    )?;
    let derivative = vector(&std::array::from_fn(|i| {
        (&first[i] - &coordinates[i] * &first[3]) * &width / &h[3]
    }))?;
    Ok((point, derivative))
}

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
