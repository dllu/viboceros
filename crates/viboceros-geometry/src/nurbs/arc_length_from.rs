//! Anchor-relative inversion without subtracting large arc-length prefixes.
use crate::{
    CurveRef, GeometryError, NurbsCurve, Real, Tolerance, curve::ArcLengthSampler, require_finite,
};

impl NurbsCurve {
    /// Finds a native parameter a signed arc length from `anchor`.
    ///
    /// Positive distances follow increasing parameters; negative distances follow
    /// decreasing parameters. Returns `None` beyond the chosen endpoint, including
    /// on closed curves: this query does not wrap through a seam. Zero returns the
    /// anchor exactly. Invalid parameters and numerical integration errors fail.
    ///
    /// Integration starts at the anchor on an exactly trimmed, optionally reversed
    /// curve. A short offset after a very long prefix is therefore not rounded away
    /// by adding it to a global cumulative length. Accuracy and native-parameter
    /// resolution remain limited by floating-point integration and representation.
    pub fn parameter_at_arc_length_from(
        &self,
        anchor: Real,
        distance: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Real>, GeometryError> {
        require_finite([distance], "anchor-relative curve distance")?;
        let domain = self.domain();
        if !anchor.is_finite() || !domain.contains(&anchor) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter: anchor,
                domain_start: *domain.start(),
                domain_end: *domain.end(),
            });
        }
        if distance == 0.0 {
            return Ok(Some(anchor));
        }
        let forward = distance > 0.0;
        let endpoint = if forward {
            *domain.end()
        } else {
            *domain.start()
        };
        if anchor == endpoint {
            return Ok(None);
        }
        let piece = if forward {
            self.try_trimmed(anchor..=endpoint)?
        } else {
            self.try_trimmed(endpoint..=anchor)?.reversed()?
        };
        let sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&piece), tolerance)?;
        if distance.abs() > sampler.total_length() {
            return Ok(None);
        }
        let parameter = sampler.parameter_at_distance(distance.abs())?;
        // Reversal negates the native domain; it does not preserve [a,b].
        Ok(Some(if forward { parameter } else { -parameter }))
    }
}

#[cfg(test)]
mod tests;
