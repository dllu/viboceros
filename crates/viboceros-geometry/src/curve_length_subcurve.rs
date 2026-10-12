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
        Ok(self
            .try_subcurve_at_arc_length_with_endpoint(anchor, length, tolerance)?
            .map(|(curve, _)| curve))
    }

    /// Returns the piece together with its endpoint in the original source's
    /// native domain. Closed seam crossings map back without geometric closest
    /// point guesses, including on self-intersecting curves.
    pub fn try_subcurve_at_arc_length_with_endpoint(
        &self,
        anchor: Real,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Option<(Self, Real)>, GeometryError> {
        self.try_subcurve_at_arc_length_with_nurbs(
            anchor,
            length,
            tolerance,
            &mut |curve, interval| curve.try_trimmed(interval),
        )
    }

    /// Retain anchor-relative length sampling and endpoint correspondence while
    /// delegating partial NURBS intervals to a caller-supplied exact extractor.
    /// Closed-seam preparation keeps its existing representation policy.
    pub fn try_subcurve_at_arc_length_with_nurbs<E: From<GeometryError>>(
        &self,
        anchor: Real,
        length: Real,
        tolerance: Tolerance,
        trim: &mut impl FnMut(
            &crate::NurbsCurve,
            std::ops::RangeInclusive<Real>,
        ) -> Result<crate::NurbsCurve, E>,
    ) -> Result<Option<(Self, Real)>, E> {
        crate::require_finite([anchor, length], "subcurve arc length")?;
        let domain = self.as_ref().domain();
        if !domain.contains(&anchor) || length == 0. {
            return Err(GeometryError::InvalidCurveTrimInterval.into());
        }
        let closed = self.as_ref().is_closed()?;
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
            oriented.try_trimmed_with_nurbs(anchor..=end, trim)?
        };
        let sampler = crate::curve::ArcLengthSampler::try_new(source.as_ref(), tolerance)?;
        if length.abs() > sampler.total_length() {
            return Ok(None);
        }
        let end = sampler.parameter_at_distance(length.abs())?;
        let original_end = if length < 0. { -end } else { end };
        let original_end = if closed {
            crate::parameter::wrapped_parameter(original_end, &domain)?
        } else {
            original_end
        };
        Ok(Some((
            source.try_trimmed_with_nurbs(*source.as_ref().domain().start()..=end, trim)?,
            original_end,
        )))
    }
}
