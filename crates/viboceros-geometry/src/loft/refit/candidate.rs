//! Admit an initial or corrected G1 proposal after a continuous source proof.
use super::*;

#[cfg(test)]
pub(super) fn prepare(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare_with_policy(curves, tolerance, angle, adaptive::FitPolicy::LOFT)
}

pub(super) fn prepare_with_policy(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
    policy: adaptive::FitPolicy,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let limit = policy.sample_limit(tolerance);
    let unsupported = || GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires certified straight regions or certified smooth cubics; adaptive subdivision remains unfinished",
    };
    let mut sections = Vec::with_capacity(curves.len());
    for source in curves {
        let degree = source.degree();
        let single_span = source.control_points().len() == degree + 1;
        let domain = source.domain();
        let knots = source.knots();
        // A proposal may cover several spans in one resolved tangent region.
        // Positional jumps and resolved tangent breaks still need
        // separate fits; the complete certificate remains authoritative.
        if degree > 16
            || !knots[..=degree].iter().all(|k| k == domain.start())
            || !knots[source.control_points().len()..]
                .iter()
                .all(|k| k == domain.end())
            || (!single_span
                && (knots
                    .chunk_by(|a, b| a == b)
                    .any(|g| g[0] > *domain.start() && g[0] < *domain.end() && g.len() > degree)
                    || super::super::parameterize::has_tangent_breaks(
                        source,
                        corner_angle(angle),
                    )?))
        {
            return Err(unsupported());
        }
        let candidate = crate::curve_fit::g1::initial_cubic(source).map_err(|_| unsupported())?;
        let chord = candidate.control_points()[0]
            .point()
            .distance_to(candidate.control_points()[3].point())?;
        // Relative errors >=1 enter a separate native resampling branch. Its
        // basis/domain policy has not been established by this initial fit.
        if tolerance >= chord {
            return Err(unsupported());
        }
        let candidate = if crate::curve_fit::g1::within_deviation(source, &candidate, limit)?
            || (!single_span
                && quarter_stations_fit(source, &candidate, limit)?
                && crate::curve_fit::g1::within_deviation(source, &candidate, tolerance)?)
        {
            candidate
        } else {
            // Native multispan references enter adaptive fitting here.
            // A common handle correction is qualified only for a single
            // Bezier source, even if it could satisfy a positional bound.
            if !single_span {
                return Err(unsupported());
            }
            // The complete initial proposal has already failed. A fitting
            // midpoint alone must not prevent the single-profile correction;
            // its rounded output still needs the full source certificate.
            let Some(adjusted) = crate::curve_fit::g1::adjusted_cubic(
                source,
                &candidate,
                if policy.single { 0. } else { limit },
            )?
            else {
                return Err(unsupported());
            };
            if !crate::curve_fit::g1::within_deviation(source, &adjusted, limit)? {
                return Err(unsupported());
            }
            adjusted
        };
        sections.push(candidate);
    }
    Ok(sections)
}

/// Measured multispan initial fits can exceed the half-tolerance reserve
/// between these stations. This selection rule cannot certify a fit: the
/// caller must still prove the complete image bound within full tolerance.
fn quarter_stations_fit(
    source: &NurbsCurve,
    candidate: &NurbsCurve,
    limit: Real,
) -> Result<bool, GeometryError> {
    let sampler = crate::ArcLengthSampler::try_new(
        crate::CurveRef::NurbsCurve(source),
        crate::Tolerance::DEFAULT,
    )?;
    // Preserve the existing eight-interval incremental fitting clock.
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
    let parameters = sampler.parameters_at_incremental_normalized_lengths(
        &fractions,
        crate::curve_fit::g1::FITTING_BATCH_TOLERANCE,
    )?;
    let search = candidate.prepare_closest_parameter_search()?;
    let numerical =
        crate::Tolerance::try_new((limit * 1e-6).max(Real::from_bits(1)), 1e-12, 1e-10)?;
    for index in [2, 4, 6] {
        let point = source.evaluate(parameters[index])?;
        let parameter = search.closest_parameter(point, numerical)?;
        if point.distance_to(candidate.evaluate(parameter)?)? > limit {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod multispan_tests;
#[cfg(test)]
mod tests;
