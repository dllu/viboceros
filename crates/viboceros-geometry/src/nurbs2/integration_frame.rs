//! Lossless temporary parameter frames for UV boundary integration.
use super::*;
use crate::parameter::lossless_parameter_origin;
use std::borrow::Cow;

#[cfg(test)]
mod tests;

impl NurbsCurve2 {
    /// Removes a knot origin when every subtraction is exact, then rescales
    /// by an exact power of two. Controls, weights, knot multiplicities, and
    /// relative span widths are unchanged. No native parameter is returned.
    /// A range that cannot be rescaled without losing a knot is rejected.
    pub(crate) fn for_integration(&self) -> Result<Cow<'_, Self>, GeometryError> {
        let domain = self.domain();
        let origin = lossless_parameter_origin(domain.clone(), self.knots.iter().copied());
        let magnitude = (domain.start() - origin)
            .abs()
            .max((domain.end() - origin).abs());
        let bits = magnitude.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let exponent = if exponent == 0 {
            (bits & ((1_u64 << 52) - 1)).ilog2() as i32 - 1074
        } else {
            exponent - 1023
        };
        if origin == 0. && exponent == 0 {
            return Ok(Cow::Borrowed(self));
        }
        let mut knots = self.knots.iter().map(|k| k - origin).collect::<Vec<_>>();
        let mut shift = -exponent;
        // Bounded factors keep both each multiplier and its inverse normal,
        // including domains as small as one subnormal unit or as large as MAX.
        while shift != 0 {
            let step = shift.clamp(-512, 512);
            let factor = 2_f64.powi(step);
            let inverse = 2_f64.powi(-step);
            for knot in &mut knots {
                let mapped = *knot * factor;
                if !mapped.is_finite() || mapped * inverse != *knot {
                    return Err(GeometryError::NumericalIntegrationDidNotConverge);
                }
                *knot = mapped;
            }
            shift -= step;
        }
        Ok(Cow::Owned(Self::try_new_rational(
            self.degree,
            self.control_points.clone(),
            knots,
        )?))
    }
}
