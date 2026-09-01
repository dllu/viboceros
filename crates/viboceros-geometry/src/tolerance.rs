use crate::{GeometryError, Real};

/// Absolute, relative, and angular tolerances used by geometric predicates.
///
/// Keeping the policy in a value passed by the caller avoids hidden global
/// epsilon choices and allows a document to retain its own modelling tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    absolute: Real,
    relative: Real,
    angular: Real,
}

impl Tolerance {
    /// Conservative defaults for a unit-agnostic new document.
    pub const DEFAULT: Self = Self {
        absolute: 1.0e-9,
        relative: 1.0e-12,
        angular: 1.0e-10,
    };

    pub fn try_new(absolute: Real, relative: Real, angular: Real) -> Result<Self, GeometryError> {
        if [absolute, relative, angular]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
        {
            Ok(Self {
                absolute,
                relative,
                angular,
            })
        } else {
            Err(GeometryError::InvalidTolerance)
        }
    }

    #[inline]
    pub const fn absolute(self) -> Real {
        self.absolute
    }

    #[inline]
    pub const fn relative(self) -> Real {
        self.relative
    }

    #[inline]
    pub const fn angular(self) -> Real {
        self.angular
    }

    /// Combined absolute/relative comparison for scalar quantities.
    #[inline]
    pub fn approx_eq(self, left: Real, right: Real) -> bool {
        if !left.is_finite() || !right.is_finite() {
            return false;
        }
        if left == right {
            return true;
        }

        let scale = left.abs().max(right.abs());
        (left - right).abs() <= self.absolute.max(self.relative * scale)
    }
}

impl Default for Tolerance {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_components() {
        for value in [0.0, -1.0, Real::NAN, Real::INFINITY] {
            assert!(Tolerance::try_new(value, 1.0e-9, 1.0e-9).is_err());
        }
    }

    #[test]
    fn combines_absolute_and_relative_comparisons() {
        let tolerance = Tolerance::try_new(1.0e-6, 1.0e-3, 1.0e-8).unwrap();
        assert!(tolerance.approx_eq(0.0, 5.0e-7));
        assert!(tolerance.approx_eq(1_000.0, 1_000.5));
        assert!(!tolerance.approx_eq(1.0, 1.01));
        assert!(!tolerance.approx_eq(Real::NAN, Real::NAN));
        assert!(!tolerance.approx_eq(Real::INFINITY, Real::INFINITY));
    }
}
