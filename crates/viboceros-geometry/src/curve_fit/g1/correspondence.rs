//! Numerical correspondence proposals accepted only by complete image bounds.
use crate::{GeometryError, NurbsCurve, Real, Tolerance};

const MAX_INTERVALS: usize = 256;

pub(crate) fn within_deviation(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    limit: Real,
) -> Result<bool, GeometryError> {
    if source
        .parameter_curve_deviation_bound(candidate, limit)?
        .is_some()
    {
        return Ok(true);
    }
    let sampler = source.parameter_sampler()?;
    let search = candidate.prepare_closest_parameter_search()?;
    let numerical = Tolerance::try_new((limit * 1e-6).max(Real::from_bits(1)), 1e-12, 1e-10)?;
    let mut parameters = vec![None; MAX_INTERVALS + 1];
    parameters[0] = Some(0.);
    parameters[MAX_INTERVALS] = Some(1.);
    for intervals in [8, 16, 32, 64, 128, MAX_INTERVALS] {
        let mut pairs = Vec::with_capacity(intervals + 1);
        for index in (0..=MAX_INTERVALS).step_by(MAX_INTERVALS / intervals) {
            let fraction = index as Real / MAX_INTERVALS as Real;
            let parameter = if let Some(parameter) = parameters[index] {
                parameter
            } else {
                let point = sampler.evaluate(fraction)?;
                let parameter = search.closest_parameter(point, numerical)?;
                // This only declines a numerical proposal. Accepted fits still
                // require a complete exact certificate across every interval.
                if point.distance_to(candidate.evaluate(parameter)?)? > limit {
                    return Ok(false);
                }
                parameters[index] = Some(parameter);
                parameter
            };
            if pairs.last().is_some_and(|p: &[Real; 2]| p[1] >= parameter) {
                return Ok(false);
            }
            pairs.push([fraction, parameter]);
        }
        if source
            .mapped_curve_deviation_bound(candidate, &pairs, limit)?
            .is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
