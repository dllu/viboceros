//! Endpoint-tangent proposal shared by geometric cubic fitting.
use crate::{GeometryError, NurbsCurve, ParameterSide};
mod adjust;
pub(crate) use adjust::{adjusted_cubic, adjusted_cubic_at_point};
mod correspondence;
// Public batch request corresponding to the measured 1e-4 fitting clock.
pub(crate) const FITTING_BATCH_TOLERANCE: crate::Real = 0.01;
pub(crate) use correspondence::reconstruction_deviation_bound;
#[cfg(test)]
pub(crate) use correspondence::within_deviation_at_pairs;
pub(crate) use correspondence::{deviation_bound, deviation_bound_at_pairs, within_deviation};

/// Initial unit-domain polynomial cubic. Each handle has length
/// `chord_length * (2 - max(0, cos(chord, tangent))^2) / 3`. It interpolates endpoint
/// positions and limiting tangent directions before control rounding. No
/// positional or angular accuracy claim is made here:
/// callers must qualify the rounded candidate against the complete source.
pub(crate) fn initial_cubic(source: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    let domain = source.domain();
    let start = source.evaluate(*domain.start())?;
    let end = source.evaluate(*domain.end())?;
    let start_tangent = source.tangent_at_on_side(*domain.start(), ParameterSide::Right)?;
    let end_tangent = source.tangent_at_on_side(*domain.end(), ParameterSide::Left)?;
    if opposed_endpoint_tangents(start_tangent, end_tangent) {
        return Err(GeometryError::UnsupportedCurveApproximation {
            context: "initial cubic fitting requires endpoint tangents outside the opposition cone",
        });
    }
    initial_cubic_from_frames(start, end, start_tangent, end_tangent)
}

/// Fitting declines a near reversal before admitting a whole-span proposal.
/// The measured cosine boundary is independent of the region angle.
pub(crate) fn opposed_endpoint_tangents(
    start: crate::UnitVector3,
    end: crate::UnitVector3,
) -> bool {
    let a = start.as_vector().to_array();
    let b = end.as_vector().to_array();
    // Preserve ordered floating products for this discontinuous proposal
    // selection rule. Compensation changes the measured boundary branch.
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] < -0.9999
}

/// Keep the original source's tangent frame at an adaptive interior endpoint.
pub(crate) fn initial_cubic_from_frames(
    start: crate::Point3,
    end: crate::Point3,
    start_tangent: crate::UnitVector3,
    end_tangent: crate::UnitVector3,
) -> Result<NurbsCurve, GeometryError> {
    let chord = start.vector_to(end)?;
    let length = chord.length()?;
    let axis = chord.normalized_nonzero()?.as_vector();
    let start_tangent = start_tangent.as_vector();
    let end_tangent = end_tangent.as_vector();
    let handle = |tangent: crate::Vector3| {
        let cosine = axis.dot(tangent)?.clamp(0., 1.);
        tangent.scaled(length * ((2. - cosine * cosine) / 3.))
    };
    NurbsCurve::try_new(
        3,
        vec![
            start,
            start.translated(handle(start_tangent)?)?,
            end.translated(handle(end_tangent)?.scaled(-1.)?)?,
            end,
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
}

#[cfg(test)]
mod sweep_partitions;
#[cfg(test)]
mod tests;
