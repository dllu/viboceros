//! One common tangent-handle correction from an incremental length station.
use crate::{ArcLengthSampler, CurveRef, GeometryError, NurbsCurve, Real, Tolerance};

/// Propose a common positive handle scale from the source midpoint of an
/// eight-interval incremental quadrature batch. An already fitting midpoint
/// does not justify a correction. A caller must certify the complete output.
pub(crate) fn adjusted_cubic(
    source: &NurbsCurve,
    initial: &NurbsCurve,
    limit: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
    let sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(source), Tolerance::DEFAULT)?;
    let parameters = sampler
        .parameters_at_incremental_normalized_lengths(&fractions, super::FITTING_BATCH_TOLERANCE)?;
    let target = source.evaluate(parameters[4])?;
    adjusted_cubic_at_point(initial, target, limit)
}

pub(crate) fn adjusted_cubic_at_point(
    initial: &NurbsCurve,
    target: crate::Point3,
    limit: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let parameter = initial.closest_parameter(target, Tolerance::DEFAULT)?;
    let point = initial.evaluate(parameter)?;
    if point.distance_to(target)? <= limit {
        return Ok(None);
    }
    let controls = initial.control_points();
    let [start, first, second, end] = std::array::from_fn(|i| controls[i].point());
    let start_handle = start.vector_to(first)?;
    let end_handle = end.vector_to(second)?;
    let b1 = 3. * parameter * (1. - parameter).powi(2);
    let b2 = 3. * parameter.powi(2) * (1. - parameter);
    let a = start_handle.scaled(b1)?.to_array();
    let b = end_handle.scaled(b2)?.to_array();
    let influence = crate::Vector3::try_from(std::array::from_fn(|i| a[i] + b[i]))?;
    let length = influence.length()?;
    // Nearly cancelling influences amplify closest-point roundoff into a
    // spurious large scale. Decline the correction; callers can subdivide.
    let frame = start_handle.length()?.max(end_handle.length()?);
    if length <= frame * Real::EPSILON.sqrt() {
        return Ok(None);
    }
    // Normalize before the ratio so squaring a large/tiny handle cannot lose
    // a finite correction. The subsequent complete certificate sees rounding.
    let scale = 1.
        + influence
            .normalized_nonzero()?
            .as_vector()
            .dot(point.vector_to(target)?)?
            / length;
    if !scale.is_finite() || scale <= 0. {
        return Ok(None);
    }
    Ok(Some(NurbsCurve::try_new(
        3,
        vec![
            start,
            start.translated(start_handle.scaled(scale)?)?,
            end.translated(end_handle.scaled(scale)?)?,
            end,
        ],
        initial.knots().to_vec(),
    )?))
}

#[cfg(test)]
mod tests;
