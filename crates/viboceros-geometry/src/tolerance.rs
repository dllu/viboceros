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
    /// Already-defined mesh facets need nonzero edges and a nonzero numerical
    /// cross product, not a minimum modelling angle or feature size. Do not use
    /// this for approximate frame orthogonality, joining, or fitting.
    pub const MESH_VALIDATION: Self = Self {
        absolute: Real::MIN_POSITIVE,
        relative: Real::MIN_POSITIVE,
        angular: Real::MIN_POSITIVE,
    };

    /// Validation policy for already-defined primitives, not a modelling or
    /// joining tolerance. Rejects zero/subnormal-size degeneracies without
    /// imposing a document-dependent minimum feature size. Approximate topology
    /// and fitting still require a dimensional tolerance from their caller.
    pub const NUMERICAL_VALIDATION: Self = Self {
        absolute: Real::MIN_POSITIVE,
        relative: Self::DEFAULT.relative,
        angular: Self::DEFAULT.angular,
    };

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
    fn mesh_validation_rejects_exactly_collinear_integer_edges() {
        use crate::{Point3, TriangleMesh};
        for x in 1..8 {
            for y in 1..8 {
                for multiplier in 2..8 {
                    let points = vec![
                        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                        Point3::try_new(x as f64, y as f64, 0.0).unwrap(),
                        Point3::try_new((x * multiplier) as f64, (y * multiplier) as f64, 0.0)
                            .unwrap(),
                    ];
                    assert!(
                        TriangleMesh::try_new(points, vec![[0, 1, 2]], Tolerance::MESH_VALIDATION)
                            .is_err(),
                        "accepted collinear {x},{y} times {multiplier}"
                    );
                }
            }
        }
    }

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
