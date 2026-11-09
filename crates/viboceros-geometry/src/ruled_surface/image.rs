//! Complete ruled images from complete boundary-curve correspondence bounds.
use crate::{GeometryError, NurbsCurve, NurbsSurface, Real};

impl NurbsSurface {
    /// Bound the symmetric Hausdorff distance of complete ruled images.
    /// Requires degree one and two controls in U, with common-sign weights
    /// on each tensor. V uses normalized parameter correspondence. Independent
    /// boundary gauges may change the U clock; no U correspondence, trimming,
    /// normals, topology or injectivity is certified.
    pub fn ruled_surface_image_deviation_bound(
        &self,
        other: &Self,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        self.ruled_surface_image_deviation_bound_with_witnesses(other, [None, None], limit)
    }

    /// Optional boundary witnesses avoid repeatedly comparing densely refined
    /// copies of the same source curve. Both actual boundaries are certified
    /// against the supplied witness; the outward sum bounds their difference.
    /// Witnesses are never trusted as replacements for stored geometry.
    pub fn ruled_surface_image_deviation_bound_with_witnesses(
        &self,
        other: &Self,
        witnesses: [Option<&NurbsCurve>; 2],
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if !limit.is_finite() || limit < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        for s in [self, other] {
            if s.degree_u() != 1 || s.control_point_count_u() != 2 {
                return Ok(None);
            }
            let sign = s.control_points()[0].weight().signum();
            if s.control_points()
                .iter()
                .any(|p| p.weight().signum() != sign)
            {
                return Ok(None);
            }
        }
        if self == other {
            return Ok(Some(0.));
        }
        let mut maximum = 0_f64;
        // The offset boundary usually rejects a wrong side before the
        // potentially dense rational source boundary needs certification.
        for i in [1, 0] {
            let end = i == 1;
            let boundary = |s: &Self| {
                s.isocurve_v(if end {
                    *s.domain_u().end()
                } else {
                    *s.domain_u().start()
                })
            };
            let a = boundary(self)?;
            let b = boundary(other)?;
            let bound = if let Some(w) = witnesses[i] {
                let Some(x) = a.parameter_curve_deviation_bound(w, limit * 0.5)? else {
                    return Ok(None);
                };
                let Some(y) = b.parameter_curve_deviation_bound(w, limit * 0.5)? else {
                    return Ok(None);
                };
                let sum = x + y;
                if sum == 0. { 0. } else { sum.next_up() }
            } else {
                let Some(x) = a.parameter_curve_deviation_bound(&b, limit)? else {
                    return Ok(None);
                };
                x
            };
            maximum = maximum.max(bound);
        }
        // For each V, common-sign weights sweep the entire segment between
        // the two boundary points as U traverses its active interval. Pair the
        // same affine segment fraction on both surfaces. Convexity bounds its
        // displacement by the larger boundary bound, in both image directions.
        Ok((maximum <= limit).then_some(maximum))
    }
}

#[cfg(test)]
mod tests;
