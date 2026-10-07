//! Anchor-relative signed arc-length pieces, retaining native leaf geometry.
use crate::{Curve3, GeometryError, Real, Tolerance};
#[cfg(test)]
mod tests;

impl Curve3 {
    /// Extracts a signed arc-length interval from a native parameter.
    /// Closed curves may cross their seam, for at most one complete traversal.
    /// Returns `None` when the requested length exceeds the available curve.
    /// Zero length and invalid anchors fail. Integration starts at the anchor,
    /// so a short interval is not lost behind a large cumulative length prefix.
    pub fn try_subcurve_at_arc_length(
        &self,
        anchor: Real,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        crate::require_finite([anchor, length], "subcurve arc length")?;
        let domain = self.as_ref().domain();
        if !domain.contains(&anchor) || length == 0. {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let (oriented, anchor) = if length < 0. {
            (self.reversed(tolerance)?, -anchor)
        } else {
            (self.clone(), anchor)
        };
        let source = if oriented.as_ref().is_closed()? {
            oriented.try_change_closed_seam(anchor)?
        } else {
            let end = *oriented.as_ref().domain().end();
            if anchor == end {
                return Ok(None);
            }
            oriented.try_trimmed(anchor..=end)?
        };
        let sampler = crate::curve::ArcLengthSampler::try_new(source.as_ref(), tolerance)?;
        if length.abs() > sampler.total_length() {
            return Ok(None);
        }
        let end = sampler.parameter_at_distance(length.abs())?;
        Ok(Some(
            source.try_trimmed(*source.as_ref().domain().start()..=end)?,
        ))
    }
}
