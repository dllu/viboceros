//! Admit an initial or corrected G1 proposal after a continuous source proof.
use super::*;

pub(super) fn prepare(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
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
        // The native smooth proposal reserves half the requested tolerance.
        // Correspondence proposals may use closest points, but acceptance
        // requires a complete image bound; unresolved maps need the fitter.
        if tolerance >= chord {
            return Err(unsupported());
        }
        let candidate =
            if crate::curve_fit::g1::within_deviation(source, &candidate, tolerance * 0.5)? {
                candidate
            } else {
                // Native multispan references enter adaptive fitting here.
                // A common handle correction is qualified only for a single
                // Bezier source, even if it could satisfy a positional bound.
                if !single_span {
                    return Err(unsupported());
                }
                let Some(adjusted) =
                    crate::curve_fit::g1::adjusted_cubic(source, &candidate, tolerance * 0.5)?
                else {
                    return Err(unsupported());
                };
                if !crate::curve_fit::g1::within_deviation(source, &adjusted, tolerance * 0.5)? {
                    return Err(unsupported());
                }
                adjusted
            };
        sections.push(candidate);
    }
    Ok(sections)
}

#[cfg(test)]
mod multispan_tests;
#[cfg(test)]
mod tests;
