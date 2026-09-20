//! Inverse surface-domain mapping without overflowing interval differences.
use super::*;

impl NurbsSurface {
    /// Converts native parameters to fractions in `[0,1]`, preserving endpoints.
    pub fn normalized_parameters(&self, u: Real, v: Real) -> Result<[Real; 2], GeometryError> {
        Ok([fraction(u, self.domain_u())?, fraction(v, self.domain_v())?])
    }
}

fn fraction(value: Real, domain: RangeInclusive<Real>) -> Result<Real, GeometryError> {
    crate::parameter::checked_parameter(value, domain.clone())?;
    let (mut start, mut end, mut value) = (*domain.start(), *domain.end(), value);
    if value == start {
        return Ok(0.);
    }
    if value == end {
        return Ok(1.);
    }
    if !(end - start).is_finite() {
        start *= 0.5;
        end *= 0.5;
        value *= 0.5;
    }
    let from_start = value - start;
    let from_end = end - value;
    Ok(if from_start <= from_end {
        from_start / (end - start)
    } else {
        1. - from_end / (end - start)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_preserve_endpoints_and_cover_wide_and_subnormal_domains() {
        for (domain, midpoint) in [
            (-Real::MAX..=Real::MAX, 0.),
            (-2.0..=8.0, 3.),
            (0.0..=Real::from_bits(2), Real::from_bits(1)),
        ] {
            assert_eq!(fraction(*domain.start(), domain.clone()).unwrap(), 0.);
            assert_eq!(fraction(*domain.end(), domain.clone()).unwrap(), 1.);
            assert_eq!(fraction(midpoint, domain).unwrap(), 0.5);
        }
        assert!(fraction(Real::NAN, 0.0..=1.0).is_err());
        assert!(fraction(2., 0.0..=1.0).is_err());
    }
}
