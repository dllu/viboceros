//! Endpoint-tangent proposal shared by geometric cubic fitting.
use crate::{GeometryError, NurbsCurve, ParameterSide};
mod adjust;
pub(crate) use adjust::adjusted_cubic;
mod correspondence;
pub(crate) use correspondence::within_deviation;

/// Initial unit-domain polynomial cubic. Each handle has length
/// `chord_length * (2 - cos(chord, tangent)^2) / 3`. It interpolates endpoint
/// positions and limiting tangent directions before control rounding. No
/// positional or angular accuracy claim is made here:
/// callers must qualify the rounded candidate against the complete source.
pub(crate) fn initial_cubic(source: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    let domain = source.domain();
    let start = source.evaluate(*domain.start())?;
    let end = source.evaluate(*domain.end())?;
    let chord = start.vector_to(end)?;
    let length = chord.length()?;
    let axis = chord.normalized_nonzero()?.as_vector();
    let start_tangent = source
        .tangent_at_on_side(*domain.start(), ParameterSide::Right)?
        .as_vector();
    let end_tangent = source
        .tangent_at_on_side(*domain.end(), ParameterSide::Left)?
        .as_vector();
    let handle = |tangent: crate::Vector3| {
        let cosine = axis.dot(tangent)?.clamp(-1., 1.);
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
mod tests;
