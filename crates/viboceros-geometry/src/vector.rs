use crate::{GeometryError, Real, Tolerance, require_finite};

/// A finite vector in three-dimensional model space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3(nalgebra::Vector3<Real>);

impl Vector3 {
    pub fn try_new(x: Real, y: Real, z: Real) -> Result<Self, GeometryError> {
        require_finite([x, y, z], "vector")?;
        Ok(Self(nalgebra::Vector3::new(x, y, z)))
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

    /// A scaled norm that avoids intermediate overflow and reports a result
    /// whose true magnitude is not representable by [`Real`].
    pub fn length(self) -> Result<Real, GeometryError> {
        let value = self.x().hypot(self.y()).hypot(self.z());
        require_finite([value], "vector length")?;
        Ok(value)
    }

    pub fn dot(self, other: Self) -> Result<Real, GeometryError> {
        let left_scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        let right_scale = other.x().abs().max(other.y().abs()).max(other.z().abs());
        if left_scale == 0.0 || right_scale == 0.0 {
            return Ok(0.0);
        }

        // Scaling avoids overflowing individual products when large terms
        // cancel. Trying every association avoids both spurious overflow and
        // spurious underflow in an otherwise representable three-factor result.
        let left = self.to_array().map(|value| value / left_scale);
        let right = other.to_array().map(|value| value / right_scale);
        let normalized = left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]));
        let magnitude = product_three(normalized.abs(), left_scale, right_scale, "dot product")?;
        Ok(normalized.signum() * magnitude)
    }

    pub fn cross(self, other: Self) -> Result<Self, GeometryError> {
        let left_scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        let right_scale = other.x().abs().max(other.y().abs()).max(other.z().abs());
        if left_scale == 0.0 || right_scale == 0.0 {
            return Self::try_new(0.0, 0.0, 0.0);
        }

        let left = self.to_array().map(|value| value / left_scale);
        let right = other.to_array().map(|value| value / right_scale);
        let normalized = [
            left[1].mul_add(right[2], -left[2] * right[1]),
            left[2].mul_add(right[0], -left[0] * right[2]),
            left[0].mul_add(right[1], -left[1] * right[0]),
        ];
        let mut result = [0.0; 3];
        for (index, component) in normalized.into_iter().enumerate() {
            result[index] = component.signum()
                * product_three(component.abs(), left_scale, right_scale, "cross product")?;
        }
        Self::try_from(result)
    }

    pub fn scaled(self, scale: Real) -> Result<Self, GeometryError> {
        require_finite([scale], "scale")?;
        Self::try_new(self.x() * scale, self.y() * scale, self.z() * scale)
    }

    pub fn normalized(self, tolerance: Tolerance) -> Result<UnitVector3, GeometryError> {
        let scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        if scale == 0.0 {
            return Err(GeometryError::Degenerate { context: "vector" });
        }

        // Divide before taking the norm, so very large vectors remain safe.
        let x = self.x() / scale;
        let y = self.y() / scale;
        let z = self.z() / scale;
        let scaled_length = x.hypot(y).hypot(z);
        if scale <= tolerance.absolute() / scaled_length {
            return Err(GeometryError::Degenerate { context: "vector" });
        }
        self.normalized_with_scale(scale, scaled_length)
    }

    /// Returns the direction of any mathematically non-zero finite vector.
    /// Geometry constructors should normally use [`Self::normalized`] so they
    /// honor model tolerance; this is for recomputing derived data from an
    /// object that has already been validated.
    pub(crate) fn normalized_nonzero(self) -> Result<UnitVector3, GeometryError> {
        let scale = self.x().abs().max(self.y().abs()).max(self.z().abs());
        if scale == 0.0 {
            return Err(GeometryError::Degenerate { context: "vector" });
        }
        let scaled_length = (self.x() / scale)
            .hypot(self.y() / scale)
            .hypot(self.z() / scale);
        self.normalized_with_scale(scale, scaled_length)
    }

    fn normalized_with_scale(
        self,
        scale: Real,
        scaled_length: Real,
    ) -> Result<UnitVector3, GeometryError> {
        let x = self.x() / scale;
        let y = self.y() / scale;
        let z = self.z() / scale;
        let inverse_length = 1.0 / scaled_length;
        let vector = Self::try_new(x * inverse_length, y * inverse_length, z * inverse_length)?;
        Ok(UnitVector3(vector))
    }
}

