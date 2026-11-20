//! Sweep1 section fitting before frame transport and point-end construction.
use super::*;
use crate::ArcLengthSampler;
use crate::curve_fit::{cubic_polygon, g1};
mod circle;

impl Sweep1 {
    /// Apply an affine section chart after common-basis preparation.
    /// The local frame controls stay valid because only knots are remapped.
    pub fn try_with_section_domain(
        mut self,
        domain: RangeInclusive<Real>,
    ) -> Result<Self, GeometryError> {
        for section in &mut self.sections {
            section.curve = section.curve.try_reparameterized(domain.clone())?;
        }
        Ok(self)
    }

    /// Fit ordered real profiles while preserving their rail stations.
    ///
    /// Supports exact polygon regions, regular open single-span cubic proposals,
    /// and full circular profiles that can use four cubic spans. Inconclusive
    /// proposals and profiles requiring adaptive fitting return an error.
    pub fn refit_sections(
        mut sections: Vec<SweepSection>,
        fit_tolerance: Real,
    ) -> Result<Vec<SweepSection>, GeometryError> {
        if !fit_tolerance.is_finite() || fit_tolerance < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        if sections.is_empty() || sections.len() > MAX_SECTIONS {
            return Err(invalid("expected 1 to 256 sections"));
        }
        // Native Sweep1 preserves the original section at a negligible fit distance.
        if fit_tolerance <= 2. * Real::EPSILON.sqrt() {
            return Ok(sections);
        }
        let single = sections.len() == 1;
        let limit = if single {
            fit_tolerance
        } else {
            fit_tolerance * 0.5
        };
        for section in &mut sections {
            if section.curve.control_points().len() > MAX_SECTION_CONTROLS {
                return Err(invalid("refit source section control budget exceeded"));
            }
            section.curve = refit(&section.curve, limit, single)?;
        }
        Ok(sections)
    }
}

fn unsupported() -> GeometryError {
    GeometryError::UnsupportedCurveApproximation {
        context: "Sweep1 Refit requires exact straight regions, four-span circular fits, or a verified single cubic. Adaptive section fitting remains unfinished",
    }
}

fn refit(source: &NurbsCurve, limit: Real, single: bool) -> Result<NurbsCurve, GeometryError> {
    if let Some(vertices) = crate::surface_pullback::exact_linear_curve_vertices(source)? {
        if 3 * (vertices.len() - 1) + 1 > MAX_SECTION_CONTROLS {
            return Err(invalid("refit polygon section control budget exceeded"));
        }
        return cubic_polygon(&vertices, limit);
    }
    if source.is_closed()? {
        return circle::fit(source, limit, single)?.ok_or_else(unsupported);
    }
    // Regular Bézier sources need no separate corner or discontinuity policy.
    if source.control_points().len() != source.degree() + 1
        || source.degree() > 16
        || source
            .control_points()
            .iter()
            .any(|c| c.weight() != source.control_points()[0].weight())
    {
        return Err(unsupported());
    }
    let initial = g1::initial_cubic(source).map_err(|_| unsupported())?;
    let chord = initial.control_points()[0]
        .point()
        .distance_to(initial.control_points()[3].point())?;
    if limit >= chord {
        return Err(unsupported());
    }
    let first = one_pass(source, &initial, limit)?;
    if !single {
        return Ok(first);
    }
    // Lone profiles use the fitted cubic as the source of a second pass.
    let clock = fit_clock(source, &first)?;
    let fitted = if first == initial {
        first
    } else {
        let fitted = one_pass(&first, &initial, limit)?;
        if !g1::within_deviation(source, &fitted, comparison_limit(limit))? {
            return Err(unsupported());
        }
        fitted
    };
    fitted.try_reparameterized(0. ..=clock)
}

fn one_pass(
    source: &NurbsCurve,
    initial: &NurbsCurve,
    limit: Real,
) -> Result<NurbsCurve, GeometryError> {
    let sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(source), Tolerance::DEFAULT)?;
    let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
    let parameters = sampler.parameters_at_incremental_normalized_lengths(&fractions, 1e-4)?;
    let samples = parameters[1..8]
        .iter()
        .map(|&t| source.evaluate(t))
        .collect::<Result<Vec<_>, _>>()?;
    let fits = |candidate: &NurbsCurve| -> Result<bool, GeometryError> {
        let search = candidate.prepare_closest_parameter_search()?;
        for &point in &samples {
            let closest = search.closest_parameter(point, Tolerance::DEFAULT)?;
            if candidate.evaluate(closest)?.distance_to(point)? > limit {
                return Ok(false);
            }
        }
        // Native acceptance uses stations. A separate complete image bound
        // guards unmeasured material between the stations.
        g1::within_deviation(source, candidate, comparison_limit(limit))
    };
    if fits(initial)? {
        return Ok(initial.clone());
    }
    let adjusted =
        g1::adjusted_cubic_at_point(initial, samples[3], limit)?.ok_or_else(unsupported)?;
    if !fits(&adjusted)? {
        return Err(unsupported());
    }
    Ok(adjusted)
}

fn comparison_limit(station_limit: Real) -> Real {
    (2. * station_limit).min(Real::MAX)
}

fn fit_length(curve: &NurbsCurve) -> Result<Real, GeometryError> {
    ArcLengthSampler::try_new(CurveRef::NurbsCurve(curve), Tolerance::DEFAULT)?
        .quadrature_length_estimate(1e-4)
}

fn fit_clock(source: &NurbsCurve, fitted: &NurbsCurve) -> Result<Real, GeometryError> {
    let source_length = fit_length(source)?;
    let fitted_length = fit_length(fitted)?;
    // Retain the source clock for small relative length changes. Larger edits
    // use the length of the first fit, before the final handle correction.
    if (source_length - fitted_length).abs() <= source_length * 0.001 {
        Ok(source_length)
    } else {
        Ok(fitted_length)
    }
}
