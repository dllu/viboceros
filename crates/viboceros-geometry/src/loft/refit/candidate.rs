//! Admit an initial or corrected G1 proposal after a continuous source proof.
use super::*;

pub(super) fn prepare(
    curves: &[NurbsCurve],
    tolerance: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let unsupported = || GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires certified straight regions or certified smooth cubics; adaptive subdivision remains unfinished",
    };
    let mut sections = Vec::with_capacity(curves.len());
    for source in curves {
        let degree = source.degree();
        let domain = source.domain();
        let knots = source.knots();
        // The initial smooth branch covers single, clamped Bezier profiles.
        // Internal kinks/regions need the existing polygon path or the pending
        // adaptive fitter; closed and multispan smooth curves need subdivision.
        if degree > 16
            || source.control_points().len() != degree + 1
            || !knots[..=degree].iter().all(|k| k == domain.start())
            || !knots[degree + 1..].iter().all(|k| k == domain.end())
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
mod tests;