fn product_three(
    first: Real,
    second: Real,
    third: Real,
    context: &'static str,
) -> Result<Real, GeometryError> {
    if first == 0.0 || second == 0.0 || third == 0.0 {
        return Ok(0.0);
    }

    let mut underflowed = false;
    for (left, right, remaining) in [
        (first, second, third),
        (first, third, second),
        (second, third, first),
    ] {
        let pair = left * right;
        if !pair.is_finite() {
            continue;
        }
        let product = pair * remaining;
        if product.is_finite() {
            if product != 0.0 {
                return Ok(product);
            }
            underflowed = true;
        }
    }

    if underflowed {
        Ok(0.0)
    } else {
        Err(GeometryError::NonFinite { context })
    }
}

/// A finite vector normalized to unit length at construction time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitVector3(pub(crate) Vector3);

impl UnitVector3 {
    pub fn try_new(x: Real, y: Real, z: Real, tolerance: Tolerance) -> Result<Self, GeometryError> {
        Vector3::try_new(x, y, z)?.normalized(tolerance)
    }

    #[inline]
    pub fn x(self) -> Real {
        self.0.x()
    }

    #[inline]
    pub fn y(self) -> Real {
        self.0.y()
    }

    #[inline]
    pub fn z(self) -> Real {
        self.0.z()
    }

    #[inline]
    pub const fn as_vector(self) -> Vector3 {
        self.0
    }
}

impl TryFrom<[Real; 3]> for Vector3 {
    type Error = GeometryError;

    fn try_from(value: [Real; 3]) -> Result<Self, Self::Error> {
        Self::try_new(value[0], value[1], value[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_huge_vectors_without_overflow() {
        let unit = Vector3::try_new(Real::MAX, Real::MAX, 0.0)
            .unwrap()
            .normalized(Tolerance::DEFAULT)
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(unit.as_vector().length().unwrap(), 1.0));
    }

    #[test]
    fn rejects_vectors_below_model_tolerance() {
        let vector = Vector3::try_new(1.0e-12, 0.0, 0.0).unwrap();
        assert_eq!(
            vector.normalized(Tolerance::DEFAULT),
            Err(GeometryError::Degenerate { context: "vector" })
        );
    }

    #[test]
    fn degeneracy_uses_vector_length_not_largest_component() {
        let absolute = Tolerance::DEFAULT.absolute();
        let vector = Vector3::try_new(0.8 * absolute, 0.8 * absolute, 0.8 * absolute).unwrap();
        assert!(vector.normalized(Tolerance::DEFAULT).is_ok());
    }

    #[test]
    fn dot_product_handles_large_cancelling_terms() {
        let left = Vector3::try_new(1.0, 1.0, -1.0).unwrap();
        let right = Vector3::try_new(Real::MAX, Real::MAX, Real::MAX).unwrap();
        assert_eq!(left.dot(right).unwrap(), Real::MAX);
    }

    #[test]
    fn dot_product_avoids_spurious_intermediate_underflow() {
        let left = Vector3::try_new(1.0e-200, 0.0, 0.0).unwrap();
        let right = Vector3::try_new(1.0e-108, 0.0, 0.0).unwrap();
        assert_eq!(left.dot(right).unwrap(), 1.0e-308);
    }

    #[test]
    fn cross_product_is_oriented_and_scaled_without_intermediate_overflow() {
        let x = Vector3::try_new(Real::MAX, 0.0, 0.0).unwrap();
        let y = Vector3::try_new(0.0, 1.0, 0.0).unwrap();
        assert_eq!(
            x.cross(y).unwrap(),
            Vector3::try_new(0.0, 0.0, Real::MAX).unwrap()
        );
        assert_eq!(
            y.cross(x).unwrap(),
            Vector3::try_new(0.0, 0.0, -Real::MAX).unwrap()
        );
    }
}
