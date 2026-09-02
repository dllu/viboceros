use crate::{GeometryError, Real, Tolerance, require_finite};

/// A finite point in a surface's two-dimensional parameter space.
///
/// Keeping parameter-space points distinct from model-space [`crate::Point3`]
/// prevents trim curves from accidentally being transformed as 3D geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2(nalgebra::Point2<Real>);

impl Point2 {
    pub fn try_new(x: Real, y: Real) -> Result<Self, GeometryError> {
        require_finite([x, y], "parameter-space point")?;
        Ok(Self(nalgebra::Point2::new(x, y)))
    }

    #[inline]
    pub fn x(self) -> Real {
        self.0.x
    }

    #[inline]
    pub fn y(self) -> Real {
        self.0.y
    }

    #[inline]
    pub fn to_array(self) -> [Real; 2] {
        [self.x(), self.y()]
    }

    /// Euclidean parameter-space distance using `hypot` to avoid spurious
    /// overflow in the squared components.
    pub fn distance_to(self, other: Self) -> Result<Real, GeometryError> {
        let distance = (other.x() - self.x()).hypot(other.y() - self.y());
        require_finite([distance], "parameter-space point distance")?;
        Ok(distance)
    }

    pub fn is_near(self, other: Self, tolerance: Tolerance) -> bool {
        self.distance_to(other)
            .is_ok_and(|distance| distance <= tolerance.absolute())
    }
}

impl TryFrom<[Real; 2]> for Point2 {
    type Error = GeometryError;

    fn try_from(value: [Real; 2]) -> Result<Self, Self::Error> {
        Self::try_new(value[0], value[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_points_reject_non_finite_coordinates() {
        assert!(Point2::try_new(Real::NAN, 0.0).is_err());
        assert!(Point2::try_new(0.0, Real::INFINITY).is_err());
    }

    #[test]
    fn parameter_point_distance_is_stable_for_large_components() {
        let first = Point2::try_new(1.0e200, 1.0e200).unwrap();
        let second = Point2::try_new(2.0e200, 2.0e200).unwrap();
        let expected = 2.0_f64.sqrt() * 1.0e200;
        assert!(
            Tolerance::try_new(1.0e-9, 1.0e-14, 1.0e-9)
                .unwrap()
                .approx_eq(first.distance_to(second).unwrap(), expected)
        );
    }
}
