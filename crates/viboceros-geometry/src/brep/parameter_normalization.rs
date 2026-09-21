//! Independent positive axis scales for parameter-space topology predicates.
use super::*;

#[cfg(test)]
mod tests;

pub(super) fn trim_parameter_epsilon(domain: [Real; 2], tolerance: Tolerance) -> Real {
    scaled_width(domain, (256. * Real::EPSILON).max(tolerance.relative()))
}

pub(super) fn floating_parameter_epsilon(domain: [Real; 2]) -> Real {
    scaled_width(domain, 256. * Real::EPSILON)
}

fn scaled_width(domain: [Real; 2], factor: Real) -> Real {
    // Parameter tolerances scale with the interval, not an arbitrary unit
    // floor or a translated coordinate origin. Scale before an overflowing
    // subtraction; an underflowed epsilon means only exact equality merges.
    let width = domain[1] - domain[0];
    if width.is_finite() {
        width * factor
    } else {
        domain[1] * factor - domain[0] * factor
    }
}

#[derive(Clone, Copy)]
pub(super) struct TrimParameterNormalization {
    coordinate_scale: [Real; 2],
    origin: [Real; 2],
    relative_scale: [Real; 2],
}

impl TrimParameterNormalization {
    pub(super) fn try_from_points(parameters: &[Point2]) -> Result<Option<Self>, GeometryError> {
        Self::prepare(parameters, false)
    }

    /// Constrained triangulation must not turn exactly collinear sampled
    /// vertices into tiny faces by dividing their axes by non-binary extents.
    /// Power-of-two divisors preserve normal-range significands. Translation
    /// and subnormal rounding still have the usual binary64 limits.
    pub(super) fn try_for_triangulation(
        parameters: &[Point2],
    ) -> Result<Option<Self>, GeometryError> {
        Self::prepare(parameters, true)
    }

    fn prepare(parameters: &[Point2], dyadic: bool) -> Result<Option<Self>, GeometryError> {
        let divisor = |scale: Real| {
            if !dyadic || scale == 0. {
                return scale;
            }
            let bits = scale.to_bits();
            let exponent = bits & 0x7ff0_0000_0000_0000;
            if exponent != 0 {
                Real::from_bits(exponent)
            } else {
                Real::from_bits(1_u64 << (63 - bits.leading_zeros()))
            }
        };
        let Some(first) = parameters.first() else {
            return Ok(None);
        };
        let mut result = Self {
            coordinate_scale: [1.; 2],
            origin: first.to_array(),
            relative_scale: [1.; 2],
        };
        let mut any_extent = false;
        for axis in 0..2 {
            let origin = first.to_array()[axis];
            let direct = parameters.iter().try_fold(0.0_f64, |scale, point| {
                let difference = point.to_array()[axis] - origin;
                difference
                    .is_finite()
                    .then_some(scale.max(difference.abs()))
            });
            let relative_scale = if let Some(scale) = direct {
                scale
            } else {
                // An overflowing difference uses scaled subtraction only on
                // this axis; it must not erase a small extent on the other.
                let scale = divisor(
                    parameters
                        .iter()
                        .map(|p| p.to_array()[axis].abs())
                        .fold(0., Real::max),
                );
                result.coordinate_scale[axis] = scale;
                result.origin[axis] = origin / scale;
                parameters
                    .iter()
                    .map(|p| (p.to_array()[axis] / scale - result.origin[axis]).abs())
                    .fold(0., Real::max)
            };
            require_finite([relative_scale], "trim parameter normalization")?;
            if relative_scale > 0. {
                result.relative_scale[axis] = divisor(relative_scale);
                any_extent = true;
            }
            // Constant axes retain divisor one, so their normalized values are
            // exactly zero. Subsequent area tests still reject collinear loops.
        }
        Ok(any_extent.then_some(result))
    }

    pub(super) fn normalize(self, parameter: Point2) -> Result<[Real; 2], GeometryError> {
        let normalized = std::array::from_fn(|axis| {
            (parameter.to_array()[axis] / self.coordinate_scale[axis] - self.origin[axis])
                / self.relative_scale[axis]
        });
        require_finite(normalized, "normalized trim parameter")?;
        Ok(normalized)
    }

    /// Logarithm of the area multiplier from normalized to original units.
    /// Avoids overflowing or underflowing products when ordering loop areas.
    pub(super) fn log_area_scale(self) -> Real {
        self.coordinate_scale
            .iter()
            .chain(&self.relative_scale)
            .map(|s| s.ln())
            .sum()
    }
}
