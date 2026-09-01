use crate::{GeometryError, Real, Tolerance, Vector3, require_finite};

/// A finite point in three-dimensional model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3(nalgebra::Point3<Real>);

impl Point3 {
    pub fn try_new(x: Real, y: Real, z: Real) -> Result<Self, GeometryError> {
        require_finite([x, y, z], "point")?;
        Ok(Self(nalgebra::Point3::new(x, y, z)))
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
    pub fn z(self) -> Real {
        self.0.z
    }

    #[inline]
    pub fn to_array(self) -> [Real; 3] {
        [self.x(), self.y(), self.z()]
    }

    /// Euclidean distance, computed with scaled hypot operations to avoid
    /// intermediate overflow.
    pub fn distance_to(self, other: Self) -> Result<Real, GeometryError> {
        self.vector_to(other)?.length()
    }

    pub fn vector_to(self, other: Self) -> Result<Vector3, GeometryError> {
        Vector3::try_new(
            other.x() - self.x(),
            other.y() - self.y(),
            other.z() - self.z(),
        )
    }

    pub fn is_near(self, other: Self, tolerance: Tolerance) -> bool {
        self.distance_to(other)
            .is_ok_and(|distance| distance <= tolerance.absolute())
    }

    pub fn translated(self, offset: Vector3) -> Result<Self, GeometryError> {
        Self::try_new(
            self.x() + offset.x(),
            self.y() + offset.y(),
            self.z() + offset.z(),
        )
    }
}

impl TryFrom<[Real; 3]> for Point3 {
    type Error = GeometryError;

    fn try_from(value: [Real; 3]) -> Result<Self, Self::Error> {
        Self::try_new(value[0], value[1], value[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_coordinates() {
        assert!(Point3::try_new(0.0, Real::NAN, 0.0).is_err());
        assert!(Point3::try_new(0.0, 0.0, Real::INFINITY).is_err());
    }

    #[test]
    fn distance_is_stable_for_large_components() {
        let a = Point3::try_new(1.0e200, 1.0e200, 0.0).unwrap();
        let b = Point3::try_new(2.0e200, 2.0e200, 0.0).unwrap();
        let expected = 2.0_f64.sqrt() * 1.0e200;
        assert!(
            Tolerance::try_new(1.0e-9, 1.0e-14, 1.0e-9)
                .unwrap()
                .approx_eq(a.distance_to(b).unwrap(), expected)
        );
    }

    #[test]
    fn overflowing_delta_is_reported() {
        let a = Point3::try_new(-Real::MAX, 0.0, 0.0).unwrap();
        let b = Point3::try_new(Real::MAX, 0.0, 0.0).unwrap();
        assert_eq!(
            a.distance_to(b),
            Err(GeometryError::NonFinite { context: "vector" })
        );
    }
}
